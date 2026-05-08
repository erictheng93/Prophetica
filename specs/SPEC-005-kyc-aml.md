# SPEC-005 — KYC / AML 整合

> 優先級：**P1**
> 版本：v0.1.4 · 最後更新：2026-05-08
> 依賴：SPEC-001
> 被依賴：SPEC-006, SPEC-010, SPEC-011
> 實作狀態：⚠️ **Backend in-memory contract 完成 / Secret hardening 完成 / Postgres 待 PR #1 後 P0 sprint / Frontend SumSub iframe 待整合**（v0.1.4 resolve §0.2 B3）

---

## 0. Implementation Status

**Status: ⚠️ Backend in-memory contract Complete / Postgres + Secret Hardening P0 Follow-up / Frontend Pending SumSub Integration（2026-05-07 重新校準）**

> **v0.1.4 變更**：§0.2 B3 → ✅ resolved。`kyc-service` 與 `mock-sumsub` 啟動時集中讀取 secret；`PROPHETICA_ENV=production` 且缺少 `SUMSUB_WEBHOOK_SECRET` / `KYC_DOCUMENT_HASH_SALT` 時 fail-fast，不再於 runtime silently fallback 到 dev secret。
>
> **v0.1.3 校準說明**：v0.1.2 標記「✅ Complete」過度樂觀，未區分 backend in-memory contract 驗證、Postgres provider / secret hardening、前端真實 SumSub 整合。round 7 程式碼驗證掃描發現 `apps/web/app/kyc/tier-2/page.tsx:82-95` 之 SumSub 區塊為「虛框 + 按鈕佔位」（未載入真實 iframe）；後續 backend review 確認 API contract + trait 設計合格、migration 完整、test 覆蓋面合格，但 Postgres wiring 與 secret fallback hardening 仍需 PR #1 後 P0 sprint 收斂。因此本 SPEC 狀態校準為「⚠️ Backend in-memory contract 完成 / Postgres + secret hardening 待 PR #1 後 P0 sprint / Frontend Pending」。SPEC 應誠實反映現況（SDD 紀律），便於 Codex / 監管 / 新進工程師準確掌握實作狀態。

### 0.1 已完成（Backend in-memory contract ✅）

SPEC-005 的 in-memory contract 實作已落地於 `kyc-service`、`mock-sumsub`、`packages/ts-sdk`、`apps/web` 與 `tools/scripts/seed.sh`。目前 API contract + trait 設計合格、migration 完整、test 覆蓋面合格，覆蓋 Tier 1 本地證件 hash / duplicate check、Tier 2/3 upgrade、SumSub webhook HMAC + idempotency、sanctions / PEP rescan、AML / SAR candidate、monthly report、前端 KYC status / submission flow，以及對應 SDK 與 mock webhook。此狀態不等同 Postgres provider / secret hardening production-ready。

完成驗證：

- `rtk cargo test -p kyc-service --locked`
- `rtk cargo clippy -p kyc-service --all-targets --locked -- -D warnings`
- `rtk cargo test -p mock-sumsub --locked`
- `rtk cargo clippy -p mock-sumsub --all-targets --locked -- -D warnings`
- `rtk pnpm --filter @prophetica/ts-sdk test`
- `rtk pnpm --filter @prophetica/ts-sdk lint`
- `rtk pnpm --filter @prophetica/web test`
- `rtk pnpm --filter @prophetica/web lint`

### 0.2 Backend P0 follow-up（PR #1 前後分流）

