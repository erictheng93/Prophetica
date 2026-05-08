# SPEC-009 — DMM 計畫與流動性策略

> 優先級：**P1**
> 版本：v0.2.1 · 最後更新：2026-05-07
> 依賴：SPEC-003, SPEC-005
> 被依賴：SPEC-011

> **v0.2.1 變更（R-2 紅線清除）**：依業主決議移除原 §5「冷啟動補貼 / 平台公告錢包」整段內容，留 placeholder 章節說明移除理由與替代方案（DMM rebate 放大）；§9 OQ #5 標 Resolved；連動 SPEC-003 §12.1 sentinel `system.platform_announcement` (uid=4) 移除（SPEC-003 v0.2.2）；ADR-0001 §6 「唯一例外」描述待新 ADR 更新為「無例外」。對應 [SPEC-000 §5 R-2](./SPEC-000-index.md)。
> **v0.2.0 變更**：跟隨 SPEC-014 v0.2.0 後端切 Rust，§7 程式碼風格範例之 `internal/liquidity/dmm_session.go` Go syntax 改寫為 Rust 等價（`tokio` + `tokio-rustls` + `thiserror`）；DMM session 之 limit 檢查邏輯不變。OQ #3「FIX 4.4 vs 自建 binary」隨 §3.4 已隱含 Resolved 為「自建 binary protobuf over TLS」，本次同步劃掉。

---

## 1. Objective

解決 P2P 訂單簿的「冷啟動流動性」問題，**同時保持平台中立性**。設計兩條互補機制：

- **Phase 1**（MVP / 軟發布）：**DMM 計畫** — 邀請外部專業做市商，平台不持倉
- **Phase 2+**（公開 Beta 後）：**Liquidity Mining** — 透過手續費 rebate 鼓勵一般玩家提供流動性

> ⚠️ **不可越線的紅線**：平台、平台子公司、平台員工、平台直接控制之機器人帳戶**永遠不得**在自家訂單簿掛單。任何「自動造市腳本」、「對沖機器人」、「概率對稱腳本」均不允許實作。

## 2. 為何 Order Book + DMM 取代 LMSR / 概率對稱？

| 方案 | 平台是否持倉 | 法律定位 | 流動性可控性 |
|------|------------|---------|------------|
| ❌ LMSR | 是（平台是 AMM）| 平台 = 莊家 | 高（公式決定）|
| ❌ 概率對稱腳本 | 是（平台機器人下單）| 平台 = 交易對手 | 高 |
| ✅ Order Book + DMM | 否（DMM 為對手方）| 平台 = 中性中介 | 中（DMM 自願報價）|
| ✅ Order Book + Liquidity Mining | 否（玩家為對手方）| 平台 = 中性中介 | 低（取決於玩家參與）|

法律上是否「莊家 / 中介」的判別線：**訂單簿上每一張 quote 的下單者是否為玩家或外部第三方**。

## 3. DMM 計畫

### 3.1 角色定義

DMM = Designated Market Maker（指定做市商）

- 法律地位：**獨立第三方**，與 Operating Co. 簽訂做市協議
- 可為：個人專業交易員（必須通過嚴格 KYC + 風險聲明）、機構（券商、量化基金、加密做市商如 Wintermute / GSR）
- 可在站內以普通玩家身份儲值點數，作為其做市本金

### 3.2 DMM 義務

每位 DMM 在簽約時承諾：

| 義務 | 量化要求 |
|------|---------|
| 雙向報價 | 開市時間內，每個指定市場必須同時掛 bid 與 ask |
| 最低盤口深度 | 每側至少 X 點 × Y 股（依市場類型訂定）|
| 最大價差 | bid-ask spread ≤ 平台公告值（預設 5%）|
| 在線時長 | 每月 ≥ 95% 時間履約 |
| 報價更新 | 訂單簿異動 100ms 內重新報價（非強制，但若履約低於門檻會被警告）|

### 3.3 DMM 福利

| 福利 | 說明 |
|------|------|
| Maker Rebate | maker 手續費 -0.1%（負費，平台補貼）|
| Taker Fee 折扣 | taker 手續費 -50%（從 0.2% → 0.1%）|
| 專屬 API | 低延遲 binary protocol（FIX-like）|
| 提前資訊 | 新市場開放前 24 小時通知（不含結果線索）|
| 月度 bonus | 履約優異者額外手續費返還 |

### 3.4 DMM API

獨立於玩家 REST API：

```
binary protocol over TLS, port 8443
Authentication: mutual TLS + DMM API key
Heartbeat: 1s

Messages:
  - LOGON
  - NEW_ORDER (LIMIT only, POST_ONLY default)
  - CANCEL_ORDER
  - MASS_CANCEL (cancel all on market_id)
  - ORDER_ACK / ORDER_FILL / ORDER_REJECT
  - MARKET_DATA (subscribe to book deltas)
  - HEARTBEAT
```

設計原則：
- 連線可承受 1000 ops/sec / connection
- 訊息為 binary protobuf 而非 JSON
- 撮合事件 P99 延遲 < 2ms 推送至 DMM

