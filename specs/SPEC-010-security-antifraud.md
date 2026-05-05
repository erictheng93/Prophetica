# SPEC-010 — 安全與反作弊（Security & Anti-Fraud）

> 優先級：**P1**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-001, SPEC-005
> 被依賴：SPEC-009

---

## 1. Objective

從基礎設施、應用層、業務層三個層次保護平台與玩家：基礎設施層擋外部攻擊，應用層擋常見漏洞，業務層擋作弊與洗錢行為。

## 2. 基礎設施安全

### 2.1 Cloudflare 配置

| 功能 | 配置 |
|------|------|
| WAF | OWASP Core Rule Set + 自訂規則（高敏感 endpoint）|
| Rate Limiting | per-IP / per-user 多層 |
| Bot Management | challenge unknown bots |
| DDoS L3/4/7 | 自動緩解 |
| Geo-rules | 拒絕 🔴 名單地區（呼應 SPEC-001）|
| TLS | 強制 1.3，HSTS preload |
| API Shield | mTLS for DMM API（SPEC-009）|
| Logpush | 全部 access log push 至 R2 + Loki |

### 2.2 Rate Limit 預設

| Endpoint | Limit |
|---------|-------|
| `POST /api/auth/*` | 10 / min / IP |
| `POST /api/orders` | 100 / min / user |
| `POST /api/orders` (DMM) | 1000 / sec / DMM session |
| `WS connect` | 5 / min / IP |
| `GET /api/markets` | 60 / min / IP |
| 其他預設 | 300 / min / IP |

### 2.3 WAF 自訂規則

```
- Block SQLi / XSS payloads (CRS managed)
- Block /api/admin/* 來自非 office VPN IP
- Block 高頻 / 異常 user-agent
- Challenge 來自 Tor exit nodes
- 拒絕已知 abuse IPs（接 AbuseIPDB）
```

## 3. 應用層安全

### 3.1 認證與授權

#### 3.1.1 概要

- 玩家登入：Email/Phone + Password + TOTP（強制）
- Password：argon2id，min length 12
- Session：JWT Access Token (15 min) + Refresh Token (7 day) + rotation
- 高敏操作 step-up：儲值 > USD 1,000、KYC 升級、自我排除 → 重新 MFA
- Admin / Compliance 後台：mTLS + Hardware MFA（YubiKey）

#### 3.1.2 JWT Access Token 設計

```
Algorithm:   EdDSA (Ed25519)
Issuer:      auth.prophetica
Lifetime:    15 minutes
Storage:     HTTP-only Secure SameSite=Lax cookie
             （SPA 使用時透過 /api/auth/session 取 access token，不直接 expose 給 JS）
```

Claims（自定義 namespace 用 `proph_` 前綴避免與 RFC 7519 衝突）：

```json
{
  "iss": "auth.prophetica",
  "aud": "api.prophetica",
  "sub": "42",                            // user_id (string for JWT spec compliance)
  "iat": 1715731200,
  "exp": 1715732100,
  "jti": "01HX...",                       // ULID, 用於 revocation list

  "proph_kyc_tier": 2,                    // 0/1/2/3，呼應 SPEC-005 §2.1
  "proph_country": "TW",                  // ISO 3166-1，依 KYC 證件
  "proph_self_excluded": false,
  "proph_mfa_verified_at": 1715731200,    // unix sec
  "proph_step_up_until": 0,               // 0 = 未 step-up；> now 表 step-up 有效
  "proph_session_id": "01HX...",          // ULID, 對應 user_sessions.id
  "proph_dmm_id": null,                   // 非空表此 token 屬於 DMM 法人帳戶
  "proph_amr": ["pwd", "totp"]            // RFC 8176 authentication methods reference
}
```

#### 3.1.3 Refresh Token 與 Rotation

```
Type:        Opaque token (32-byte random URL-safe base64)
Storage:     HTTP-only Secure cookie (path=/api/auth)
             + 後端 user_sessions 表 hash 對照
Lifetime:    7 days（每次成功使用後重新發 7 天）
Rotation:    每次 /api/auth/refresh 必發新 access + 新 refresh
             舊 refresh token 立即標記為 used
```