| ID | 項目 | 優先級 / 時機 | 狀態 |
|----|------|---------------|------|
| **B1** | SumSub webhook `RED` 結果必須落到 Frozen / freeze 語意，避免只更新 KYC record 而未凍結帳戶 | **PR #1 merge 前修** | ✅ resolved（kyc.rs:341 RED→Frozen；新增 `sumsub_webhook_red_without_sanction_freezes_user` / `sumsub_webhook_yellow_keeps_user_pending` / `sumsub_webhook_replays_exact_raw_payload_hash_not_applicant_id` 三條 contract test） |
| **B2** | Tier 2 流程順序（upgrade request、provider applicant、webhook、tier promotion 的 canonical ordering） | 待 **SPEC-005a** 先確定 contract 才能修 | 阻塞中，不得先以臨時流程落地 |
| **B3** | Secret fallback hardening：禁止 production 路徑 silently fallback 到 mock / default secret；缺 secret 應 fail-fast | **PR #1 merge 前修** | ✅ resolved |
| **M1** | Postgres provider wiring / repository 實作從 in-memory contract 轉正式 persistence | **PR #1 後 P0 sprint** | Follow-up |
| **M2** | Postgres-backed webhook idempotency 與 duplicate event replay 驗證 | **PR #1 後 P0 sprint** | Follow-up |
| **M3** | Postgres-backed sanctions / PEP rescan 狀態寫入與查詢路徑 | **PR #1 後 P0 sprint** | Follow-up |
| **M4** | AML / SAR candidate persistence 與 monthly report query 收斂 | **PR #1 後 P0 sprint** | Follow-up |
| **M5** | DB transaction 邊界與 cross-table consistency（KYC record、freeze、audit event）驗證 | **PR #1 後 P0 sprint** | Follow-up |
| **M6** | Secret / config 分層測試：dev mock、test fixture、production required secret 三態明確化 | **PR #1 後 P0 sprint** | Follow-up |
| **M7** | Postgres migration metadata 與 SQLx offline artifacts 對齊最終 repository queries | **PR #1 後 P0 sprint** | Follow-up |

### 0.3 待完成（Frontend / 整合 ⏳）

| 項目 | 狀態 | 阻擋 |
|------|------|------|
| **真實 SumSub Web SDK iframe 載入** | ⏳ Tier-2 page 仍為「虛框 + 按鈕佔位」（[`apps/web/app/kyc/tier-2/page.tsx:82-95`](../apps/web/app/kyc/tier-2/page.tsx)）| 需先跑 [PoC-B Cloudflare Edge runtime](../docs/poc/PRE-FLIGHT-POCS.md) §4.2.1 確認 iframe 在 Edge runtime 下載入 |
| **Tier 2/3 OCR / Liveness / EDD** | ⏳ kyc-service 僅 Tier 1 本地驗證實作；Tier 2/3 完整流程需 SumSub iframe + webhook 端到端 | 同上 |
| **Self-exclusion 名單比對** | ⏳ §2.4.1 step 3「對 self-excluded 名單比對」未在 kyc-service 實作 | 待 SPEC-006 §5.4 落地後跨 service 整合 |
| **Daily batch screening rescan scheduler** | ⏳ `/internal/screening/rescans` 為手動 API；§6 「每日 batch」cron 排程未實作 | 動工排程基礎設施（Temporal / Airflow / cron）後接入 |
| **Income stale 標記（12 個月未更新）** | ⏳ §2.1.1 提及但無實作 | 動態 query 計算或加 cron |
| **EdDSA / LIFF 相容性 PoC** | ⏳ T-9 PoC-A 待跑（[PRE-FLIGHT-POCS §3](../docs/poc/PRE-FLIGHT-POCS.md)）| 阻擋 PR #1 §11.3 |
| **真實 SumSub sandbox 帳號 + 合約** | ⏳ 營運啟用項 | 不阻擋工程，但阻擋實際 KYC 流程上線 |
| **監管報表提交管道**（執照地 FIU 整合）| ⏳ 營運啟用項 | 不阻擋工程 |

### 0.4 進入 PR #1 acceptance 之條件

- 不阻擋 PR #1（§0.1 backend in-memory contract 完整；§0.2 中 B1 / B3 已 resolved，B2 待 SPEC-005a，M1-M7 屬 PR #1 後 P0 sprint；§0.3 待完成項目多屬「PR #1 後」工作）
- 但 SPEC-005 §0 不可再標 ✅ Complete，避免誤導 Compliance / 監管面試陳述
- PR #1 merge 後第一輪 sprint 之優先級：跑 PoC-B → 整合真實 SumSub iframe → §0.3 第一列轉 ✅；同步啟動 §0.2 M1-M7 backend P0 follow-up
- 外部供應商合約、真實 SumSub / Onfido sandbox 帳號、監管報表提交管道與 Compliance Officer 實際作業配置屬營運啟用項；不阻塞本 SPEC 之 backend in-memory contract 完成

## 1. Objective

建立分層的客戶身份驗證（KYC）與反洗錢（AML）系統，以最低摩擦、最高合規覆蓋的方式處理玩家入場、儲值、交易、結算過程的身份與風險審查。**不自建 KYC 流程**，採用業界 SaaS 整合節省成本與法律風險。

