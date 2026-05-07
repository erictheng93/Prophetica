# SPEC-016 — 支付通路整合（Payment Gateway Integration）

> 優先級：**P0（必須在第一行 production code 之前完成）**
> 版本：v0.1.1 · 最後更新：2026-05-07
> 依賴：SPEC-001, SPEC-002, SPEC-005, SPEC-010
> 被依賴：SPEC-007, SPEC-008, SPEC-013

> **v0.1.1 變更（H-5）**：§6.1 chargeback 永久封號門檻明示為「**≥ 1**」並補 cross-ref 至 SPEC-010 §6.2 / SPEC-002 §8.1，三檔統一門檻；H-5 跨檔對齊。

---

## 1. Objective

定義玩家儲值（fiat → 站內點數）的端到端支付通路：通路選型、收款主體、webhook 簽章、退款 / chargeback、跨幣別匯率鎖定、PCI 範圍劃線、多通路備援、結帳 UX。本 SPEC 補齊 SPEC-002（資金安全 / 帳本）與 SPEC-001（執照地金流）之間的「實際支付路徑」議題。

## 2. 範圍與不在範圍

### 2.1 In-scope

- 支付通路選型與分級（Tier-1 / Tier-2 / 備援）
- 收款主體與託管帳戶對應
- Webhook 簽章驗證、idempotency、retry 規範
- Chargeback / 退款流程
- 跨幣別匯率鎖定機制
- PCI DSS 範圍劃線
- 多通路備援切換策略
- 結帳 UX 標準（含 RG 警示、上限提示）

### 2.2 Out-of-scope

- 帳本記錄細節（SPEC-002 §4）
- KYC 驗證（SPEC-005）
- 金流相關反作弊（SPEC-010 §6）
- 出金（**本平台無出金通道**，呼應 SPEC-001 §1.1）

## 3. 支付通路選型

### 3.1 通路分級

| 級別 | 通路 | 主要市場 | 收款主體 | 啟用優先序 |
|------|------|---------|---------|-----------|
| **Tier-1** | 綠界 ECPay | 台灣（信用卡 / ATM 轉帳）| **執照地 Op Co.** 之台灣分行帳戶 | MVP Day 1 |
| **Tier-1** | Stripe | 國際（信用卡 / Apple Pay / Google Pay）| Op Co. 在執照地 Stripe 帳戶 | MVP Day 1 |
| **Tier-2** | 藍新 NewebPay | 台灣備援 | 同綠界 | MVP + 30 天 |
| **Tier-2** | PayMongo | 菲律賓本地 | Op Co. 信託帳戶 | Phase 2 |
| **Tier-2** | Momo / VNPay | 越南 | Op Co. 信託帳戶 | Phase 2 |
| **備援** | Wise / Airwallex（B2B 跨境）| 多國法幣 | Op Co. | 視情況啟用 |

### 3.2 通路接受度核實（動工前驗證項目）

> 對應 [SPEC-000 §5 Q-4](./SPEC-000-index.md)。在簽通路合約前必須驗證：

| 議題 | 驗證對象 |
|------|---------|
| 通路是否接受「娛樂遊戲」MCC（7995 / 7993 / 7800）？ | 各通路業務窗口 |
| 收款主體可否為境外 Op Co.？ | 通路合規部門 |
| 通路是否在合約中限制 prediction market 業務？ | 法律意見 |
| 退款 / chargeback rate 上限與懲罰條款 | 合約細則 |

> ⚠️ 若 Tier-1 通路之任一在簽約階段拒絕，**不得**降級為 Tier-2 帶風險上線；改尋找替代通路或調整營運計畫。

## 4. 收款主體與託管帳戶對應

### 4.1 收款邏輯

```
玩家 ─法幣─→ Payment Gateway ─結算─→ Op Co. 託管銀行帳戶（Segregated Trust）
                                                      │
                                                      ▼
                                          Ledger (SPEC-002 §4) 雙簿記入帳
                                                      │
                                                      ▼
                                          Player.Wallet 記入點數
```

