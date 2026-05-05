# SPEC-009 — DMM 計畫與流動性策略

> 優先級：**P1**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-001, SPEC-003, SPEC-005
> 被依賴：SPEC-011

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

## 5. 冷啟動補貼（極端情境，預留設計）

> **預設不啟用**。本節僅為「若 DMM 與 LM 皆不足，新興市場流動性極差時」之最後手段。需 Compliance Officer 個案核准。

機制：

- 由「平台公告錢包」（位於 Operating Co. 控制下，**對玩家完全透明揭露**）
- 限期掛入限量訂單（典型：< 1% 該市場預期成交量）
- 每筆掛單在 UI 標記為 `[Platform Bootstrap]`
- 成交後產生的 PnL 平台自吞，不分配給其他玩家
- 需在月度報表中向監管揭露 bootstrap 訂單金額

> 此設計仍會將平台短暫推向「下場」位置，**強烈建議避免**。優先依靠 DMM 與 LM。

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

```go
// internal/liquidity/dmm_session.go

package liquidity

type DMMSession struct {
    DMMID    string
    Conn     *tls.Conn
    Limits   DMMLimits
    Stats    *PerSessionStats
    onAck    func(OrderAckMsg) error
    onFill   func(FillMsg) error
}

type DMMLimits struct {
    MaxOpenOrdersPerMarket int
    MaxPositionPts         int64
    MaxOrderRatePerSec     int
    AllowedMarkets         []string
}

// HandleNewOrder validates against DMM limits before forwarding
// to the matching engine. Limits beyond engine-level pre-trade checks.
func (s *DMMSession) HandleNewOrder(o NewOrderMsg) error {
    if !contains(s.Limits.AllowedMarkets, o.MarketID) {
        return ErrMarketNotAllowed
    }
    if s.Stats.OpenOrders(o.MarketID) >= s.Limits.MaxOpenOrdersPerMarket {
        return ErrTooManyOpenOrders
    }
    if s.Stats.Position(o.MarketID).Add(o.Qty) > s.Limits.MaxPositionPts {
        return ErrPositionLimitExceeded
    }
    return s.engine.Submit(toEngineOrder(o, s.DMMID))
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
3. DMM API 採用 FIX 4.4 標準還是自建 binary？
4. LM 從手續費提撥 30% 是否過高？需平衡平台收入
5. 冷啟動補貼是否完全禁用？或保留極端情境彈性？