`user_sessions` 表（呼應 SPEC-014 user 服務）：

```sql
CREATE TABLE user_sessions (
    id                  VARCHAR(32) PRIMARY KEY,           -- ULID
    user_id             BIGINT NOT NULL,
    refresh_token_hash  CHAR(64) NOT NULL,                 -- SHA256(refresh_token)
    issued_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at          TIMESTAMPTZ NOT NULL,
    used_at             TIMESTAMPTZ,                        -- 一旦被 rotate 就設定，舊 token 不再可用
    successor_id        VARCHAR(32) REFERENCES user_sessions(id),  -- 指向 rotation 後的新 session
    revoked_at          TIMESTAMPTZ,
    revoke_reason       VARCHAR(64),                        -- 'user_logout' / 'reuse_detected' / 'admin_force' / ...
    user_agent          TEXT,
    ip                  INET,
    UNIQUE (refresh_token_hash)
);
CREATE INDEX idx_sessions_user_active ON user_sessions(user_id) WHERE revoked_at IS NULL AND used_at IS NULL;
```

#### 3.1.4 Refresh Token Reuse Detection（強制）

**Refresh token 必須一次性使用**。若同一 token 被使用第二次：

```
1. 偵測：refresh_token_hash 命中且 used_at IS NOT NULL
2. 系統判定為 token 外洩或重放攻擊
3. 動作：
   a. 立即拒絕本次 refresh，回 ERROR_CODE_REFRESH_TOKEN_REUSED (errors.proto: 113)
   b. 撤銷該 user 在所有裝置上的所有 sessions（連鎖 successor 一併撤）
   c. 玩家 Email + 站內訊息通知「您的帳號可能被盜，請重新登入並改密碼」
   d. 寫入 SIEM critical event
   e. 玩家下次登入需通過 step-up MFA
```

#### 3.1.5 Step-up MFA 機制

對應 SPEC-002 §8 / SPEC-005 / SPEC-006 / SPEC-016 等高敏感操作：

| 操作 | Step-up 觸發條件 | Step-up 有效期 |
|------|----------------|----------------|
| 儲值 > USD 1,000 等值 | 必須 | 30 分鐘 |
| KYC 升級至 Tier 2/3 | 必須 | 完成本次升級即清 |
| 啟用 / 變更自我排除 | 必須 | 完成本次操作即清 |
| 30 天無交易後突然儲值 | 必須 | 30 分鐘 |
| 改密碼 / 改 Email | 必須 | 完成本次操作即清 |
| Admin / Compliance / Oracle / Adjudicator 操作 | 必須（hardware MFA）| 30 分鐘，每筆操作後重置 |

實作流程：

```
1. 玩家發起高敏操作 → API 檢查 jwt.proph_step_up_until > now
2. 不通過 → 回 ERROR_CODE_STEP_UP_REQUIRED (errors.proto: 111)，前端跳 MFA 挑戰
3. 玩家完成 TOTP 挑戰 → /api/auth/step-up 簽發**新** access token
   （proph_step_up_until = now + 30min；不延長 access token 整體 exp）
4. 玩家以新 token 重試原操作
```

#### 3.1.6 Token 撤銷（Revocation）

JWT 為 stateless，不主動驗證每個請求是否在黑名單。為兼顧效能與可撤銷性：

- **登出**：refresh token 標記 `revoked_at`；access token 自然在 ≤ 15 分鐘內過期
- **強制全裝置登出**（用於 token reuse、密碼變更）：撤銷該 user 所有 sessions；access token 透過 Redis 短期 deny list（key: `revoked:{jti}`，TTL = access lifetime）
- **管理員強制下線**：同上 + 寫 audit log

#### 3.1.7 對齊 SPEC-005 KYC tier 之動態 claim 更新

當 KYC tier 升級（如 Tier 1 → Tier 2 通過 SumSub）：