**禁區**（呼應 ADR-0002 §8）：
- ❌ Tech Service Co. 名下之台灣銀行帳戶**禁止**接收任何玩家儲值
- ❌ Holdings 帳戶**禁止**接收玩家儲值
- ❌ 創辦人 / 員工**個人帳戶**永久禁止

### 4.2 結算頻率

| 通路 | 結算頻率 | T+N |
|------|---------|-----|
| 綠界 | 每日 | T+3 至 T+7 |
| Stripe | 每 2 日 | T+2 至 T+4 |
| 藍新 | 每日 | T+5 |

**重要**：通路結算前該款項屬於通路代收，不算入 `Bank.TrustAccount` 餘額。Ledger 記帳順序：
1. webhook 確認 → `Pending.Deposit` 帳戶（asset）+ `Player.Wallet` 同步（liability）
2. 通路結算入帳 → `Pending.Deposit` 轉至 `Bank.TrustAccount.{currency}`
3. 兩階段都需 idempotent

> SPEC-002 §4.2 之 Chart of Accounts 需擴增 `Pending.Deposit.{provider}` 子帳戶。

## 5. Webhook 處理

### 5.1 必含驗證

| 項目 | 規則 |
|------|------|
| HMAC 簽章 | 各通路規範各異；**簽章驗證失敗直接 401，禁止記日誌** |
| Replay 防護 | Webhook 必含 timestamp，視窗 5 分鐘外的事件拒收 |
| Idempotency | 以通路 transaction_id 為 key，呼應 SPEC-002 §6.1 |
| TLS | 接收 endpoint 強制 TLS 1.3 |
| IP 白名單 | 各通路公告之 IP range；命中之外 → drop |

### 5.2 Endpoint 規範

```
POST /api/internal/webhooks/payment/{provider}
Headers:
  X-Provider-Signature: <provider HMAC>
  X-Provider-Timestamp: <unix sec>
  X-Idempotency-Key: <provider txn_id>

Body: 通路原生 JSON / form-data
```

回應：
- `200 OK`：處理成功（含「已處理過」case）
- `4xx`：簽章 / 格式錯誤；通路會 retry，需審查
- `5xx`：系統錯誤；通路 retry

### 5.3 Retry 規範

通路 retry 行為各異，後端必須假設：
- **同一 transaction_id 至少送達 1 次，最多送達 N 次**
- N 次 retry 都應產生**同一筆** ledger transaction（idempotent）
- 通路規範之最終 retry 截止前未回 200 → 通路標記為「pending」並由運維介入

### 5.4 失敗對帳

每日 T+1：
- 對比平台 ledger 之該日 `deposit` transaction 數量 vs 通路後台對帳檔
- 差異 > 0 自動寫入 `Suspense.Reconciliation`（SPEC-002 §7.2）+ P0 告警

### 5.5 通路 Webhook Payload 對映表

> 各通路 webhook payload schema 不一致。本節定義從**通路原生格式**到後端統一 `ProviderWebhookEnvelope`（見 [`packages/proto/v1/payment.proto`](../packages/proto/v1/payment.proto)）的對映規則。每個通路在 `services/payment-service/internal/providers/{provider}/` 內實作 driver-specific verifier。

#### 5.5.1 共用流程

```
provider webhook (HTTPS POST)
  │
  ▼
1. Cloudflare WAF + IP allowlist（drop 非通路 IP 段）
2. Provider-specific HMAC verifier（簽章演算法 / header 名各異）
3. Replay window 檢查（payload timestamp ± 5 min）
4. Driver decode：通路原生 JSON → ProviderWebhookEnvelope
5. Idempotency lookup（key = provider + provider_transaction_id）
6. 進主流程：依事件類型呼叫 ledger PostTransaction（deposit / refund / chargeback）
```

#### 5.5.2 通路對映表

