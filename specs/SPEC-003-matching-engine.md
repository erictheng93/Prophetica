# SPEC-003 — 撮合引擎詳規（Matching Engine）

> 優先級：**P0**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-002
> 被依賴：SPEC-004, SPEC-007, SPEC-009

---

## 1. Objective

設計一個 **單寫者、in-process、event-sourced** 的訂單簿撮合引擎，支援預測市場 YES/NO 雙腿合約的下單、撤單、成交、推播，達成 P99 < 5ms 的撮合延遲，並提供完整的事件流供下游帳本、結算、推播服務消費。

## 2. 設計原則

| 原則 | 內涵 |
|------|------|
| **單一寫者**（Single Writer） | 每個市場一個 goroutine 獨佔該市場的訂單簿，禁止鎖、禁止並發寫入 |
| **In-Process Memory** | 訂單簿完全在 Go 進程記憶體，不經 Redis、不經網路 |
| **Event Sourcing** | 撮合引擎只產生事件，所有下游（帳本、推播、持倉、K 線）由消費者重建 |
| **WAL + Snapshot** | 寫入 append-only log（磁碟 + Kafka），定期 snapshot，crash recovery 從 snapshot + replay |
| **確定性**（Deterministic） | 給定相同事件序列，重建出的訂單簿狀態必須 100% 一致 |
| **Price-Time FIFO** | 撮合演算法為**價格優先 → 時間優先**，與 Kalshi 一致 |
| **平台不持倉** | 引擎無 house book，無自動造市，所有 quote 來自玩家或 DMM |

## 3. 合約模型

### 3.1 市場（Market）

每個市場代表一個 **二元結果（Binary Outcome）** 事件。

```go
type Market struct {
    ID            string         // 'mkt_2026_election_winner'
    Title         string
    Description   string
    OutcomeYES    Outcome        // YES 腿
    OutcomeNO     Outcome        // NO 腿
    OpensAt       time.Time
    ClosesAt      time.Time      // 截止下單
    SettlesAt     time.Time      // 預計結算
    Status        MarketStatus   // open / closed / settling / settled / cancelled
    OracleSource  string         // 結果來源
    MinTickPts    int64          // 最小價格單位（內部單位百分點），預設 1 = 0.01 點
    MinLot        int64          // 最小股數，預設 1
    MaxOrderQty   int64          // 單筆最大股數，防作弊
    FeeBps        int            // 手續費 basis points，預設 200 (=2%)
}
```

### 3.2 結果腿（Outcome）

```go
type Outcome string

const (
    OutcomeYES Outcome = "YES"
    OutcomeNO  Outcome = "NO"
)
```

### 3.3 為何只用 YES 腿，不需要 NO 訂單簿？

> **關鍵設計決策**：本系統採用 Kalshi 模型，**只維護 YES 腿訂單簿**。賣 YES @ 0.65 等價於買 NO @ 0.35。

#### 3.3.1 設計理由

- 訂單簿單一化，記憶體與 CPU 減半
- 結算時 1 股 YES = $1（中獎）或 $0（沒中），1 股 NO 互補
- 所有 NO 操作在引擎邊界轉換為 YES 操作

#### 3.3.2 邊界轉換規則

| 玩家動作 | 引擎內部處理 |
|---------|-------------|
| 買 YES @ 0.65 × 100 | Bid YES @ 65 × 100 |
| 賣 YES @ 0.65 × 100 | Ask YES @ 65 × 100 |
| 買 NO @ 0.35 × 100 | Ask YES @ 65 × 100（轉換）|
| 賣 NO @ 0.35 × 100 | Bid YES @ 65 × 100（轉換）|

> 邊界層 SDK 自動轉換，玩家在 UI 看到的仍是「買 NO」，但後端引擎只處理 YES。

## 4. 訂單模型

### 4.1 訂單類型（Order Types）

| 類型 | 含義 | 行為 |
|------|------|------|
| **LIMIT** | 限價單 | 指定價格，未成交部分掛入訂單簿 |
| **MARKET** | 市價單 | 不指定價格，吃單至成交或耗盡，剩餘 cancel |
| **IOC** (Immediate-Or-Cancel) | 即時成交否則取消 | 立即撮合可成交部分，剩餘自動取消 |
| **FOK** (Fill-Or-Kill) | 全部成交否則取消 | 必須一次全量成交，否則完全取消 |
| **POST_ONLY** | 只掛單 | 必為 maker，若會立即成交則拒絕 |

