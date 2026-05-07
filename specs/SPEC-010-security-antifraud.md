# SPEC-010 — 安全與反作弊（Security & Anti-Fraud）

> 優先級：**P1**
> 版本：v0.2.2 · 最後更新：2026-05-07
> 依賴：SPEC-001, SPEC-005
> 被依賴：SPEC-009, SPEC-002, SPEC-004, SPEC-008, SPEC-012, SPEC-013, SPEC-015, SPEC-016, SPEC-017

> **v0.2.2 變更**：§3.1.2 PoC 表補 cross-ref 至 [`docs/poc/PRE-FLIGHT-POCS.md §3`](../docs/poc/PRE-FLIGHT-POCS.md)（EdDSA / LIFF SDK 相容性 PoC 詳細 plan）。
>
> **v0.2.1 變更**：(1) §3.1.2 補 EdDSA mobile / LIFF SDK 相容性 PoC 要求（T-9），動工前需驗證 LIFF SDK 對 Ed25519 token 之支援；不支援則對 LIFF route 回退 RS256 token；(2) §3.2 OWASP A03 Injection 對策由「ORM + parameterized query」修正為「`sqlx::query!` 編譯時檢查 SQL + parameterized binding」（H-4），對齊 SPEC-014 §3.1「不引入 ORM」決議；(3) §6.2 chargeback 永久封號門檻由「> 1」修正為「**≥ 1**」（H-5），與 SPEC-016 §6.1 / SPEC-002 §8.1 統一；(4) §6.2 帳本反沖 cross-ref 補至 SPEC-002 §8.2。
> **v0.2.0 變更**：新增 §9.3 Multi-sig 治理中央表（M-3）。集中所有跨 SPEC 之 multi-sig governance（MS-1 ~ MS-17）+ 已 Resolved 不入表決策（hedge / 計價 / 數據主權）+ Admin UI 實作要求 + 為何集中。原 SPEC-002 §4.2.1 / §8.1、SPEC-004 §3 §6.1、SPEC-008 §9.3、SPEC-012 §11、SPEC-013 §3.1 §6、SPEC-015 §7.2 §8.3 §9.2、SPEC-017 §3.3、SPEC-016 §13 之簽核要求皆改以「呼應 SPEC-010 §9.3 MS-N」方式引用本表。被依賴 SPEC 列表由原本 SPEC-009 擴增為 9 份，反映本檔成為 governance 中央表。

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

> **Client 相容性 PoC（T-9，動工前驗證）**：Ed25519 簽章 / 驗證性能優於 RS256 ~10x，但部分 mobile / browser JWT lib 預設僅支援 RS256 / HS256。動工前必須完成以下 PoC（**詳細 PoC plan 見 [PRE-FLIGHT-POCS §3](../docs/poc/PRE-FLIGHT-POCS.md)**）：
>
> | Client | 驗證項目 | 後備 |
> |--------|---------|------|
> | LIFF SDK（LINE）| `liff.getIDToken()` 之 JWT 驗證鏈是否支援 EdDSA | 若不支援 → server 端為 LIFF route 改發 RS256 token，cookie 命名加 `_liff` 前綴區分 |
> | iOS / Android（若未來開發 native app）| 主流 JWT lib（如 `jose`, `JOSESwift`）EdDSA 支援度 | 同上回退 RS256 |
> | Browser（apps/web Next.js + axios / fetch）| 不需 client-side 驗證（cookie HTTP-only，Next.js middleware 走 server-side `jsonwebtoken` 0.9+）| 無需後備 |
>
> PoC 結果若任一回退，需開新 ADR（雙演算法 token 簽發策略）並更新本節。

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
| A03 Injection | **`sqlx::query!` 編譯時檢查 SQL**（呼應 SPEC-014 §3.1 不引入 ORM）+ parameterized binding；DB user 最小權限（schema-per-service，呼應 SPEC-014 §7.2） |
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
- 帳本反沖儲值（呼應 SPEC-002 §8.2）
- 玩家 24 小時內可上訴
- 累計 chargeback **≥ 1** → 永久封號 + 黑名單（H-5 跨檔對齊；呼應 SPEC-016 §6.1 / SPEC-002 §8）

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