| Provider | HMAC header | HMAC algo | Timestamp 來源 | Provider txn ID 欄位 | settled_amount 欄位 | Currency 欄位 | 事件類型映射 |
|----------|------------|-----------|---------------|-------------------|-------------------|--------------|-------------|
| **ECPay 綠界** | `CheckMacValue`（form field，非 header）| MD5 / SHA256（依設定）| `PaymentDate` | `MerchantTradeNo` | `TradeAmt`（TWD 整數元）× 100 | 固定 TWD | `RtnCode=1` → SETTLED；`RtnCode≠1` → FAILED |
| **Stripe** | `Stripe-Signature` | HMAC-SHA256 | header `t=` 部分 | `id`（event）+ `data.object.id`（charge）| `data.object.amount_received`（已是最小單位）| `data.object.currency`（lower case ISO）| `payment_intent.succeeded` → SETTLED；`charge.refunded` → REFUNDED；`charge.dispute.created` → CHARGEBACK |
| **NewebPay 藍新** | `TradeSha`（form field）| SHA256 + AES | `PayTime` | `MerchantOrderNo` | `Amt`（TWD 元）× 100 | 固定 TWD | `Status=SUCCESS` → SETTLED |
| **PayMongo** | `Paymongo-Signature` | HMAC-SHA256（含 `t=` / `te=` / `li=`）| header `t=` | `data.id`（event）| `data.attributes.amount` | `data.attributes.currency` | `payment.paid` → SETTLED；`payment.refunded` → REFUNDED |
| **MoMo VN** | `Signature`（query param）| HMAC-SHA256 + RSA | `responseTime` | `requestId` + `transId` | `amount`（VND 整數元）× 100 | 固定 VND | `resultCode=0` → SETTLED |
| **VNPay** | `vnp_SecureHash` | HMAC-SHA512 | `vnp_PayDate` | `vnp_TxnRef` | `vnp_Amount`（VND × 100，已最小單位）| 固定 VND | `vnp_ResponseCode=00` → SETTLED |

> 表中「× 100」表示 driver 在 decode 時必須換算為**內部單位**（SPEC-002 §3.2）。對於通路本身已用最小單位（如 Stripe / VNPay）則直接帶入。

#### 5.5.3 Phase 1 換算規則

> SPEC-016 §7.1 規定 Phase 1 ledger 僅有 `Bank.TrustAccount.TWD`。對非 TWD 通路（PayMongo / MoMo / VNPay 等）：

- Driver decode 時取通路提供的 `fx_rate`（若無則拒絕，要求業主與通路重簽合約包含 FX 透明欄位）
- `settled_twd = original.amount × fx_rate`，存入 `ProviderWebhookEnvelope.settled_twd`
- 主流程僅 deposit `settled_twd` 到 `Bank.TrustAccount.TWD`，原幣別僅作 audit 紀錄
- `fx_rate` 字串保留小數精度（如 `"31.5"`），不轉 float

#### 5.5.4 Driver 必含介面（Rust）

```rust
// services/payment-service/src/providers/mod.rs

use async_trait::async_trait;
use axum::http::Request;
use axum::body::Bytes;
use crate::proto::proph::v1::ProviderWebhookEnvelope;

#[async_trait]
pub trait Driver: Send + Sync {
    /// 驗證 HMAC、replay window、IP 白名單，並 decode 為統一 envelope。
    /// 失敗回傳 ErrorCode（呼應 errors.proto）。
    async fn verify_and_decode(&self, req: Request<Bytes>)
        -> Result<ProviderWebhookEnvelope, ProviderError>;

    /// 創建支付意圖（玩家發起儲值入口）。
    async fn create_intent(&self, input: CreateIntentInput)
        -> Result<CreateIntentOutput, ProviderError>;

    /// 對通路發出退款。
    async fn issue_refund(&self, input: IssueRefundInput)
        -> Result<IssueRefundOutput, ProviderError>;

    /// 通路健康檢查（用於 §9 多通路備援）。
    async fn health(&self) -> HealthStatus;
}
```

#### 5.5.5 對映表維護規則