### 4.2 訂單狀態機

```
                  ┌─────────────┐
                  │   PENDING   │  接收 API、尚未進入撮合佇列
                  └──────┬──────┘
                         │ enqueue
                         ▼
                  ┌─────────────┐
                  │   ACTIVE    │  在訂單簿上等待撮合
                  └──┬───────┬──┘
            partial  │       │  full match
                     ▼       ▼
              ┌─────────┐ ┌──────────┐
              │ PARTIAL │ │  FILLED  │
              └────┬────┘ └──────────┘
                   │ user cancel / time-in-force
                   ▼
              ┌──────────┐
              │ CANCELLED│
              └──────────┘
```

### 4.3 訂單資料結構

```go
type OrderID = uint64
type Side    int

const (
    Bid Side = 1   // 買 YES
    Ask Side = -1  // 賣 YES
)

type Order struct {
    ID             OrderID
    UserID         uint64
    MarketID       string
    Side           Side
    Type           OrderType        // LIMIT / MARKET / IOC / FOK / POST_ONLY
    PricePts       int64            // 內部單位（0-100, 對應 0.00 ~ 1.00）
    QtyTotal       int64            // 原始數量
    QtyRemaining   int64            // 剩餘未成交
    Status         OrderStatus
    SubmittedAt    int64            // unix nano，作為時間優先依據
    ClientOrderID  string           // 玩家自定義 ID，用於冪等
    TIF            TimeInForce      // GTC / IOC / FOK / DAY
    PostOnly       bool
}
```

## 5. 訂單簿資料結構

### 5.1 整體結構

每個 Market 一份訂單簿：

```go
type OrderBook struct {
    MarketID  string

    // 價格 → 該價位的訂單佇列
    // 採 sorted map (B-tree)，bid 由高到低，ask 由低到高
    Bids      *btree.BTreeG[*PriceLevel]  // descending
    Asks      *btree.BTreeG[*PriceLevel]  // ascending

    // 訂單 ID → Order 物件，用於 O(1) 撤單
    OrdersByID map[OrderID]*Order

    // 性能指標
    LastTradePts  int64
    BestBidPts    int64
    BestAskPts    int64
    SeqNo         uint64  // 嚴格遞增的事件序號
}

type PriceLevel struct {
    PricePts  int64
    Queue     *list.List   // FIFO，雙向鏈結串列
    TotalQty  int64        // 該價位累計掛單量（cache）
}
```

### 5.2 為何 B-tree 而非 skip list / RB-tree？

| 結構 | 插入 | 刪除 | 範圍掃描 | 記憶體 | 評價 |
|------|------|------|---------|-------|------|
| sorted slice | O(N) | O(N) | O(K) | 最低 | 慢 |
| RB-tree | O(logN) | O(logN) | O(K logN) | 中 | OK |
| skip list | O(logN) | O(logN) | O(K) | 高 | OK |
| **B-tree** | O(logN) | O(logN) | O(K) | **低（cache friendly）** | **最佳** |