- kyc-service 寫入 `user_kyc_records` + Kafka 發 `kyc.events`
- user-service 訂閱該 topic，更新該 user 之活躍 sessions 對應 cache
- 玩家**現有 access token 仍持原 tier**（最多 15 分鐘陳舊）；refresh 時 user-service 簽發含新 tier 的 access token
- 高敏操作（如 Tier 2 解鎖之高額儲值）若 access token 仍為 Tier 1，回 `ERROR_CODE_KYC_TIER_REQUIRED`，玩家強制 refresh 後成功

### 3.2 OWASP Top 10 對應

| 風險 | 對策 |
|------|------|
| A01 Broken Access Control | RBAC + 每個 endpoint 顯式授權檢查 |
| A02 Cryptographic Failures | 全鏈路 TLS、Vault 管金鑰、PII at-rest 加密 |
| A03 Injection | ORM + parameterized query；DB user 最小權限 |
| A04 Insecure Design | 此 SPEC 整體 |
| A05 Security Misconfiguration | IaC + 配置審查 + Drift detection |
| A06 Vulnerable Components | Renovate 自動更新 + SCA scan |
| A07 Auth Failures | TOTP + step-up + 強密碼 |
| A08 Software/Data Integrity | container image signing、SLSA |
| A09 Logging Failures | 呼應 SPEC-007 |
| A10 SSRF | egress proxy + URL allowlist |

### 3.3 密鑰管理

- HashiCorp Vault（自建或 HCP Vault）
- 應用透過 sidecar agent 取 secret，不存於 env / ConfigMap
- 每年輪換敏感金鑰（DB password、JWT signing key、AES at-rest key）
- KYC 證件加密金鑰由獨立 envelope 管理（KEK + DEK）

### 3.4 依賴安全掃描

- CI 必跑：
  - SAST（SonarQube / Semgrep）
  - SCA（Snyk / Trivy）
  - Container image scan
  - IaC scan（Checkov）
- 任何 high / critical CVE → CI fail，阻擋 merge

## 4. 反作弊（Business Layer）

### 4.1 威脅模型

| 威脅 | 動機 |
|------|------|
| 自我成交 / 多帳號 | 套取 LM reward、操縱結算 |
| 洗售（Wash Trading）| 假成交量、誤導他人 |
| 結算操縱（Pump & Dump）| 在事件前累積巨大持倉，臨終結算前推升價格 |
| 內幕交易 | Oracle / Admin 知情提前下注 |
| Sybil 攻擊 | 大量假帳號搶 LM reward / 推薦獎金 |
| 帳號接管 | 偷取憑證後挪用點數 |
| 卡片詐欺 / Chargeback | 用偷來的卡儲值，事後 chargeback |

### 4.2 偵測機制

#### 4.2.1 多帳號連結

每位玩家計算 `user_link_score`：

```
特徵：
- IP 重疊（過去 30 天）
- Device fingerprint 相似度
- 同一信用卡 / 銀行帳號
- 同一 KYC 證件居住地址
- 行為時間相似（凌晨 2:30 同步登入）

權重平均 → 0-100 分
> 80 分 → 自動標記 + Compliance review
```

#### 4.2.2 自我成交偵測

呼應 SPEC-003 的 STP（pre-trade），加上 post-trade 偵測：

- 過去 24 小時，user A 與 user B 若彼此貢獻 > 50% 對方成交量 → 標記
- 三方循環交易（A→B→C→A）偵測

#### 4.2.3 Pump & Dump 偵測

- 結算前 30 分鐘，異常單邊大量買入 → 警告
- 持倉集中度（前 5 名持倉者佔 > 60%）→ 結算後人工 review

#### 4.2.4 Oracle 內幕交易偵測

- 任一 Oracle / Admin / 員工的家屬 / 直屬下級的點數活動，全部留痕
- 員工帳戶白名單，**禁止**在自己負責結算的市場下單
- 違反 → 立即停權 + 民事索賠 + 刑事追訴

### 4.3 行為分析整合