### 9.3 Multi-sig 治理中央表（Single Source of Truth）

> 為避免 SPEC-002 / 004 / 008 / 012 / 013 / 015 / 017 各自詮釋「4-eye」/「2-of-N」/「3-of-3」造成 admin / Compliance UI 流程分裂、監管稽核時無法給出單一答覆，本表為平台**所有**跨 SPEC multi-sig governance 的單一真相來源。
>
> 任何新增規則必須以 PR 形式入本表，並在原 SPEC 以「呼應 SPEC-010 §9.3 MS-N」方式引用；解除任一規則需先 PR 修改本表並在 ADR 中 superseded 對應原 SPEC 章節。

#### 9.3.1 規則總表

| ID | 場景 / 動作 | 觸發者 | 簽核要求 | 來源 SPEC |
|----|------------|-------|---------|----------|
| MS-1 | 市場上架審查（DRAFT → SCHEDULED）| Market Operator | **2-of-2** Compliance Reviewer + Legal Reviewer（獨立寫入 `market_reviews`，不可同人雙簽） | SPEC-015 §7.2 |
| MS-2 | 市場排程變更（SCHEDULED 階段 OpensAt / ClosesAt）| Schedule Manager | **2-of-2** Schedule Manager + Compliance（4-eye）| SPEC-015 §8.3 |
| MS-3 | 強制下架（任一階段 → CANCELLED）| Compliance Officer | **3-of-3** Compliance Officer + Tech Lead + Legal | SPEC-015 §9.2 |
| MS-4 | Oracle 提交（CLOSED → PENDING）| Oracle Operator | **2-of-N** 獨立提交且 `outcome_result` 一致；不一致自動進 DISPUTED | SPEC-004 §3 / §4.2 / SPEC-015 §6.3 |
| MS-5 | 申訴裁決（DISPUTED → SETTLING / CANCELLED）| Adjudicator | **3 名合議**：3:0 / 2:1 通過；1:1:1 三派分立升級至 Compliance Officer 拍板 | SPEC-004 §6.1 / SPEC-015 §6.5 |
| MS-6 | 結算 reversal / 帳本 compensation transaction | T3 客服 | **4-eye** T3 + Compliance；寫入 `Txn type = 'cs_compensation'` | SPEC-002 §8.1 / SPEC-012 §11 |
| MS-7 | Frozen 解凍（Compliance / EDD pending → Wallet）| Compliance | **4-eye** Compliance + RG Officer（呼應 SPEC-002 §4.2.1 metadata.approved_by）| SPEC-002 §4.2.1 |
| MS-8 | 自我排除期滿解凍（自動）| RG Scheduler | 系統觸發；`approved_by = 'system:rg-scheduler'`（不需人工簽核但全程留痕）| SPEC-002 §4.2.1 / SPEC-006 §5 |
| MS-9 | Kill switch 解除（停盤 → 恢復）| Senior Engineer | **2-of-3** CTO / Compliance / Senior Engineer | SPEC-008 §9.3 |
| MS-10 | 安全敏感模組 PR merge（ledger / 撮合 / 結算 / 認證 / KYC / DMM）| PR Author | **4-eye** 兩名 reviewer 簽核（含一名 module owner）| SPEC-010 §9.2 |
| MS-11 | unsafe block 新增（R-5 例外）| PR Author | **4-eye** + 對應 ADR + cargo geiger CI gate review-required label | SPEC-017 §3.3 R-5 |
| MS-12 | Production release（prod deploy）| Release Manager | **3-of-5** release-managers team approval | SPEC-013 §3.1 |
| MS-13 | Hotfix 直 prod canary（reduced gate）| Senior Engineer | Senior Engineer 簽核 + 事後 RCA 24h 內提交 | SPEC-013 §6 |
| MS-14 | 新增 `PLATFORM_SYSTEM_USER_IDS` sentinel（撮合引擎禁字）| Tech Lead | **4-eye** + 對應 ADR + Compliance Officer review；CI sentinel-scan 同步更新 whitelist | SPEC-003 §12.1 / ADR-0001 §6 |
| MS-15 | DMM 簽約 / 額度提升 | DMM 招募負責人 | **2-of-2** Compliance + Risk；額度提升另需 Tech Lead 對 in-flight position 風險意見 | SPEC-009 §3.6 / §3.7 |
| MS-16 | 退款 API 內部呼叫（Chargeback / 系統故障）| T3 客服 | **4-eye** T3 + Compliance | SPEC-016 §13 |
| MS-17 | 跨檔決策 Resolved（SPEC-000 §5）| Tech Lead 提案 | 業主 sign-off 1 名（影響合規時加 Legal 1 名）| SPEC-000 §5 維護規則 |