選用 [google/btree](https://github.com/google/btree)，每個節點 32 個 entries，cache line 對齊。

### 5.3 為何 list.List（雙向鏈結）而非 slice？

每個 PriceLevel 內部訂單佇列：
- **撮合**：總是從 head 取 → O(1)
- **加入**：append 到 tail → O(1)
- **撤單**：給定 *list.Element 指針 → O(1) 移除

slice 撤單需 O(N)，不可行。

## 6. 撮合演算法

### 6.1 主流程偽碼

```go
func (e *Engine) handleOrder(o *Order) []Event {
    events := []Event{OrderAccepted{o.ID, e.SeqNo()}}

    book := e.books[o.MarketID]
    var oppositeSide *btree.BTreeG[*PriceLevel]
    if o.Side == Bid { oppositeSide = book.Asks } else { oppositeSide = book.Bids }

    for o.QtyRemaining > 0 {
        bestLevel := peekBestPriceLevel(oppositeSide)
        if bestLevel == nil { break }

        // 價格不交叉 → 不可成交
        if !canCross(o, bestLevel) { break }

        // FOK 預檢
        if o.Type == FOK && !canFillCompletely(o, oppositeSide) {
            return append(events, OrderCancelled{o.ID, ReasonFOKReject})
        }

        // POST_ONLY 命中對手單 → 拒絕
        if o.PostOnly {
            return append(events, OrderRejected{o.ID, ReasonPostOnlyCross})
        }

        match, ev := matchAtLevel(o, bestLevel)
        events = append(events, ev...)
        if !match { break }
    }

    if o.QtyRemaining > 0 {
        switch o.Type {
        case LIMIT:
            insertIntoBook(book, o)
            events = append(events, OrderResting{o.ID})
        case MARKET, IOC:
            events = append(events, OrderCancelled{o.ID, ReasonIOCRemainder})
        }
    }
    return events
}
```

### 6.2 價格交叉判定

```go
func canCross(taker *Order, level *PriceLevel) bool {
    if taker.Type == MARKET { return true }
    if taker.Side == Bid {
        return taker.PricePts >= level.PricePts
    }
    return taker.PricePts <= level.PricePts
}
```

### 6.3 成交事件產生

每筆撮合產生一筆 `Trade` 事件：

```go
type Trade struct {
    SeqNo      uint64
    MarketID   string
    PricePts   int64
    Qty        int64
    TakerID    OrderID
    MakerID    OrderID
    TakerUserID uint64
    MakerUserID uint64
    TakerFeeBps int
    MakerFeeBps int   // maker rebate 可為負
    TimestampNs int64
}
```

### 6.4 撤單

```go
func (e *Engine) cancelOrder(orderID OrderID) []Event {
    book := e.books[orderID.MarketID()]  // 假設 ID 內含 market
    o, ok := book.OrdersByID[orderID]
    if !ok { return []Event{CancelRejected{orderID, NotFound}} }
    if o.Status == FILLED || o.Status == CANCELLED {
        return []Event{CancelRejected{orderID, AlreadyFinal}}
    }

    level := findLevel(o)
    level.Queue.Remove(o.element)        // O(1)
    level.TotalQty -= o.QtyRemaining
    if level.Queue.Len() == 0 {
        deleteLevel(book, o.Side, level.PricePts)
    }
    delete(book.OrdersByID, orderID)
    o.Status = CANCELLED
    return []Event{OrderCancelled{orderID, ReasonUserRequest}}
}
```

## 7. 並發模型

### 7.1 Per-Market Single Writer

```go
type Engine struct {
    markets map[string]*MarketShard
}

type MarketShard struct {
    book      *OrderBook
    ingressCh chan command         // 該市場所有寫入命令
    eventCh   chan Event           // 輸出事件流
}

// 每個市場一個 goroutine
func (s *MarketShard) run() {
    for cmd := range s.ingressCh {
        switch c := cmd.(type) {
        case *PlaceOrderCmd:
            events := s.book.handleOrder(c.Order)
            s.publish(events)
        case *CancelCmd:
            events := s.book.cancelOrder(c.OrderID)
            s.publish(events)
        }
    }
}
```

### 7.2 為何單寫者勝過 mutex？

- 無鎖競爭，無 spin lock，無 priority inversion
- CPU cache locality 最優（同一 goroutine 反覆操作同一資料結構）
- 撮合邏輯本質序列化（成交順序必須確定）
- 鎖會限制吞吐至 ~10k ops/sec，single writer 可達 ~500k ops/sec

### 7.3 跨市場並行

不同市場彼此獨立，不同 goroutine 並行運行。

```
CPU Core 0: Goroutine for Market A
CPU Core 1: Goroutine for Market B
CPU Core 2: Goroutine for Market C
...
```

Linux 用 `runtime.LockOSThread()` 將熱市場鎖到特定 core，搭配 `taskset` 提升 cache hit rate。

## 8. 持久化（Event Sourcing + WAL）

### 8.1 寫入路徑

```
Order API → ingressCh (in-memory) → MarketShard.run()
                                          ↓
                                    生成 Events
                                    ↙          ↘
                              WAL (本地磁碟)   Kafka topic
                                    ↓              ↓
                              Snapshot 用      下游 consumers:
                                              - Ledger Service
                                              - Position Service
                                              - WebSocket Pub
                                              - K 線 / Tick DB
                                              - 風控服務
```

### 8.2 WAL 格式

每筆事件序列化為 protobuf，append 到 daily log file：

```
/var/lib/proph/wal/2026-05-05/market_X.wal.gz
```

- 每行：`[seq_no][timestamp][event_type][payload_proto]`
- gzip 壓縮、fdatasync 每 100 events 或 50ms（whichever 先）
- Kafka 寫入用 `acks=all` + idempotent producer

### 8.3 Snapshot

每 60 秒（或每 100k events）產生 snapshot：

```
/var/lib/proph/snapshots/2026-05-05T14-30-00/market_X.snap
```

包含：
- 完整 OrderBook 序列化
- 最後處理的 SeqNo

### 8.4 Crash Recovery

```go
func (s *MarketShard) recover() {
    snap := loadLatestSnapshot(s.MarketID)
    s.book = snap.Book
    nextSeq := snap.SeqNo + 1

    walEvents := readWALFrom(s.MarketID, nextSeq)
    for _, ev := range walEvents {
        s.book.applyEvent(ev)  // 純函數重建
    }
}
```

恢復時間目標：**< 30 秒** for 任何單一市場（< 1M 事件）。

## 9. 事件型別清單

| Event | 觸發 | 下游消費 |
|-------|------|---------|
| `OrderAccepted` | 通過驗證進入引擎 | 風控、稽核 |
| `OrderRejected` | 拒收（餘額、限額、風控） | 玩家通知 |
| `OrderResting` | 進入訂單簿掛單 | 訂單簿 WS 推播 |
| `Trade` | 撮合成交 | 帳本、持倉、K 線、稽核 |
| `OrderFilled` | 訂單完全成交 | 玩家通知 |
| `OrderPartiallyFilled` | 部分成交 | 玩家通知 |
| `OrderCancelled` | 用戶撤單或 IOC/FOK | 玩家通知、訂單簿 WS |
| `MarketStatusChanged` | open → closed → settling → settled | 全玩家 |
| `BookSnapshot` | 訂閱者初次連線推送 | WS 訂閱者 |
| `BookDelta` | 增量更新 | WS 訂閱者 |

## 10. API 介面

### 10.1 REST 下單（同步）

```
POST /api/v1/markets/{market_id}/orders
Headers:
  Authorization: Bearer <jwt>
  Idempotency-Key: <client-uuid>
  X-Client-Order-Id: <player-supplied>

Body:
{
  "side": "buy" | "sell",
  "outcome": "YES" | "NO",
  "type": "LIMIT" | "MARKET" | "IOC" | "FOK" | "POST_ONLY",
  "price_pts": 65,           // 0-100, optional for MARKET
  "quantity": 100,
  "time_in_force": "GTC" | "DAY" | "IOC" | "FOK"
}

Response 200:
{
  "order_id": "ord_01H...",
  "status": "ACTIVE" | "FILLED" | "PARTIAL" | "CANCELLED",
  "filled_qty": 50,
  "avg_price_pts": 64,
  "remaining_qty": 50,
  "fee_pts": 65
}
```

### 10.2 撤單

```
DELETE /api/v1/orders/{order_id}
```

### 10.3 WebSocket 訂單簿推播

```
ws://api.../ws/markets/{market_id}/book

→ subscribe: { "depth": 10 }
← snapshot: { "seq": 12345, "bids": [...], "asks": [...] }
← delta:    { "seq": 12346, "side": "bid", "price_pts": 65, "qty_delta": -100 }
← trade:    { "seq": 12347, "price_pts": 65, "qty": 100, "ts": ... }
```

WebSocket 連線必須能透過 `seq` 偵測 gap（漏訊），客戶端 detect gap 後重新 snapshot。

### 10.4 WebSocket 個人訂單推播

```
ws://api.../ws/me/orders

← order_update: { "order_id": ..., "status": "FILLED", ... }
```

## 11. 性能 SLO

| 指標 | 目標 | 量測 |
|------|------|------|
| 撮合延遲（API 接收 → 事件發布）| P50 < 1ms, P99 < 5ms | OpenTelemetry span |
| 單市場吞吐 | 持續 30,000 orders/sec | 壓測 |
| 全平台吞吐 | 100,000 orders/sec（100 個市場）| 壓測 |
| WebSocket 推播延遲 | P99 < 30ms (engine → 客戶端 receive) | RUM |
| Crash Recovery | < 30 秒 / 市場 | DR 演練 |
| 訂單簿深度查詢 | P99 < 5ms（snapshot 提供）| API 監控 |

## 12. 風控前置（Pre-trade Risk Check）

引擎**接收訂單前**必須通過：

| 檢查 | 條件 | 失敗回應 |
|------|------|---------|
| 餘額充足 | `wallet.available ≥ price * qty + fee` | INSUFFICIENT_FUNDS |
| 持倉上限 | `position.qty + new_qty ≤ market.MaxPositionPerUser` | POSITION_LIMIT_EXCEEDED |
| 單筆上限 | `qty ≤ market.MaxOrderQty` | ORDER_QTY_TOO_LARGE |
| 自我成交 | `taker.userID != maker.userID`（找對手前驗）| SELF_TRADE_PREVENTED |
| 價格區間 | `0 < price < 100`（嚴格區間，0 / 100 結算用）| INVALID_PRICE |
| 市場狀態 | `market.status == OPEN` | MARKET_NOT_OPEN |
| 玩家狀態 | `user.status == ACTIVE` & `!self_excluded` | USER_RESTRICTED |
| 反作弊評分 | `risk_score < 80` | RISK_REJECTED |
| **平台帳戶禁區** | `order.user_id ∉ PLATFORM_SYSTEM_USER_IDS`（呼應 ADR-0001 §6）| PLATFORM_USER_NOT_ALLOWED |

風控前置在獨立 service 完成，撮合引擎假設輸入已通過。

### 12.1 PLATFORM_SYSTEM_USER_IDS Sentinel Set（Compliance 紅線）

> **強制紅線**：呼應 [ADR-0001 §6](../docs/architecture/decisions/ADR-0001-order-book-with-dmm-over-lmsr.md)。引擎 `ingressCh` 必須對 incoming order 執行 `order.user_id != PLATFORM_SYSTEM_USER_IDS` 檢查；命中則直接 reject 並寫入 SIEM。任何將平台 / 系統 user_id 加入下單能力之 PR **必須 CI fail + reject**。

#### 12.1.1 常數定義（Go）

```go
// services/matching-engine/internal/engine/sentinel.go
package engine

// PLATFORM_SYSTEM_USER_IDS 列舉所有「禁止下單」之系統 user_id。
// 任何 incoming order 的 user_id 命中本集合則直接拒絕。
//
// 紅線理由：
//   - 平台不得在自家訂單簿掛單（SPEC-001 §1.1 ② No House Position）
//   - ADR-0001 §6 列為靜態掃描禁字
//
// 新增 user_id 至本集合需要：
//   1. ADR-XXXX 簽核
//   2. Compliance Officer 4-eye review
//   3. 與本檔案的 git blame 對照可追溯
var PLATFORM_SYSTEM_USER_IDS = map[uint64]string{
    1: "system.root",                  // bootstrap / migration only, never used at runtime
    2: "system.compliance",            // 凍結 / 解凍帳戶之 audit shim, never trades
    3: "system.settlement_oracle",     // Oracle 提交 shim, never trades
    4: "system.platform_announcement", // 公告錢包（ADR-0001 §6 唯一例外，預設不啟用）
    5: "system.cs_compensation",       // 客服補償帳戶，呼應 SPEC-002 §8
    6: "system.suspense",              // 對帳暫掛，呼應 SPEC-002 §4.2
    7: "system.affiliate_pool",        // 推薦獎勵發放暫存池，呼應 SPEC-011
    8: "system.dmm_rebate_pool",       // DMM 補貼暫存池，呼應 SPEC-009
}

// IsPlatformSystemUser returns true if the given user_id is reserved as a
// platform sentinel and must NEVER appear as an order placer.
func IsPlatformSystemUser(uid uint64) bool {
    _, ok := PLATFORM_SYSTEM_USER_IDS[uid]
    return ok
}
```

#### 12.1.2 引擎入口檢查（強制）

```go
// services/matching-engine/internal/engine/ingress.go
func (s *MarketShard) handleCommand(c command) {
    if order, ok := c.(*PlaceOrderCmd); ok {
        if engine.IsPlatformSystemUser(order.UserID) {
            // 立即 reject，並產生 critical 等級 audit log
            s.publishEvent(OrderRejected{
                OrderID: order.ID,
                Reason:  ReasonPlatformUserNotAllowed,
            })
            metrics.PlatformUserOrderBlocked.Inc()
            audit.Critical("platform sentinel user attempted order", map[string]any{
                "user_id":   order.UserID,
                "order_id":  order.ID,
                "market_id": order.MarketID,
            })
            return
        }
        // ... 進入正常撮合
    }
}
```

#### 12.1.3 CI 靜態掃描

呼應 ADR-0001 §6，CI pipeline 必含以下檢查（在 SPEC-013 §3.2 列為阻斷 gate）：

```bash
# .github/workflows/sentinel-scan.yml （示意）
# 1. 禁字掃描（ADR-0001 §6）
rg -n -e 'auto_quote|market_balancer|house_book|probability_symmetry|auto_hedge' \
   --type go services/ packages/ \
   && echo "ADR-0001 §6 forbidden words detected" && exit 1

# 2. PLATFORM_SYSTEM_USER_IDS 集合不得在 ingressCh 路徑外被旁路
rg -n -e 'PLATFORM_SYSTEM_USER_IDS' --type go services/matching-engine/ \
   | grep -vE '(sentinel\.go|ingress\.go|sentinel_test\.go)' \
   && echo "PLATFORM_SYSTEM_USER_IDS referenced outside whitelisted paths" && exit 1

# 3. DMM session 檢查：DMM order 進入 engine 前必須轉換 user_id 為 DMM 法人帳戶
#    （SPEC-009 §3.4），不可使用 system user_id
rg -n -e 'PLATFORM_SYSTEM_USER_IDS\[.*\]' --type go services/dmm-gateway/ \
   && echo "DMM gateway must not reference platform sentinel users" && exit 1
```

#### 12.1.4 例外情況

唯一可能在訂單簿掛單的「系統相關」是 ADR-0001 §6 之 `system.platform_announcement`（user_id = 4）「冷啟動補貼」極端情境。本路徑：

- **預設 `disabled` feature flag**
- 啟用需 Compliance Officer + Tech Lead + CTO 三方簽核
- 啟用後該帳戶之 order 在訂單簿 UI 強制標記 `[Platform Bootstrap]`（呼應 SPEC-009 §5）
- 月度監管報表強制揭露
- **強烈建議永不啟用**

## 13. 自我成交防止（Self-Trade Prevention, STP）

### 13.1 觸發

當 incoming taker order 將與**同一 user_id** 的 resting order 撮合時。

### 13.2 處置策略（可配置，預設 CANCEL_NEWEST）

| 策略 | 行為 |
|------|------|
| `CANCEL_NEWEST` | 取消 incoming order |
| `CANCEL_OLDEST` | 取消 resting maker |
| `CANCEL_BOTH` | 兩者皆取消 |
| `DECREMENT_BOTH` | 兩單數量取較小值的差後繼續 |

DMM / 機構帳戶可設不同策略，零售玩家強制 `CANCEL_NEWEST` 防作弊。

## 14. 邊界條件 / 已知陷阱

### 14.1 整數運算與精度

- **絕不使用 float**。所有價格與數量為 int64。
- 價格內部單位：1 = 0.01（0-100 對應 0.00-1.00）
- 點數內部單位：1 = 0.01 點（與 SPEC-002 一致）
- 手續費 basis points：1 bp = 0.01%

### 14.2 金額計算

```go
// trade value (in 點 internal units, 100 = 1 點)
// price_pts: 0-100, qty: 股數
// trade_value = price_pts * qty  →  internal pts × 1
// 例: 65 × 100 = 6500 internal pts = 65.00 點
tradeValue := int64(price_pts) * int64(qty)

// fee = trade_value * fee_bps / 10000
fee := tradeValue * int64(feeBps) / 10000
```

### 14.3 NO 腿價格驗證

NO 訂單轉換為 YES 時：`yes_price = 100 - no_price`。轉換後價格必須仍在 (0, 100) 區間。

### 14.4 市場關閉時刻的訂單

`market.ClosesAt` 之後：
- 新訂單**直接拒絕**
- 訂單簿上的活躍訂單**全部 cancel**，未成交數量 reverse 鎖倉
- 觸發 `MarketStatusChanged` → `CLOSED` 事件
- 等待結算（見 SPEC-004）

## 15. 程式碼風格範例

```go
// internal/engine/orderbook.go

package engine

import (
    "container/list"
    "github.com/google/btree"
)

type OrderBook struct {
    marketID   string
    bids       *btree.BTreeG[*PriceLevel]
    asks       *btree.BTreeG[*PriceLevel]
    ordersByID map[OrderID]*orderHandle
    seq        uint64
}

type orderHandle struct {
    order  *Order
    level  *PriceLevel
    elem   *list.Element
}

// applyOrder is the only mutator. It must be called from the market's
// dedicated goroutine; no locks taken. Returns events in order.
func (b *OrderBook) applyOrder(o *Order) []Event {
    b.seq++
    o.SeqNo = b.seq

    events := []Event{OrderAccepted{Seq: b.seq, OrderID: o.ID}}

    opposite := b.oppositeBook(o.Side)
    for o.QtyRemaining > 0 {
        best, ok := peekBest(opposite, o.Side)
        if !ok || !canCross(o, best) {
            break
        }
        ev := b.matchAtLevel(o, best)
        events = append(events, ev...)
    }

    if o.QtyRemaining > 0 && o.Type == TypeLimit {
        b.insertResting(o)
        events = append(events, OrderResting{OrderID: o.ID})
    }
    return events
}
```

## 16. 測試策略

### 16.1 單元測試（goleak）

- `orderbook_test.go`：100% line coverage
- 撮合 happy path
- 自我成交防止
- IOC / FOK / POST_ONLY 行為
- 撤單 (rest / partial / 不存在)

### 16.2 屬性測試（property-based, gopter）

- 任意訂單序列下，不變式皆成立：
  - Σ(bid qty) + Σ(ask qty) + Σ(filled qty) = Σ(submit qty)
  - best_bid < best_ask（無交叉）
  - SeqNo 嚴格遞增

### 16.3 重放確定性測試

- 從 WAL 重放歷史事件，重建狀態必須與 snapshot byte-equal

### 16.4 壓力測試

- 100 萬 orders、1 個市場：吞吐、P99
- 100 個市場、每市場 10k orders：跨核擴展性

### 16.5 混沌測試

- kill -9 + 自動 recovery：30 秒內必須完整恢復
- 磁碟滿：必須拒收新訂單，已接受訂單持續處理
- 網路分區：Kafka 不可達時 WAL 仍寫入磁碟

## 17. 部署與運維

### 17.1 部署模式

- 撮合引擎以 **DaemonSet + hostNetwork: true** 部署在 K8s
- 或：bare-metal Linux server，systemd 管理
- 不放在 sidecar / sidecar mesh 內（避免 Envoy 引入延遲）

### 17.2 一個市場一個分片，跨機器分配

```
Machine 1: markets ['m1', 'm2', ..., 'm50']  (per-process load balanced)
Machine 2: markets ['m51', ..., 'm100']
```

熱市場（成交量 > 閾值）獨佔一個 process，避免 GC 拖累其他市場。

### 17.3 升級

- 滾動升級：新機器啟動 → 從 snapshot + WAL 恢復 → ingress 切換 → 舊機器排乾
- 強制 graceful shutdown：拒收新 order，處理完佇列後 snapshot 並退出

## 18. 驗收條件

- [ ] 單元測試 + 屬性測試覆蓋率 ≥ 95%
- [ ] 壓測：1M orders 處理、平均 P50/P99 達 SLO
- [ ] 重放確定性測試通過
- [ ] Kafka 投遞 zero-loss 驗證（kill -9 後事件不丟失）
- [ ] WebSocket 推播延遲 P99 < 30ms 在 1000 並發訂閱者下
- [ ] STP 行為與配置策略一致
- [ ] Crash recovery < 30 秒

## 19. Open Questions

> 跨檔決策已收斂至 [SPEC-000 §5](./SPEC-000-index.md) 中央追蹤表。本節僅保留 SPEC-003 自身範圍內議題。

1. WAL 是否需要 mmap 加速？或直接 buffered write 已足夠？
2. Snapshot 序列化格式：protobuf vs MessagePack vs custom binary？
3. 訂單 ID 生成策略：snowflake vs ULID vs DB sequence？
4. 是否支援 stop-limit / trailing-stop 等高階訂單類型（首發版不需要）？
5. ~~DMM API 是否走獨立 binary protocol（FIX 協議）還是共用 REST？~~（→ SPEC-000 Q-8，已 Resolved by SPEC-009 §3.4：自建 binary protobuf over TLS）