- 新增通路必須先在本表新增一列、實作 Driver、補 unit test 含 happy / replay / HMAC fail 三案例。
- 通路若改變 webhook 格式，driver 需 bump version，driver registry 同時支援新舊版至少 30 天。
- 任何「通路特殊處理」（如綠界 form-encoded vs JSON）都被 driver 層吸收，主流程**只看** `ProviderWebhookEnvelope`。

## 6. Chargeback 與退款

### 6.1 Chargeback 流程

```
通路通知 chargeback ─→ 平台凍結對應玩家
                          │
                          ▼
              ledger 反沖：Bank.TrustAccount -X / Player.Wallet +X
              （allowed 短暫負餘額 — 呼應 SPEC-002 §8.2）
                          │
                          ├─→ 玩家 24h 內可上訴（提供證據）
                          └─→ 累計 chargeback **≥ 1** → 永久封號 + 黑名單
                              （呼應 SPEC-010 §6.2 統一門檻；H-5 跨檔對齊）
```

### 6.2 退款流程（玩家主動 / 平台主動）

| 觸發 | 處置 |
|------|------|
| 玩家主動退款（未交易、KYC 失敗）| 退款至原入金通道；點數 reverse |
| 系統故障導致重複扣款 | 自動偵測 + 退款；無需人工 |
| 市場創建錯誤（呼應 SPEC-004 §8.3）| 平台自願退手續費 + 退款 |
| 玩家身份證件造假被偵測 | 退款至原通道（不論 chargeback）|

### 6.3 退款限制

- 退款**只能原路返回**（信用卡退原卡、ATM 退原帳戶）
- 部分退款最低粒度 = 1 點 = TWD 1（依計價單位）
- 退款金額不得超過原始儲值
- 退款超過 90 天則拒絕（通路普遍時限）

## 7. 跨幣別與匯率

> **Phase 1 — 永遠單幣別 TWD**（呼應 [SPEC-002 §3.1](./SPEC-002-funds-safety.md) 已 Resolved 之 1 點 = TWD 1）。ledger 僅維持 `Bank.TrustAccount.TWD` 一個資產帳戶；國際玩家以非 TWD 法幣儲值時，**由通路在 webhook 中換算為 TWD 後入帳**，平台 ledger 永遠以 TWD 計價，**任何幣別皆於通路端完成換算**。
>
> Phase 2（多幣別）保留設計但**目前無啟用計畫**。Q-3 已 Resolved 為 TWD 1:1，Phase 2 不再被 Q-3 阻塞；觸發條件改為**業務驅動**：(a) Op Co. 開立非 TWD 計價之國際版本（例如 USD ledger）、或 (b) 平台需主動承擔 FX 對沖（例如向 DMM 結算非 TWD 點數）。在無上述業務需求前，§7.4 之多幣別 schema 不實作。

### 7.1 Phase 1 — 單幣別（TWD）入帳路徑

```
玩家以 USD 100 儲值
  │
  ▼
通路在結算時用通路匯率 USD/TWD = 31.5 換算為 TWD 3,150
  │
  ▼ webhook payload:
  {
    settled_amount_twd: 3150,
    original_amount: 100, original_currency: "USD",
    fx_rate: 31.5, fx_locked_at: "..."
  }
  │
  ▼
Ledger（與 SPEC-002 §4.4.1 同型）:
  Entry 1: Bank.TrustAccount.TWD  +315,000  (3,150 TWD × 100 內部單位)
  Entry 2: Player.Wallet.X        -315,000

  Σ entries = 0 ✓
```

#### 7.1.1 Webhook 必含欄位（FX 透明化）

不論玩家原儲值幣別，後端必須從 webhook 取得：

| 欄位 | 用途 |
|------|------|
| `settled_amount_twd` | 入 ledger 的金額（內部單位 × 100）|
| `original_amount` / `original_currency` | 對玩家顯示「您儲值 100 USD ≈ 3,150 TWD」|
| `fx_rate` / `fx_locked_at` | 稽核留痕（存入 `ledger_transactions.metadata`）|
| `provider_fee_twd` | 通路手續費（換算成 TWD）|