#### 9.3.2 已 Resolved governance 決策（不入 multi-sig 表）

下列決策已於 SPEC-000 §5 拍板，不需每次簽核但記於此供 audit：

| 決策 | 內容 | 來源 |
|------|------|------|
| Hedge 政策 | **允許**同帳戶同市場同時持 YES + NO；風控前置層**不**加 `HEDGE_NOT_ALLOWED` 檢查；自我對沖刷量由 §4.2.2 post-trade 偵測 | [SPEC-000 §5 Q-7](./SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker) |
| 點數計價單位 | 1 點 = TWD 1（永遠單幣別 TWD） | SPEC-000 §5 Q-3 |
| 數據主權 | KYC 證件 / WORM hash / 影像類資產一律 Cloudflare R2（EU bucket + Object Lock） | SPEC-000 §5 Q-6 |

#### 9.3.3 Admin / Compliance UI 實作要求

- 後台簽核 UI 必須**單一 component** 對應本表所有 MS 規則（schema 由本表驅動），**不允許**各功能各寫一套流程；新增 MS 規則 = 補本表 + 補 schema，**不**改 component 程式碼路徑。
- 簽核失敗（單簽嘗試、自簽自核、利益衝突命中、權限不足）的拒絕行為由 component 集中處理，回 `errors.proto: ERROR_CODE_MULTISIG_REJECTED` 並附 MS-ID。
- 所有 multi-sig 動作必須寫入 `audit_logs`（呼應 SPEC-012 §5.3），`metadata` 包含：
  ```jsonc
  {
    "ms_id": "MS-3",
    "action": "force_delist_market",
    "subject_id": "mkt_2028_us_election",
    "signers": [{"user_id": 12, "role": "compliance_officer", "ts": "..."},
                {"user_id": 34, "role": "tech_lead", "ts": "..."},
                {"user_id": 56, "role": "legal", "ts": "..."}],
    "decision": "approve",
    "reason": "..."
  }
  ```
- 簽核者**強制 hardware MFA**（YubiKey / Passkey）—— 對齊 §3.1.5 step-up MFA 機制；MS-3 / MS-9 / MS-12 等高敏作業另要求 step-up 過去 30 分鐘內仍有效。
- Compliance 後台首頁顯示「待簽核 multi-sig 工單」清單，依 MS-ID 分類，超過各自 SLA（MS-3 / MS-9 < 1h，MS-1 / MS-2 < 24h，其餘 < 4h）觸發告警。

#### 9.3.4 為何集中

- **避免 admin UI 分裂**：原本 SPEC-002 / 004 / 008 / 012 / 013 / 015 / 017 各自詮釋「4-eye」/「3-of-3」造成 12 個 service × 各自一套 UI；集中後一份 component 對應全部。
- **監管稽核**：監管問詢「平台 multi-sig 治理為何」時提供單一答覆來源，不需翻閱七份 SPEC。
- **新進 Compliance Officer onboarding**：本表為治理 SOP 入口；對應每條 MS 配套之操作手冊由 Compliance 團隊維護於 `infra/runbooks/multisig-{ms-id}.md`。
- **演化追蹤**：簽核規則調整（例如未來 Compliance Officer 角色拆分為 AML Officer / RG Officer）只需改本表 + ADR，不需散彈式跨 SPEC patch。

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