### 3.5 DMM 履約監控

每日批次計算每位 DMM：

```python
metrics = {
    'uptime_pct': online_minutes / market_open_minutes,
    'spread_avg': sum(bid_ask_spread_per_minute) / count,
    'depth_avg': sum(min_depth_per_side_per_minute) / count,
    'volume': total_traded_qty,
    'rebate_paid': maker_rebate_total,
}

if uptime_pct < 0.95: warn
if spread_avg > target_spread * 1.2: warn
if 連續 3 個月 warn: 終止合約
```

### 3.6 DMM 招募流程

1. 平台公告招募（官網 + 業界社群）
2. 候選者提交：身份證明、過往做市紀錄、KYC、AML
3. Compliance + Risk 雙簽核
4. 簽署做市協議（Operating Co. 律師起草）
5. 沙盒環境壓測（1 週）
6. 上線小額額度（USD 10k 等值點數）
7. 履約良好後逐步提高額度

### 3.7 DMM 風險管理

DMM **以自身點數做市**，承擔所有部位風險。但平台需保護玩家不因 DMM 系統故障受損：

- DMM 帳戶**最大持倉限額**（防止單一 DMM 主導市場）
- DMM 必須維持最低保證金（cash buffer）
- 系統偵測 DMM 連續異常（如價差爆衝、持倉爆量）→ 自動凍結該 DMM 該市場下單能力

## 4. Phase 2: Liquidity Mining

### 4.1 啟動前置條件

**MUST** 在以下指標全部達成後才啟用：

- [ ] 反洗售（wash trading）偵測系統線上 ≥ 90 天
- [ ] 反多帳號偵測準確率 > 95%
- [ ] 平台 DAU ≥ 5,000
- [ ] 至少 3 家 DMM 穩定運作 ≥ 6 個月
- [ ] Compliance 簽核 LM 反作弊白皮書

### 4.2 LM 機制

- 每個市場開放「LP 角色」註冊
- LP 可被動或主動掛單
- 每日結算時依公式計算 LP 該日對流動性的貢獻：

```
LP_contribution = Σ_t [time_at_quote * min(bid_depth, ask_depth) * inverse_spread]
```

- 平台從**該市場手續費**中提撥 X%（預設 30%）依貢獻比例分配給 LP

### 4.3 LM 反作弊

LM 最大風險是**自我成交 wash trading 套取 reward**。對策：

- LP 帳戶**強制 KYC Tier 2+**
- 同 device fingerprint / IP / KYC hash 之間**不視為對手方**
- LP 不可直接從自己掛的單成交（已由 STP 處理，加強檢查）
- 每日 LM reward 計算前，過濾「疑似套利」交易：
  - 同一玩家在 N 分鐘內買賣同一 outcome
  - 兩玩家彼此交易頻率異常高
  - 兩玩家儲值來源相同
- LM reward **延遲 7 日**發放，期間若偵測到套利可撤銷
- 每月公開 LM 排行榜，讓社群監督

### 4.4 LM 發放方式

點數 reward 寫入 wallet：
- 即時記錄為「pending LM reward」
- 7 日確認期過後轉為「available」
- 計入收入帳戶 `Expense.LiquiditySubsidy`

## 5. 冷啟動補貼（**已移除 v0.2.1**）

> **本節原規範「平台公告錢包」之冷啟動補貼路徑已於 2026-05-07 整段移除**（呼應 [SPEC-000 §5 R-2](./SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker) 業主決議）。
>
> ### 移除理由
>
> 原設計即便標註「強烈建議避免」、「3-of-3 簽核啟用」、「UI 標記 `[Platform Bootstrap]`」、「月度揭露」，仍**保留了一條程式碼路徑**讓平台得以在自家訂單簿掛單。此路徑已由業主決議移除；SDD 精神：**spec 上不存在的東西就不能在 code 中存在**。
>
> ### 替代方案：流動性不足時改注入 DMM rebate
>
> 流動性極差之新興市場，補貼預算改透過 §3.3 **DMM Maker Rebate** 放大（評估從 -0.1% 提高至 -0.2% / -0.3%，由 Compliance + 財務做財務模型評估），由**獨立第三方 DMM**在訂單簿掛單，平台始終維持中立中介角色，**永不下場**。若 DMM 仍不足，市場應**延後上架**而非由平台暫代造市。
>
> ### 後續落地清單
>
> - [x] SPEC-003 §12.1.1 sentinel `system.platform_announcement` (uid=4) 同步移除（SPEC-003 v0.2.2）
> - [x] SPEC-003 §12.1.4「例外情況」整節刪除（SPEC-003 v0.2.2）
> - [ ] **ADR-0001 §6 待 patch**：原描述「`system.platform_announcement` 為唯一例外」需更新為「**無例外** — 平台永遠不在訂單簿掛單」；需開新 ADR 標記 ADR-0001 §6 該條 superseded
> - [x] SPEC-009 §9 OQ #5 Resolved 為「**完全禁用，不留彈性**」
> - [ ] CI sentinel-scan 不必再為 `system.platform_announcement` 留 whitelist 例外（隨 SPEC-003 §12.1.3 grep gate 自動生效）
>
> ### 章節編號保留
>
> 為避免破壞既有 SPEC cross-reference（如其他 SPEC 引用「SPEC-009 §6 流動性指標」），本章節編號 §5 保留為 placeholder，§6 ~ §9 不重編。