### 7.2 平台 FX 風險

Phase 1 平台不承擔 FX 風險（換算發生於通路端）。通路收取的 FX spread 視為玩家成本。

### 7.3 玩家 UI 對 FX 的呈現

- 結帳前顯示原幣別金額 + 預估 TWD 點數，並標注「最終點數依通路結算匯率為準，可能微幅變動」
- 入帳完成後，玩家錢包與儲值紀錄統一以 TWD 點數呈現
- 退款比照 §6.3 「原路返回」原則，TWD 對原幣金額之精算差額由通路端處理

### 7.4 Phase 2（保留設計，目前無啟用計畫）— 多幣別 ledger

> 以下為延伸設計，**目前無啟用計畫**。觸發條件為業務驅動（見 §7 開頭）：Op Co. 開非 TWD 計價國際版本、或平台主動承擔 FX 對沖。Phase 1 不實作此 schema，僅保留設計以避免未來大規模 migration。

```
資產：Bank.TrustAccount.{TWD,USD,JPY,VND,...}     ← 多幣別託管
費用：Expense.FXLoss                              ← 月度結轉
收入：Revenue.FXGain
特殊：FX.Adjustment                                ← 每筆 cross-currency 交易暫掛差額
```

匯率鎖定原則：
- 匯率以通路結算時點（webhook 提供）為準
- 平台不主動 hedge，FX 風險由 Op Co. 承擔
- 每月將 `FX.Adjustment` 結轉至 `Revenue.FXGain` / `Expense.FXLoss`

## 8. PCI DSS 範圍劃線

### 8.1 PCI 範圍最小化

> 平台**永遠不直接接觸卡片號碼**。所有卡資料透過通路 SDK / 跳轉至通路頁面收集。

| 元件 | PCI 範圍 |
|------|---------|
| Web 前端 | **out-of-scope**（卡資料以 iframe / redirect 至通路頁面收集）|
| API Gateway | out-of-scope |
| Backend 服務 | out-of-scope |
| Webhook receiver | out-of-scope（通路只送 token / last 4 digits）|

### 8.2 不可儲存的資料

- ❌ 完整 PAN（卡號）
- ❌ CVV / CVC（任何時刻）
- ❌ Track 1/2 magstripe data
- ❌ PIN

### 8.3 可儲存（受限）

- ✅ Token（通路產生）
- ✅ 卡號末 4 碼（顯示用）
- ✅ Issuer / 國家 / 卡別（風控用）
- ✅ Holder name（受限，僅 Tier 2/3 KYC 留存）

### 8.4 自評每年一次

- SAQ A 等級即可（純 redirect / iframe 模型）
- 由通路提供 Attestation of Compliance (AoC)
- Compliance 每年更新自評

## 9. 多通路備援與自動切換

### 9.1 健康檢查

- 每 60 秒對每個啟用通路的 health endpoint ping
- 連續 3 次失敗 → 標記 `degraded`
- 連續 10 次失敗 → 標記 `down`

### 9.2 切換邏輯

```
玩家發起儲值
  │
  ▼
路由器選擇通路（優先序：Tier-1 healthy > Tier-2 healthy > 備援）
  │
  ├─ Tier-1 全部 down → 顯示「儲值暫時不可用，預計 X 分鐘恢復」
  └─ 至少一個 healthy → 提供給玩家
```

### 9.3 玩家偏好

- UI 提供通路選擇（卡 / ATM / 其他）
- 玩家偏好不超越「healthy 通路」之約束

## 10. 結帳 UX 必含元素

呼應 SPEC-006 責任博弈、SPEC-001 條款：

### 10.1 儲值頁面