## 2. KYC 分層（Tiered KYC）

### 2.1 三層設計

| 層級 | 觸發 | 驗證內容 | 月收入申報 | 限制 |
|------|------|---------|-----------|------|
| **Tier 0** 訪客 | 未註冊 / 未儲值 | 無 | — | 僅瀏覽公開市場、看歷史結算 |
| **Tier 1** 基礎 | 完成註冊 + 首次儲值 | Email、手機 OTP、生日（≥18）、首發國對應證件驗證（見 §2.4）| **不申報**（保留 NULL）| 累計儲值 < TWD 30,000 / 月、單筆 < TWD 5,000 |
| **Tier 2** 標準 | 累計儲值達 Tier 1 上限或自願升級 | 證件 OCR + 自拍人臉比對 + Liveness Detection + 地址證明 | **可選申報**（鼓勵以解鎖更高上限） | 累計儲值 < TWD 300,000 / 月（已申報）；TWD 100,000 / 月（未申報）|
| **Tier 3** 強化（EDD）| Tier 2 上限 / 高風險訊號 / 平台主動觸發 | 收入來源證明、銀行對帳單、視訊驗證、PEP/Sanctions 篩查 | **必填**並核對證明文件 | 個案核定（依執照地法規）|

> **Canonical enum identifier**：跨服務（user-service / kyc-service / DB enum / protobuf / seed scripts / 前端）統一使用大寫 underscored 字面 **`TIER_0` / `TIER_1` / `TIER_2` / `TIER_3`**。中文「Tier 0/1/2/3 基礎/標準/強化」僅為 UI 顯示用語。任何 SPEC、ADR、seed、code 不得使用 `LEVEL_*`、`tier1`、`Tier1` 等變體。

### 2.1.1 月收入申報設計（declared_monthly_income）

| 欄位 | 規則 |
|------|------|
| 資料型態 | `BIGINT NULL`（內部單位 = TWD × 100，與點數內部單位一致） |
| 取值範圍 | NULL（未申報）/ 0 / 正整數 |
| 申報來源 | Tier 2 自願填寫（UI 區間選單，例如「< TWD 30k」、「30k-60k」、「60k-100k」、「> 100k」皆對應一個區間中位數）；Tier 3 由 KYC 證明文件抽取 |
| 隱私 | 視同 PII，不入日誌；查詢時客服只看區間，不看精確值 |
| 過期 | 12 個月未更新 → 標記 `stale`，提示玩家重新申報；不影響交易 |
| 違反代價 | Tier 3 申報數值與證明文件不符 → 凍結帳戶 + 人工複審 |

#### Fallback 行為（給 SPEC-006 §4.1 等下游消費者）

```python
def effective_monthly_income(user) -> int | None:
    if user.declared_monthly_income is not None:
        return user.declared_monthly_income
    # 未申報：回傳 None，下游公式必須對 None 採保守 fallback
    return None
```

**呼應 SPEC-006 §4.1 RG 評分公式**：當 `effective_monthly_income(user) is None`，「儲值金額 / 申報月收入」維度視為**中性**（不加分，但若該玩家儲值 > TWD 30,000 / 月直接觸發 Tier 2 升級流程，間接強制申報）。

### 2.2 升級觸發條件

```
玩家行為 → 觸發 Tier 升級需求
  - 累計儲值跨層級閾值
  - 單筆儲值跨層級閾值
  - 異常行為（IP 跳動、多帳號疑慮、裝置指紋共用）
  - 大額結算 payout
  - PEP 命中 / Sanctions 命中
  - 玩家主動升級（解鎖更高儲值上限）
```

升級失敗 → 帳戶**功能凍結**，已有持倉保留至結算，不影響玩家既有資產。

### 2.3 升級條件對齊 §2.2

升級條件涵蓋金額（觸發升級的累計儲值閾值）與行為（觸發 EDD 的可疑活動），詳見 §7.3 EDD 觸發情境與 SPEC-006 §4.2 RG 評分對照行動。

### 2.4 Tier 1 多市場身份證件驗證

> 平台首發市場暫定為台灣（主市場）+ 日 / 韓 / 越 / 馬 / 菲（一般市場），最終名單由 Operating Co. 與執照地律師確認；國家分級與 🔴 紅名單之具體內容由業主治理（注入 Geo-block runtime 配置），不在本 SPEC 維護。Tier 1 不依賴 SaaS 全套 OCR，僅做最低驗證；**不同國家的證件類型與驗證方法不同**，本節定義首發 5 國的對應規則。