## 6. 訂單簿流動性指標監控

呼應 SPEC-007，加入以下指標：

```
proph_market_spread_bps                 gauge, labels: market_id
proph_market_depth_pts                  gauge, labels: market_id, side
proph_market_quoted_seconds_total       counter, labels: market_id
proph_dmm_uptime_pct                    gauge, labels: dmm_id
proph_lm_reward_pending_pts_total       gauge
proph_lm_reward_distributed_pts_total   counter
```

## 7. 程式碼風格範例

```rust
// services/dmm-gateway/src/session.rs

use std::collections::HashSet;
use std::sync::Arc;

use thiserror::Error;
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

use crate::engine::{EngineClient, EngineError};
use crate::messages::NewOrderMsg;
use crate::stats::PerSessionStats;

pub struct DmmSession {
    pub dmm_id: String,
    pub conn:   TlsStream<TcpStream>,
    pub limits: DmmLimits,
    pub stats:  Arc<PerSessionStats>,
    pub engine: EngineClient,
}

pub struct DmmLimits {
    pub max_open_orders_per_market: u32,
    pub max_position_pts:           i64,
    pub max_order_rate_per_sec:     u32,
    pub allowed_markets:            HashSet<String>,
}

#[derive(Debug, Error)]
pub enum DmmError {
    #[error("market not allowed for this DMM")]
    MarketNotAllowed,
    #[error("too many open orders for this market")]
    TooManyOpenOrders,
    #[error("DMM position limit exceeded")]
    PositionLimitExceeded,
    #[error(transparent)]
    Engine(#[from] EngineError),
}

impl DmmSession {
    /// `handle_new_order` validates against DMM-level limits **before**
    /// forwarding to the matching engine. These limits are layered on top of
    /// the engine-level pre-trade checks defined in SPEC-003 §12.
    ///
    /// DMM order 進入引擎前必須以 DMM 法人 user_id 標記（呼應
    /// SPEC-003 §12.1.3 CI 靜態掃描第 3 條），不可使用 system sentinel id。
    pub async fn handle_new_order(&self, msg: NewOrderMsg) -> Result<(), DmmError> {
        if !self.limits.allowed_markets.contains(&msg.market_id) {
            return Err(DmmError::MarketNotAllowed);
        }
        if self.stats.open_orders(&msg.market_id) >= self.limits.max_open_orders_per_market {
            return Err(DmmError::TooManyOpenOrders);
        }
        if self.stats.position(&msg.market_id) + msg.qty > self.limits.max_position_pts {
            return Err(DmmError::PositionLimitExceeded);
        }
        self.engine
            .submit(crate::engine::to_engine_order(msg, &self.dmm_id))
            .await?;
        Ok(())
    }
}
```

## 8. 驗收條件

- [ ] DMM API 沙盒環境啟動，可從 sample DMM client 下單 / 撤單 / 收推播
- [ ] DMM 履約監控腳本每日自動跑，產生 dashboard
- [ ] DMM 簽約模板由律師完成
- [ ] 平台 codebase 無任何「自動造市」「自動對沖」程式碼存在
- [ ] LM 啟動前置檢查清單在 Phase 2 前完成
- [ ] LM 反 wash trading 偵測通過模擬攻擊測試

## 9. Open Questions

1. DMM 招募是公開招募還是邀請制？目標數量？
2. DMM rebate 是 -0.1% 還是更激進（-0.2%）？需做財務模型
3. ~~DMM API 採用 FIX 4.4 標準還是自建 binary？~~ ✅ **Resolved 2026-05-07**：採 **自建 binary protobuf over TLS**，§3.4 已落地 message 清單。理由：FIX 4.4 對 prediction market 雙腿合約語意支援不佳（FIX 為股票 / 期貨設計），且自建 protobuf 與內部 `packages/proto/v1/engine.proto` schema 共用，演化成本低。
4. LM 從手續費提撥 30% 是否過高？需平衡平台收入
5. ~~冷啟動補貼是否完全禁用？或保留極端情境彈性？~~ ✅ **Resolved 2026-05-07（業主決議）**：**完全禁用，不留彈性**。整段 §5「冷啟動補貼」已移除；SPEC-003 §12.1.1 sentinel `system.platform_announcement` (uid=4) 同步移除；ADR-0001 §6 「唯一例外」描述待新 ADR 更新為「無例外」。流動性不足走 §3.3 DMM rebate 放大或市場延後上架。對應 [SPEC-000 §5 R-2](./SPEC-000-index.md)。