- Sift 或 Sardine：第三方裝置與行為風險評分
- 內建規則引擎：白話規則由 Compliance 配置（不需工程介入）
- ML 模型（V2）：用標註 SAR 樣本訓練

### 4.4 處置流程

| 評分 | 處置 |
|------|------|
| 0–40 | 無 |
| 41–60 | 限制單筆下注上限 |
| 61–80 | 凍結帳戶 + 強制 KYC 升級 |
| 81–100 | 永久封號 + SAR 提交 |

## 5. 帳號接管（ATO）防護

| 防護 | 內容 |
|------|------|
| TOTP 強制 | 註冊時即強制 |
| 異常登入告警 | 新地區 / 新裝置觸發 Email 提醒 |
| 30 天內無交易但突然儲值 → step-up | 強制重新 KYC |
| 關鍵操作通知 | 改密碼 / 改 Email / 自我排除 → 24h 撤回窗口 |
| 暗網密碼洩漏比對 | HaveIBeenPwned 整合，命中強制改密碼 |

## 6. Chargeback 防護

### 6.1 防範

- 高風險 BIN 拒收
- 大額儲值要求 3DS2
- 第一次儲值限額（< USD 100）
- KYC 完成前不允許 > USD 500 儲值

### 6.2 應對

- Chargeback 收到 → 立即凍結對應玩家
- 帳本反沖儲值（呼應 SPEC-002）
- 玩家 24 小時內可上訴
- 累計 chargeback > 1 → 永久封號 + 黑名單

## 7. 資安測試

### 7.1 紅隊演練

每 6 個月一次，由第三方紅隊：

- 應用層滲透測試（OWASP）
- 業務邏輯滲透（嘗試套利、繞過 KYC）
- 社交工程（針對員工）
- 紅隊報告必須有 30 / 90 天修復計畫

### 7.2 Bug Bounty

公開或私募 bounty：

- 平台：HackerOne / Bugcrowd
- 報酬：critical USD 5,000+ / high USD 1,000+ / medium USD 200+
- 範圍：production domain + DMM API
- 排除：DDoS、社工

### 7.3 內部紅隊

- 員工每年一次釣魚測試
- 員工帳號權限季度審查
- DBA 操作日誌每週 review

## 8. 資料外洩應變

### 8.1 偵測

- 異常資料庫存取告警（時段、表、量）
- DLP（Data Loss Prevention）監控對外流量
- Honey token：放幾筆假資料，被查詢即觸發告警

### 8.2 應變

依 GDPR 72 小時通報：

1. 偵測 → 30 分鐘內 P0 告警
2. 1 小時內成立 IR 小組
3. 6 小時內封堵根因
4. 24 小時內初步影響評估
5. 72 小時內監管通報 + 玩家通知（影響重大時）

## 9. 程式碼安全規範

### 9.1 Code Review 必檢清單

- 任何 SQL 語句使用 parameterized query
- 所有外部輸入有 validation
- 所有錢包 / 帳本變動有 idempotency
- 所有對外 URL 走 egress proxy
- 沒有 hard-coded secret
- 沒有日誌中 PII

### 9.2 安全敏感模組需要 4-eye review

- 帳本服務
- 結算服務
- 認證 / 授權
- KYC integration
- DMM API

## 10. 驗收條件

- [ ] Cloudflare WAF 規則上線並通過渗透測試
- [ ] OWASP Top 10 對策皆有對應實作
- [ ] Vault + KMS 配置完成，無 secret 在 env / ConfigMap
- [ ] 多帳號偵測腳本上線
- [ ] STP 即時生效，post-trade 偵測每日 batch
- [ ] Chargeback 流程演練通過
- [ ] 紅隊演練至少完成一次

## 11. Open Questions

1. Sift vs Sardine vs 自建 ML？
2. Bug Bounty 平台選擇與啟動時機？
3. 是否購買 Cyber Liability 保險？
4. 對 Tor 用戶完全封鎖還是僅 challenge？
5. DDoS 大流量壓測由誰執行？