| 國家 | 主要證件類型 | 驗證方法 | 隱式驗證可達性 | 備註 |
|------|-------------|---------|--------------|------|
| 🇹🇼 台灣 | 身份證號 (10 碼)| Checksum 驗證 + 出生年月對照（不存原值，僅 SHA256 hash）| ✅ 可單獨完成 | 統一證號末碼校驗公式公開 |
| 🇯🇵 日本 | マイナンバー（個人番号 12 碼）| Checksum 驗證 + 姓名 katakana / kanji 雙寫對照 | ✅ 可單獨完成 | 不收 マイナンバーカード 影像（個資敏感）；改要求姓名 + 生日 + 居住都道府縣 |
| 🇰🇷 韓國 | 주민등록번호（13 碼，前 6 碼出生年月日）| Checksum 驗證 + 出生年月日對照 | ✅ 可單獨完成 | 因法規敏感，本欄位**不入庫**，僅生成 hash 做唯一性檢查 |
| 🇻🇳 越南 | CCCD（公民身份證 12 碼）| Format 驗證 + 出生省份代碼對照 | ⚠️ 部分需 OCR | 紙本式樣多元，舊版 CMND 9 碼亦支援 |
| 🇲🇾 馬來西亞 | NRIC / MyKad（12 碼）| Checksum 驗證 + 出生州代碼對照 | ✅ 可單獨完成 | 中段 6 碼為 YYMMDD，內含年齡資訊可直接 ≥18 驗證 |
| 🇵🇭 菲律賓 | PhilSys ID（PSN 16 碼）/ 駕照 / 護照 | Format 驗證 + 出生年月對照（PSN 採用較晚，多數使用駕照）| ⚠️ 大部分需 OCR | Tier 1 接受護照影像最低品質 OCR，不通過則直接導入 Tier 2 |

#### 2.4.1 Tier 1 隱式驗證流程

```
玩家輸入：姓名、生日、國家、證件號 → 後端驗證流程
1. 國家對應 checksum / format 規則檢查（不通過 → 拒絕）
2. 生日 ≥ 18 歲（不通過 → 拒絕並停權）
3. 證件號計算 SHA256(salt + ID)，與 self-exclusion 名單比對（命中 → 拒絕，呼應 SPEC-006 §5.4）
4. 證件號 hash 與既有 user_kyc_records 比對（命中 → 拒絕重複註冊）
5. 寫入 user_kyc_records（tier=1, document_id_hash, status='passed', expires_at=null）
```

#### 2.4.2 不在首發 5 國的玩家