| 元素 | 規則 |
|------|------|
| 月度上限進度條 | 顯示「本月已儲值 / 月上限」（呼應 SPEC-006 §7.2）|
| 接近上限警示 | ≥ 80% 顯著紅字提醒；100% 拒絕儲值 |
| 點數無回兌聲明 | 結帳前最後一頁明示「此次儲值之點數無法兌回現金」並玩家勾選確認 |
| KYC tier 限額提示 | 若玩家此次儲值會跨 Tier 上限，提前提示需升級 |
| RG 援助連結 | 頁尾持續可見 |

### 10.2 結帳前最終確認

```
你即將以 [TWD 1,000] 儲值 [1,000 點]

✓ 我了解此次儲值的點數**不可兌回現金、虛擬貨幣、實體商品**
✓ 我了解平台僅提供撮合服務，並非交易對手
✓ 我了解平台收取交易手續費，但不分享我的輸贏

[ 取消 ]   [ 確認儲值 ]
```

未勾選兩個 checkbox 不可前進。

### 10.3 儲值完成後

- 顯示「點數已入帳」
- 提示玩家設定 / 調整月度上限（如尚未設定）
- 連結至「我的責任博弈」頁面

## 11. 監控指標（呼應 SPEC-007 §3）

```
proph_payment_gateway_request_seconds_bucket  histogram, labels: provider, status
proph_payment_gateway_webhook_received_total  counter, labels: provider, type
proph_payment_gateway_webhook_invalid_signature_total counter, labels: provider
proph_payment_gateway_settlement_lag_seconds gauge, labels: provider
proph_payment_chargeback_total                counter, labels: provider, reason
proph_payment_refund_total                    counter, labels: provider, reason
proph_payment_provider_health                 gauge, labels: provider (1=healthy, 0=down)
proph_payment_fx_loss_pts_total               counter, labels: source_currency
```

## 12. 性能與可靠性 SLO

| 指標 | 目標 |
|------|------|
| 儲值頁載入 P99 | < 2 秒 |
| Webhook 接收 → ledger commit P99 | < 30 秒 |
| 同一 transaction_id 重送 100 次 → ledger 只一筆 | 100% |
| 通路切換（degraded → 備援啟用） | < 60 秒 |
| 對帳差異率 | < 0.001% / 月 |

## 13. 安全要求

- Webhook endpoint 走 Cloudflare WAF + IP 白名單
- 通路 API 金鑰存於 Vault，依各通路最小權限
- 退款 API 內部呼叫需 4-eye 簽核（T3 客服 + Compliance）
- 對帳差異 > USD 1,000 自動凍結出帳

## 14. 驗收條件

- [ ] Tier-1（綠界 + Stripe）通路接入並通過沙盒測試
- [ ] HMAC 驗證 + idempotency 端到端測試（重送 100 次只 commit 1 筆）
- [ ] FX 帳戶體系與 Ledger schema 整合測試
- [ ] Chargeback 流程演練：webhook → 凍結 → 反沖 → 上訴 → 結案
- [ ] 多通路備援切換演練：模擬 Tier-1 全部 down，自動切至 Tier-2
- [ ] PCI SAQ A 自評通過
- [ ] 結帳 UX 之 §10.2 同意 checkbox 通過 E2E 測試（未勾不可前進）
- [ ] 對帳腳本通過 1M 筆儲值資料對比 < 5 分鐘

## 15. Open Questions

> 跨檔決策已收斂至 [SPEC-000 §5](./SPEC-000-index.md) 中央追蹤表。本節僅保留 SPEC-016 自身範圍內議題。

1. 通路 chargeback rate 是否須在合約中爭取 < 0.5%（業界標準）？
2. 玩家通路偏好是否要加密儲存（避免被推測使用習慣）？
3. 跨境匯率 hedge 是否在 Op Co. 規模達一定門檻後啟動（如 Wise 之 forward contract）？
4. 越南 / 菲律賓本地通路（Momo / PayMongo）是否需要設第二家 Op Co. 才能合規收款？
5. 退款是否提供「退至錢包點數」之選項（玩家未真的轉錯）？— 法律風險：點數無回兌原則之邊界