- 「一般市場」名單外的玩家（紅名單地區）→ 由 [SPEC-001 §2.1](./SPEC-001-legal-compliance.md#21-技術實作層級) Geo-block Layer 1-3 在註冊前阻擋
- 首發 5 國以外、🟢 一般市場以外的中性國家（例如印尼、泰國）→ Tier 1 暫不開放，註冊時提示「您的所在國尚未支援，預計 Phase 2 開放」並導入 waiting list

#### 2.4.3 SaaS 整合對應

- Tier 1：上述驗證**不**送 SumSub（成本考量）；自建純後端規則
- Tier 2：所有國家統一走 SumSub Web SDK；§3.1 流程不變
- Tier 3：SumSub + iProov 視訊 + 所在國居住地址證明文件

## 3. SaaS 供應商整合

### 3.1 推薦組合（依成本與覆蓋）

| 工具 | 用途 | 月費估算 |
|------|------|---------|
| **SumSub** | 主要 KYC（OCR + Liveness + Sanctions + PEP）| US$0.50 / 次比對 |
| **iProov** | 進階 Liveness（Tier 3 視訊驗證） | US$2 / 次 |
| **ComplyAdvantage** | 持續制裁名單篩查（Tier 2/3 monthly rescan）| US$500 / 月 base |
| **Sift / Sardine** | 行為與裝置指紋風險評分 | US$0.005 / event |

> SumSub 涵蓋 90% 需求；其他為補強。**MVP 階段只接 SumSub**，Tier 3 才啟用其他。

### 3.2 整合架構

```
[ 玩家瀏覽器 ]
      ↓ 選擇 KYC 升級
[ prophetica UI ]
      ↓ 顯示 SumSub Web SDK iframe
[ SumSub SDK ]
      ↓ 證件拍攝 + 人臉 + Liveness
[ SumSub Cloud (EU) ]
      ↓ webhook 結果
[ 後端 KYC Service ]
      ↓ 寫入 user_kyc_records
[ 帳戶狀態更新 + 通知玩家 ]
```

### 3.3 Webhook 處理規範

```
POST /api/internal/webhooks/sumsub
Headers: X-Payload-Digest (HMAC-SHA1)

Body 範例:
{
  "applicantId": "...",
  "externalUserId": "user_42",
  "type": "applicantReviewed",
  "reviewResult": {
    "reviewAnswer": "GREEN" | "RED" | "YELLOW"
  },
  "kycReason": "...",
  "sanctionMatches": [...]
}
```

**強制要求**：
- HMAC 驗證失敗直接 401，不處理
- `externalUserId` 必須能對應到內部 user_id
- 處理結果以 `SHA256(raw_payload)` 為冪等鍵，對應 `kyc_webhook_events.raw_payload_sha256 UNIQUE`；`applicantId` 僅用於對應 provider applicant，不可作為 webhook event 去重鍵
- review = `GREEN` 時通過升級；review = `YELLOW` 時維持 `Pending`，等待下一次 webhook / 補件結果
- review = `RED` 時自動凍結帳戶（待人工複審），不立即停權；此要求由 §0.2 B1 追蹤至 PR #1 merge 前落地
- `sanctionMatches` 非空時直接 `Frozen`，覆蓋 `reviewAnswer` 判定

## 4. KYC 資料 schema

```sql
CREATE TABLE user_kyc_records (
    id                  BIGSERIAL PRIMARY KEY,
    user_id             BIGINT NOT NULL,
    tier                SMALLINT NOT NULL,            -- 0/1/2/3
    status              VARCHAR(32) NOT NULL,         -- pending/passed/failed/expired
    provider            VARCHAR(32) NOT NULL,         -- sumsub/iproov/manual
    provider_applicant_id VARCHAR(128),
    review_result       JSONB,
    sanction_match      JSONB,
    pep_match           JSONB,
    document_id_hash    CHAR(64),                     -- 證件號碼 SHA256（不存原值）
    submitted_at        TIMESTAMPTZ NOT NULL,
    reviewed_at         TIMESTAMPTZ,
    expires_at          TIMESTAMPTZ,                  -- 證件有效期，到期需 reverify
    metadata            JSONB NOT NULL DEFAULT '{}'
);
CREATE INDEX idx_kyc_user ON user_kyc_records(user_id, tier);
CREATE INDEX idx_kyc_expires ON user_kyc_records(expires_at) WHERE expires_at IS NOT NULL;
```

## 5. 證件儲存與隱私

### 5.1 儲存策略

- **不**將證件影像存於應用 DB
- SumSub 提供之證件影像 URL 透過後端代理 + 短期 token 取用
- 內部歸檔副本：每日批次下載至 R2（EU region）+ Object Lock，AES-256 at-rest 加密
- 加密金鑰由 Vault 託管，**最小權限存取**（僅合規審計人員）

### 5.2 個資去識別化

- 證件號碼**不**明文存 DB，僅存 SHA256(salt + ID number)
- 比對時：對輸入 ID 計算同樣 hash 後比對 hash
- 鹽值由 Vault 管理，每年輪換（換鹽後舊資料保留供稽核）

## 6. 制裁名單與 PEP 篩查

### 6.1 名單來源

- OFAC Specially Designated Nationals (SDN)
- 聯合國安理會制裁名單
- 歐盟制裁名單
- 英國 OFSI 名單
- 台灣資恐防制法名單
- 各執照地附加名單（Malta MFSA / Curaçao / etc.）

### 6.2 篩查時機

| 時機 | 範圍 | 命中處置 |
|------|------|---------|
| Tier 2 升級時 | 全名單 | 命中 → 拒絕升級 + Compliance 通報 |
| 每日 batch | Tier 2+ 全用戶 | 新命中 → 凍結帳戶 + 人工核實 |
| 大額儲值 / 結算 | 該玩家 | 命中 → 凍結 |

### 6.3 PEP（政治公眾人物）

PEP 不是禁止對象，但需 EDD：

- 自動標記
- 帳戶限制升級至 Tier 3
- 每次大額儲值需 Compliance 預核

## 7. AML 交易監控

### 7.1 規則引擎

平台採 **rule-based + ML scoring** 雙軌：

```
ml_score = sift_event_score + sardine_decision
rule_flags = [
  velocity_check,
  amount_threshold,
  geo_anomaly,
  device_sharing,
  rapid_deposit_then_withdraw,  // 雖然無 withdraw，但有 in-game purchase
  structured_deposits  // 拆單規避閾值
]

if ml_score > 80 OR len(rule_flags) >= 2:
    create SAR_candidate
```

### 7.2 SAR（Suspicious Activity Report）流程

可疑案件由 Compliance Officer 評估：

1. 系統自動產生 SAR draft
2. Compliance 補充訊息、做出判斷
3. 確認可疑 → 提交至執照地金融情報單位（FIU）
4. 玩家通常**不通知**（依當地法規，多數國家禁止 tipping off）
5. 帳戶處置：凍結 / 限制 / 持續監控（依個案）

### 7.3 強制觸發 SAR 的情境

- 單筆儲值 > USD 10,000
- 30 日累計儲值 > USD 30,000
- 不尋常下注模式（極大單筆、極端勝率玩家集中下注）
- 玩家拒絕 KYC 升級且帳戶留有大量點數

## 8. 玩家 UX

### 8.1 KYC 流程設計原則

| 設計準則 | 實作 |
|---------|------|
| 最少摩擦 | Tier 1 只需 OTP + 生日，10 秒完成 |
| 進度可見 | 進度條 + 預估完成時間 |
| 失敗友善 | 失敗原因白話翻譯（不是 SumSub 原始錯誤碼）|
| 異步通知 | KYC review 通常 2-15 分鐘，玩家可離開頁面，完成時 Email + 站內訊息 |
| 隱私說明 | 採集前必有單獨頁面說明「我們收什麼、為什麼、保存多久」 |

### 8.2 失敗重試

- 同一玩家 24 小時內最多 5 次嘗試
- 5 次後鎖定 24 小時
- 連續鎖定 3 次 → 強制人工 review

## 9. 整合驗收測試

### 9.1 Happy Path

- [ ] 新玩家 Tier 0 → Tier 1 註冊流程 < 60 秒
- [ ] Tier 1 → Tier 2 升級（含 KYC SaaS 流程）端到端 < 5 分鐘
- [ ] Tier 2 → Tier 3 升級流程含視訊驗證

### 9.2 失敗情境

- [ ] SumSub webhook HMAC 驗證失敗 → 401
- [ ] webhook 重複送達 → 冪等
- [ ] 制裁名單命中 → 自動凍結 + 推送 Compliance Slack
- [ ] 玩家 KYC 失敗 → 帳戶凍結但持倉保留至結算

### 9.3 邊界

- [ ] 證件過期 → 提前 30 天提醒、過期後限制儲值
- [ ] 玩家修改個資（姓名、生日）→ 強制重新 KYC

## 10. 監管報表

| 報表 | 頻率 | 提交對象 |
|------|------|---------|
| Monthly KYC Volume Report | 每月 | 執照地監管 |
| Quarterly AML Report | 每季 | 執照地監管 |
| Annual Compliance Audit | 每年 | 第三方審計 + 監管 |
| SAR | 即時 | 執照地 FIU |

報表格式由執照地監管要求決定（Malta MFSA / Curaçao / etc.）。

## 11. 驗收條件

- [ ] SumSub 整合通過 Sandbox 測試（模擬 GREEN/RED/PENDING）
- [ ] webhook HMAC + idempotency 驗證測試通過
- [ ] 制裁名單每日 batch 篩查腳本可在 30 分鐘內掃描 1M 用戶
- [ ] KYC 證件影像加密儲存通過 OWASP 檢核
- [ ] PII 去識別化（hash + salt）驗證通過
- [ ] AML 規則引擎可被 Compliance Officer 自助配置（不需工程介入）

## 12. Open Questions

1. SumSub 還是 Onfido？需詢價並比較台灣 / 越南 / 印尼證件支援度
2. 證件影像是否必要保存於台灣境內？（個資跨境傳輸）
3. PEP 名單是否使用 ComplyAdvantage 還是 Refinitiv？
4. SAR 提交是否需自動化整合 FIU 系統？（多數需手動）
5. 視訊驗證是否強制錄影留存？保存期？
