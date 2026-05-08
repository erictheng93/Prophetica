# SPEC-003 — 撮合引擎詳規（Matching Engine）

> 優先級：**P0**
> 版本：v0.2.3 · 最後更新：2026-05-07
> 依賴：SPEC-002
> 被依賴：SPEC-004, SPEC-007, SPEC-009

> **v0.2.3 變更**：(1) **T-4 hostNetwork 市場路由機制落地**：§17.2 重寫為「市場路由與分配機制」，含 etcd-driven consistent hash routing / 路由架構圖 / 路由決策表（5 種觸發場景）/ 跨服務銜接 / MVP-Scale-up-Scale-out 演進路徑；補 SPEC-007 §6.4 之 etcd lookup 0.5ms hop budget；(2) **T-6 Thread / Runtime 模型分流**：§17.3 重寫，明示「熱市場（前 5%）獨佔 node + current_thread runtime + core pinning」vs「冷市場 multiplex node + multi-thread runtime + work-stealing」之分流策略；含動態升降級流程與 SPEC-017 §6.1.1 範例對齊註記（兩個 binary 或單 binary env-dispatched）；(3) §17.5 升級流程補「升級期間凍結 reroute 決策」防止誤判。
> **v0.2.2 變更（R-2 紅線清除）**：依業主決議移除 §12.1.1 sentinel `system.platform_announcement` (uid=4)，連帶 §12.1.4「例外情況」整段內容刪除（改為「**無**」）；新增 `RESERVED_REMOVED_USER_IDS` 集合與兩個 unit test 防止 uid=4 未來被重用；對齊 SPEC-009 §5 整段移除冷啟動補貼。對應 [SPEC-000 §5 R-2](./SPEC-000-index.md)；ADR-0001 §6 「唯一例外」描述待新 ADR 更新為「無例外」。
> **v0.2.1 變更**：(1) §12 加 hedge 允許註記（呼應 [SPEC-000 §5 Q-7](./SPEC-000-index.md) Resolved），明示**不**加 `HEDGE_NOT_ALLOWED` pre-trade 檢查；(2) §12.1.1 sentinel 常數 / §12.1.2 引擎入口檢查 / §12.1.3 CI 靜態掃描三節之 Go 程式碼樣板改寫為 Rust（sorted const slice + `binary_search_by_key`、`tracing::error` audit log、`rg --type rust`），對齊 SPEC-014 v0.2.0 後端切 Rust；(3) §15 `orderbook.rs` Rust 範例取代原 `internal/engine/orderbook.go` Go 範例；(4) §16 測試策略改 Rust 工具鏈（`cargo nextest` 取代 `goleak`、`proptest` 取代 `gopter`、新增 §16.6 mutation testing 對齊 SPEC-017 §3.3 R-4）；(5) §3.1 `Market` struct / §3.2 `Outcome` enum / §14.2 金額計算亦由 Go 改寫為 Rust（型別契約清理）。注意 §6 ~ §11 之 `\`\`\`go` 程式區塊仍保留為**演算法 pseudo-code**，由 §6 開頭註記說明，本次不動。
> **v0.2.0 變更**：跟隨 SPEC-014 後端切 Rust，本檔之 Go 程式碼示例改寫為 Rust（per-market goroutine → per-market tokio task；`type OrderID = uint64` → 之後直接落地為 ULID `[u8; 16]`，不再經過 PR #1 之 Go→Rust 過渡）。撮合熱路徑在 Rust 下進一步無 GC、無 runtime 抖動，§12 P99 < 5ms SLO 不變但目標達成路徑更直接。

---

## 1. Objective

設計一個 **單寫者、in-process、event-sourced** 的訂單簿撮合引擎，支援預測市場 YES/NO 雙腿合約的下單、撤單、成交、推播，達成 P99 < 5ms 的撮合延遲，並提供完整的事件流供下游帳本、結算、推播服務消費。

## 2. 設計原則

| 原則 | 內涵 |
|------|------|
| **單一寫者**（Single Writer） | 每個市場一個專屬 tokio task 獨佔該市場的訂單簿（per-market `mpsc::Receiver` 為唯一輸入），禁止鎖、禁止並發寫入 |
| **In-Process Memory** | 訂單簿完全在 Rust 進程記憶體，不經 Redis、不經網路 |
| **Event Sourcing** | 撮合引擎只產生事件，所有下游（帳本、推播、持倉、K 線）由消費者重建 |
| **WAL + Snapshot** | 寫入 append-only log（磁碟 + Kafka），定期 snapshot，crash recovery 從 snapshot + replay |
| **確定性**（Deterministic） | 給定相同事件序列，重建出的訂單簿狀態必須 100% 一致 |
| **Price-Time FIFO** | 撮合演算法為**價格優先 → 時間優先**，與 Kalshi 一致 |
| **平台不持倉** | 引擎無 house book，無自動造市，所有 quote 來自玩家或 DMM |

## 3. 合約模型

### 3.1 市場（Market）

每個市場代表一個 **二元結果（Binary Outcome）** 事件。本 struct 由 SPEC-015 §6 之 `markets` 表序列化而來，注入撮合引擎 in-memory（呼應 SPEC-015 §10.1）。

```rust
use chrono::{DateTime, Utc};

pub struct Market {
    pub id:                String,            // 'mkt_2026_election_winner'
    pub title:             String,
    pub description:       String,
    pub outcome_yes:       Outcome,           // YES 腿
    pub outcome_no:        Outcome,           // NO 腿
    pub opens_at:          DateTime<Utc>,
    pub closes_at:         DateTime<Utc>,     // 截止下單
    pub settles_at:        DateTime<Utc>,     // 預計結算
    pub status:            MarketStatus,      // Open / Closed / Settling / Settled / Cancelled
    pub oracle_source:     String,            // 結果來源
    pub min_tick_pts:      i64,               // 最小價格單位（內部單位百分點），預設 1 = 0.01 點
    pub min_lot:           i64,               // 最小股數，預設 1
    pub max_order_qty:     i64,               // 單筆最大股數，防作弊
    pub fee_bps:           u32,               // 手續費 basis points，預設 200 (=2%)
}
```

### 3.2 結果腿（Outcome）

```rust
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Yes = 0,
    No  = 1,
}

impl Outcome {
    pub const fn as_str(self) -> &'static str {
        match self { Outcome::Yes => "YES", Outcome::No => "NO" }
    }
}
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

```rust
// Canonical Order ID: ULID (128-bit, 已於 §19 OQ #3 拍板)
// Rust 下直接以 [u8; 16] 落地（serde / Display 包裝為 "ord_<ULID>" 字串）
pub type OrderID = [u8; 16];

#[repr(i8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Side {
    Bid = 1,   // 買 YES
    Ask = -1,  // 賣 YES
}

pub struct Order {
    pub id:              OrderID,
    pub user_id:         u64,
    pub market_id:       String,
    pub side:            Side,
    pub kind:            OrderType,       // Limit / Market / IOC / FOK / PostOnly
    pub price_pts:       i64,             // 內部單位（0-100, 對應 0.00 ~ 1.00）
    pub qty_total:       i64,             // 原始數量
    pub qty_remaining:   i64,             // 剩餘未成交
    pub status:          OrderStatus,
    pub submitted_at_ns: i64,             // unix nano，作為時間優先依據
    pub client_order_id: String,          // 玩家自定義 ID，用於冪等
    pub tif:             TimeInForce,     // GTC / IOC / FOK / DAY
    pub post_only:       bool,
}
```

## 5. 訂單簿資料結構

### 5.1 整體結構

每個 Market 一份訂單簿：

```rust
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::cmp::Reverse;

pub struct OrderBook {
    pub market_id: String,

    // 價格 → 該價位的訂單佇列
    // 採 BTreeMap（B-tree based），bid 以 Reverse(price) 為 key 達成由高到低
    pub bids: BTreeMap<Reverse<i64>, PriceLevel>,
    pub asks: BTreeMap<i64, PriceLevel>,

    // 訂單 ID → 訂單 handle，用於 O(1) 撤單
    pub orders_by_id: HashMap<OrderID, OrderHandle>,

    // 性能指標
    pub last_trade_pts: i64,
    pub best_bid_pts:   i64,
    pub best_ask_pts:   i64,
    pub seq_no:         u64,  // 嚴格遞增的事件序號
}

pub struct PriceLevel {
    pub price_pts: i64,
    pub queue:     VecDeque<OrderID>,  // FIFO；O(1) push_back / pop_front
    pub total_qty: i64,                // 該價位累計掛單量（cache）
}
```

### 5.2 為何 BTreeMap 而非 skip list / RB-tree？

| 結構 | 插入 | 刪除 | 範圍掃描 | 記憶體 | 評價 |
|------|------|------|---------|-------|------|
| sorted Vec | O(N) | O(N) | O(K) | 最低 | 慢 |
| RB-tree | O(logN) | O(logN) | O(K logN) | 中 | OK |
| skip list | O(logN) | O(logN) | O(K) | 高 | OK |
| **`std::collections::BTreeMap`** | O(logN) | O(logN) | O(K) | **低（cache friendly）** | **最佳** |

Rust 標準函式庫 `BTreeMap` 已是 B-tree（每節點多 entries，cache friendly），不需第三方 crate。撤單熱路徑透過 `OrderHandle`（記錄價位 + queue 中位置）達成 O(1)，避免 BTreeMap 重新查找。

### 5.3 為何 VecDeque（雙端佇列）而非 Vec？

每個 PriceLevel 內部訂單佇列：
- **撮合**：`pop_front()` → O(1)
- **加入**：`push_back()` → O(1)
- **撤單**：透過 `OrderHandle{price_pts, queue_index_or_marker}` 找到位置 → O(1) 邏輯刪除（tombstone），下次撮合時跳過

`Vec` 之中段刪除為 O(N)，不可行；`VecDeque` 之中段刪除亦 O(N)，故撤單採 tombstone 模式。

## 6. 撮合演算法

> **語言註記（v0.2.0）**：以下 §6 / §7 / §8 / §10 / §11 之 ```go ``` 程式區塊為**演算法說明用的 pseudo-code**，沿用 v0.1 之 Go 風格表述。canonical 型別定義已於 §4.3 改為 Rust（`OrderID = [u8; 16]`、`Order` struct、`Side` enum），實作以 Rust 為準（per-market `tokio::task` 取代 goroutine、`mpsc::Sender` 取代 channel、`BTreeMap` 取代 google/btree）。SPEC-017 之 PR #1 落地時依本節邏輯翻譯為 Rust，演算法本身不變。

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
CPU Core 0: tokio task for Market A（pinned via runtime affinity）
CPU Core 1: tokio task for Market B
CPU Core 2: tokio task for Market C
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

> **Hedge 允許**：本表**不**包含 `HEDGE_NOT_ALLOWED` 檢查（呼應 [SPEC-000 §5 Q-7](./SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker) Resolved）。同一玩家在同一市場同時持 YES + NO 為合法業務場景，由 SPEC-002 §4.4.4 NOTE 3 之逐 `position_id` 結算邏輯支撐；潛在 wash trading 由 SPEC-010 §4.2.2 post-trade 偵測攔截，刻意不在 hot path pre-trade 檢查（避免每張訂單 query position-service 增加 P99 1 hop）。

風控前置在獨立 service 完成，撮合引擎假設輸入已通過。

### 12.1 PLATFORM_SYSTEM_USER_IDS Sentinel Set（Compliance 紅線）

> **強制紅線**：呼應 [ADR-0001 §6](../docs/architecture/decisions/ADR-0001-order-book-with-dmm-over-lmsr.md)。引擎 `ingressCh` 必須對 incoming order 執行 `order.user_id != PLATFORM_SYSTEM_USER_IDS` 檢查；命中則直接 reject 並寫入 SIEM。任何將平台 / 系統 user_id 加入下單能力之 PR **必須 CI fail + reject**。

#### 12.1.1 常數定義（Rust）

```rust
// services/matching-engine/src/sentinel.rs

//! `PLATFORM_SYSTEM_USER_IDS` 列舉所有「禁止下單」之系統 user_id。
//! 任何 incoming order 的 `user_id` 命中本集合則直接拒絕。
//!
//! # 紅線理由
//!   - 平台不得在自家訂單簿掛單
//!   - ADR-0001 §6 列為靜態掃描禁字
//!
//! # 新增 user_id 至本集合需要
//!   1. ADR-XXXX 簽核
//!   2. Compliance Officer 4-eye review（呼應 SPEC-010 §9.3 MS-14）
//!   3. 與本檔案的 git blame 對照可追溯

/// Sentinel `user_id` → 用途註記。Sorted const slice，O(log N) lookup，零 alloc。
///
/// 註：uid=4 (`system.platform_announcement`) 已於 v0.2.2 移除（呼應 SPEC-009 §5 R-2 紅線清除）。
/// 集合刻意保留 id 4 之 gap，**不**重新編號 5/6/7/8 為 4/5/6/7，避免歷史 audit log 與 DB 引用斷鏈。
/// `binary_search_by_key` 對 sorted slice with gap 仍正確運作。
pub const PLATFORM_SYSTEM_USER_IDS: &[(u64, &str)] = &[
    (1, "system.root"),                  // bootstrap / migration only, never used at runtime
    (2, "system.compliance"),            // 凍結 / 解凍帳戶之 audit shim, never trades
    (3, "system.settlement_oracle"),     // Oracle 提交 shim, never trades
    // uid=4 reserved (formerly system.platform_announcement; 已移除，永不重用)
    (5, "system.cs_compensation"),       // 客服補償帳戶，呼應 SPEC-002 §8
    (6, "system.suspense"),              // 對帳暫掛，呼應 SPEC-002 §4.2
    (7, "system.affiliate_pool"),        // 推薦獎勵發放暫存池，呼應 SPEC-011
    (8, "system.dmm_rebate_pool"),       // DMM 補貼暫存池，呼應 SPEC-009
];

/// `RESERVED_REMOVED_USER_IDS` 列舉曾經存在但已正式移除之 sentinel id；
/// 任何嘗試重新註冊此集合中的 id（即便用途不同）必須 reject + 寫 critical audit log，
/// 避免歷史 ledger / WAL 中之 `user_id=4` 紀錄被誤認為新 sentinel。
pub const RESERVED_REMOVED_USER_IDS: &[(u64, &str)] = &[
    (4, "formerly system.platform_announcement, removed 2026-05-07 per R-2"),
];

/// Returns `true` iff the given `user_id` is reserved as a platform sentinel
/// and must **never** appear as an order placer.
#[inline]
pub fn is_platform_system_user(uid: u64) -> bool {
    PLATFORM_SYSTEM_USER_IDS
        .binary_search_by_key(&uid, |&(id, _)| id)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_ids_are_sorted() {
        // binary_search_by_key 之正確性前提：必須維持 sorted
        let ids: Vec<u64> = PLATFORM_SYSTEM_USER_IDS.iter().map(|&(id, _)| id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "PLATFORM_SYSTEM_USER_IDS must be sorted by user_id");
    }

    #[test]
    fn sentinel_ids_are_unique() {
        use std::collections::HashSet;
        let set: HashSet<u64> = PLATFORM_SYSTEM_USER_IDS.iter().map(|&(id, _)| id).collect();
        assert_eq!(set.len(), PLATFORM_SYSTEM_USER_IDS.len(), "duplicate sentinel user_id");
    }

    #[test]
    fn ordinary_user_is_not_sentinel() {
        assert!(!is_platform_system_user(42));
        assert!(!is_platform_system_user(1_000_000));
    }

    #[test]
    fn all_sentinel_ids_match() {
        for &(id, _) in PLATFORM_SYSTEM_USER_IDS {
            assert!(is_platform_system_user(id));
        }
    }

    #[test]
    fn removed_uid_4_is_not_active_sentinel() {
        // R-2: system.platform_announcement (uid=4) removed; must NOT be active.
        // 但仍視為「系統保留」— 不應被一般 user 註冊使用。
        assert!(!is_platform_system_user(4),
            "uid=4 已於 v0.2.2 移除，不應作為 active sentinel");
        assert!(RESERVED_REMOVED_USER_IDS.iter().any(|&(id, _)| id == 4),
            "uid=4 必須記錄於 RESERVED_REMOVED_USER_IDS 防止重用");
    }

    #[test]
    fn reserved_removed_ids_do_not_overlap_with_active() {
        let active: std::collections::HashSet<u64> =
            PLATFORM_SYSTEM_USER_IDS.iter().map(|&(id, _)| id).collect();
        for &(id, _) in RESERVED_REMOVED_USER_IDS {
            assert!(!active.contains(&id),
                "removed uid {} must not reappear in active sentinel set", id);
        }
    }
}
```

#### 12.1.2 引擎入口檢查（強制）

```rust
// services/matching-engine/src/ingress.rs

use tracing::error;

use crate::events::{OrderRejected, RejectReason};
use crate::metrics;
use crate::sentinel::is_platform_system_user;
use crate::shard::MarketShard;
use crate::types::Command;

impl MarketShard {
    pub async fn handle_command(&mut self, cmd: Command) {
        if let Command::PlaceOrder(order) = &cmd {
            if is_platform_system_user(order.user_id) {
                // 立即 reject，並產生 critical 等級 audit log
                self.publish_event(OrderRejected {
                    order_id: order.id,
                    reason: RejectReason::PlatformUserNotAllowed,
                });
                metrics::PLATFORM_USER_ORDER_BLOCKED.inc();
                error!(
                    target: "audit.critical",
                    user_id = order.user_id,
                    order_id = ?order.id,
                    market_id = %order.market_id,
                    "platform sentinel user attempted order"
                );
                return;
            }
        }
        // ... 進入正常撮合
        self.dispatch(cmd).await;
    }
}
```

#### 12.1.3 CI 靜態掃描

呼應 ADR-0001 §6 與 SPEC-017 §8 sentinel-scan job，CI pipeline 必含以下檢查（在 SPEC-013 §3.2 列為阻斷 gate）：

```bash
# .github/workflows/sentinel-scan.yml （示意；對齊 SPEC-017 §8 sentinel-scan job）
# 1. 禁字掃描（ADR-0001 §6）
rg -n -e 'auto_quote|market_balancer|house_book|probability_symmetry|auto_hedge' \
   --type rust services/ crates/ \
   && echo "ADR-0001 §6 forbidden words detected" && exit 1

# 2. PLATFORM_SYSTEM_USER_IDS 集合不得在 sentinel / ingress 路徑外被旁路
rg -n -e 'PLATFORM_SYSTEM_USER_IDS' --type rust services/matching-engine/ \
   | grep -vE '(sentinel\.rs|ingress\.rs|sentinel/tests\.rs|tests/sentinel)' \
   && echo "PLATFORM_SYSTEM_USER_IDS referenced outside whitelisted paths" && exit 1

# 3. DMM session 檢查：DMM order 進入 engine 前必須轉換 user_id 為 DMM 法人帳戶
#    （SPEC-009 §3.4），不可使用 system user_id
rg -n -e 'PLATFORM_SYSTEM_USER_IDS' --type rust services/dmm-gateway/ \
   && echo "DMM gateway must not reference platform sentinel users" && exit 1
```

#### 12.1.4 例外情況：**無**

> **本節原描述「`system.platform_announcement` (uid=4) 為唯一例外」之冷啟動補貼路徑已於 v0.2.2 整段移除**（呼應 [SPEC-000 §5 R-2](./SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker) 業主決議與 SPEC-009 §5 同步）。
>
> 平台**永遠不**在自家訂單簿掛單，**無**任何 feature flag、簽核流程、或 UI 標記可以啟用平台 order。
>
> §12.1.1 sentinel 集合中 uid=4 已移除，並列入 `RESERVED_REMOVED_USER_IDS` 防止歷史紀錄與未來 user_id 撞號；§12.1.3 CI sentinel-scan 不需特別 whitelist。冷啟動流動性不足走 SPEC-009 §3.3 DMM rebate 放大或市場延後上架。
>
> ADR-0001 §6 「唯一例外」描述待新 ADR 標記為 superseded（後續落地）。

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

```rust
// trade value (in 點 internal units, 100 = 1 點)
// price_pts: 0-100, qty: 股數
// trade_value = price_pts * qty  →  internal pts × 1
// 例: 65 × 100 = 6500 internal pts = 65.00 點
let trade_value: i64 = i64::from(price_pts) * qty;

// fee = trade_value * fee_bps / 10000
let fee: i64 = trade_value * i64::from(fee_bps) / 10_000;
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

```rust
// services/matching-engine/src/orderbook.rs

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};

use crate::events::{Event, OrderAccepted, OrderResting};
use crate::types::{Order, OrderId, OrderType, PriceLevel, Side};

pub struct OrderBook {
    market_id: String,
    bids: BTreeMap<Reverse<i64>, PriceLevel>,
    asks: BTreeMap<i64, PriceLevel>,
    orders_by_id: HashMap<OrderId, OrderHandle>,
    seq: u64,
}

struct OrderHandle {
    price_pts: i64,
    side:      Side,
    /// VecDeque tombstone marker — 撤單時不真的移除（O(N)），
    /// 改 mark active=false，撮合時跳過已 cancel。
    active:    bool,
}

impl OrderBook {
    /// `apply_order` is the **only** mutator. It must be called from the
    /// market's dedicated `tokio::task::LocalSet` runtime — no `Mutex`,
    /// no `RwLock`, no `Arc<RefCell>`. Returns events in deterministic order.
    pub fn apply_order(&mut self, mut order: Order) -> Vec<Event> {
        self.seq += 1;
        order.seq_no = self.seq;

        let mut events = Vec::with_capacity(4);
        events.push(Event::OrderAccepted(OrderAccepted {
            seq: self.seq,
            order_id: order.id,
        }));

        while order.qty_remaining > 0 {
            let Some(best) = self.peek_best_opposite(order.side) else { break };
            if !can_cross(&order, best) { break; }
            events.extend(self.match_at_level(&mut order));
        }

        if order.qty_remaining > 0 && order.kind == OrderType::Limit {
            self.insert_resting(&order);
            events.push(Event::OrderResting(OrderResting { order_id: order.id }));
        }
        events
    }
}

fn can_cross(taker: &Order, level: &PriceLevel) -> bool {
    match (taker.kind, taker.side) {
        (OrderType::Market, _) => true,
        (_, Side::Bid) => taker.price_pts >= level.price_pts,
        (_, Side::Ask) => taker.price_pts <= level.price_pts,
    }
}
```

## 16. 測試策略

### 16.1 單元測試（cargo nextest）

- 對應檔案：`services/matching-engine/src/orderbook.rs` 之 `#[cfg(test)] mod tests`
- 撮合 hot path crate（`book` / `match` 模組）line coverage **≥ 95% baseline**（呼應 [SPEC-013 §3.3](./SPEC-013-cicd-release.md) 分層門檻 + §18 整體 ≥ 95%；H-6 跨檔對齊）；剩餘 ≤ 5% 須為**經 ADR 標註之 unreachable 分支**（如 `unreachable!()` / `panic!("invariant violated")`），不可為缺測之業務 path。stretch goal 100%（追蹤剩餘 5% 之 dead code / error path 並逐筆消除）
- 涵蓋情境：
  - 撮合 happy path（單筆 / 多檔位 sweep）
  - 自我成交防止（STP 各策略：CANCEL_NEWEST / CANCEL_OLDEST / CANCEL_BOTH / DECREMENT_BOTH）
  - IOC / FOK / POST_ONLY 行為
  - 撤單（resting / partial fill 後撤 / 不存在 / 已 FILLED）
- runner：`cargo nextest run -p matching-engine --locked`（對齊 SPEC-017 §4.5 Makefile 之 `make test`）

### 16.2 屬性測試（[`proptest`](https://docs.rs/proptest)）

針對任意訂單序列，不變式皆成立：

- `Σ(bid qty) + Σ(ask qty) + Σ(filled qty) == Σ(submitted qty)`
- 任意時刻 `best_bid < best_ask`（無交叉）
- `seq` 嚴格遞增且無 gap
- 撤單後 `total_qty` 與 queue 長度一致
- POST_ONLY 不可成交，違反必觸發 `OrderRejected`

採 `proptest::prop_compose!` 生成 `Order` 序列，shrink 失敗最小集；CI 跑種子固定（`PROPTEST_SEED`）以利重現。

### 16.3 重放確定性測試

- 從 WAL 重放歷史事件，重建狀態必須與 snapshot byte-equal（對 `bincode` / `rmp-serde` 序列化結果做 SHA256 比對）
- 跨 build 之 determinism：clippy `clippy::float_arithmetic = "deny"`（禁 float），確保不同 CPU 不會給出不同結果

### 16.4 壓力測試

- 100 萬 orders、1 個市場：吞吐、P99 latency 達 §11 SLO
- 100 個市場、每市場 10k orders：跨核擴展性（per-market `tokio::task::LocalSet` + core_affinity，呼應 SPEC-014 §3.1 R-3）
- 工具：`criterion` benchmark + `tokio-console` 觀測 task starvation

### 16.5 混沌測試

- kill -9 + 自動 recovery：30 秒內必須完整恢復（從 snapshot + WAL replay）
- 磁碟滿：必須拒收新訂單，已接受訂單持續處理至 graceful shutdown
- 網路分區：Kafka 不可達時 WAL 仍寫入磁碟，待 Kafka 恢復後 producer 自動補送（呼應 SPEC-008 §4.2）

### 16.6 Mutation testing（nightly）

- 由 SPEC-017 §8.2 nightly workflow 限縮跑 `crates/sdk/ledger` + `services/matching-engine/src/{book,match}` + `services/wallet-ledger-service/src/ledger`
- 門檻：caught mutation rate ≥ 75%（呼應 SPEC-017 §3.3 R-4）
- 不阻塞 PR；持續 < 75% 開 issue 補測試

## 17. 部署與運維

### 17.1 部署模式

- 撮合引擎以 **DaemonSet + hostNetwork: true** 部署在 K8s
- 或：bare-metal Linux server，systemd 管理
- 不放在 sidecar / sidecar mesh 內（避免 Envoy 引入延遲）

### 17.2 市場路由與分配機制（T-4）

> 撮合引擎走 hostNetwork DaemonSet 模式（§17.1），意味著每個 K8s node 上有一個撮合 process；不能用標準 K8s Service load balancer，需自訂市場路由機制。

#### 17.2.1 路由架構

```
                                     ┌─────────────────────┐
                                     │   etcd cluster      │
                                     │  /market-routing/   │
                                     │  {market_id} = node │
                                     └──────────┬──────────┘
                                                │
                                                │ watch
                                                ▼
                  ┌──────────────────────────────────────────────────────┐
                  │             market-service                            │
                  │    (consistent_hash with virtual nodes; 24×replicas)  │
                  │     publishes routing decisions to etcd               │
                  └──────────┬─────────────────────┬─────────────────────┘
                             │                     │
              POST /orders   │ lookup              │ event: market.routing.changed
                             ▼                     ▼
                ┌────────────────────┐   ┌────────────────────┐
                │   api-gateway      │   │  matching-engine   │
                │  forwards to       │   │  pod on selected   │
                │  designated node   │   │  node (hostPort)   │
                └────────────────────┘   └────────────────────┘
```

#### 17.2.2 路由決策

| 階段 | 機制 |
|------|------|
| **市場創建（DRAFT → SCHEDULED）**| market-service 計算 `consistent_hash(market_id, virtual_nodes=24)` 對應到當前 active node 集合；寫入 etcd `/market-routing/{market_id} = {node_ip, hostPort}` |
| **市場開盤（SCHEDULED → OPEN）**| matching-engine pod watch etcd 該 key，被指派之 pod 從 SPEC-008 §6.1 snapshot + WAL 還原 OrderBook，載入 in-memory；ingress ready 後寫 `/market-routing/{market_id}/state = ready` |
| **下單路由**| api-gateway 查 etcd（cached 30s）取得目標 node IP + hostPort；轉發 gRPC 至該 node 之撮合引擎；若 etcd state 不為 ready，回 503 + Retry-After header |
| **node failure**| Kubernetes node failure detector（kubelet liveness）+ market-service 同步偵測 etcd lease expiry → 觸發該 node 之所有市場 reroute；新 node 從 snapshot + Kafka WAL replay 還原（呼應 SPEC-008 §4.2 RTO < 5 分鐘）|
| **node 添加（scale up）**| 新 node 加入後，market-service 重新計算 consistent hash；**僅** rebalance 受影響之 1/N 市場（virtual nodes 設計使 rebalance 量最小）；rebalance 時市場進入 `MIGRATING` 狀態，拒收新 order 直至新 node ready |
| **熱 / 冷市場分類**| 每日 cron 計算前 24 小時各市場成交量；前 5% 之熱市場標記為 `hot`，獨佔一個 K8s node（hostPort + node taint + affinity）；冷市場 multiplex 共享 node |

#### 17.2.3 跨服務銜接

- market-service 之路由決策邏輯位於 `services/market-service/src/routing.rs`
- etcd client 採 `etcd-client` crate（tokio-async）
- routing 變更事件透過 Kafka topic `market.routing` broadcast（呼應 SPEC-014 §6.2）
- 路由決策之 hot path P99 budget：api-gateway → etcd cache lookup ≤ 0.5ms（呼應 SPEC-007 §6.4 將 hop budget 補為 etcd lookup 之 0.5ms 而非 default 0ms）

#### 17.2.4 演進路徑

- **MVP（< 100 markets）**：consistent hash + 單 etcd cluster
- **Scale-up（100-1000 markets）**：考慮加 routing cache 至 api-gateway（Redis）；reduce etcd watch fan-out
- **Scale-out（> 1000 markets）**：考慮多區 etcd（federation）；本 SPEC 不細化，留待規模確認後新 ADR

### 17.3 Thread / Runtime 模型（T-6）

> SPEC-017 §6.1.1 範例之「每 market 一個 thread + core_affinity pin」假設**僅在熱市場 ≤ CPU cores 時成立**。實務上 100 market on 16 cores 時，`core_affinity::set_for_current` 退化為「群組親和性」，多 thread 仍會被 OS scheduler 在共用 core 上切換，P99 抖動仍會發生。本節定義熱 / 冷市場分流策略。

#### 17.3.1 熱市場（trade volume > 閾值，標記為 `hot`）

- **獨佔 K8s node**：透過 §17.2.2 hot 標記 + node taint + pod affinity，確保該 node 上僅運行該熱市場的 matching-engine pod
- **單一 process + tokio current_thread runtime + LocalSet + core_affinity**：呼應 SPEC-017 §6.1.1 / SPEC-014 R-3 原始設計
- **CPU pinning**：thread pin 到 node 上的單一 core（透過 `taskset` 或 K8s `cpuManagerPolicy: static`）
- **適用場景**：成交量前 5% 之熱市場，P99 對 tail latency 敏感
- **預估規模**：MVP 階段熱市場 ≤ 10 個（每個獨佔一台中等規格 node：8 vCPU / 16GB / NVMe SSD）

#### 17.3.2 冷市場（其餘 95%）

- **multiplex 共享 node**：每 node 運行一個 matching-engine pod，承載多個冷市場
- **tokio multi-thread runtime + per-market `tokio::task`**（不採 LocalSet，避免 OS thread 數爆炸）
- **無 CPU pinning**：work-stealing 適用於冷市場（成交量不集中、scheduling jitter 影響可忽略）
- **適用場景**：長尾市場、剛上架市場、低成交量市場
- **預估規模**：MVP 階段每 node 承載 ~50 冷市場

#### 17.3.3 動態升降級

- **冷 → 熱**：每日 cron 偵測前 24 小時成交量 ≥ 熱市場閾值 → 自動觸發 promotion 流程：(a) 新 node 啟動專屬 pod；(b) snapshot + WAL replay 還原 OrderBook；(c) 流量切換；(d) 舊 node 上該市場 evict
- **熱 → 冷**：連續 7 天成交量低於熱閾值 → 自動 demotion，反向流程
- **promotion / demotion governance**：每次升降級寫 audit log，week-over-week 報表給 Compliance

#### 17.3.4 SPEC-017 §6.1.1 之範例對齊

SPEC-017 §6.1.1 之 `main.rs` 範例（每 market 一 thread + core_affinity）對應**熱市場**之撮合引擎 binary。冷市場 binary 為**獨立 main.rs**，採 `#[tokio::main]` multi-thread runtime + `tokio::spawn` per-market task。兩個 binary 共用 `crates/sdk` + `services/matching-engine/src/{book,match}` 核心邏輯。

實作落地：
- `services/matching-engine-hot/src/main.rs` — 熱市場 binary
- `services/matching-engine-cold/src/main.rs` — 冷市場 binary
- `services/matching-engine/` — 共用 core lib crate（不含 main.rs）

或者單一 binary 透過 env var (`PROPH_ENGINE_MODE=hot|cold`) 在 main.rs 啟動時 dispatch — 取決於 Codex PR #1 之拆分粒度決議；SPEC-017 §11.2 acceptance check 應反映此選擇。

### 17.5 升級

- 滾動升級：新機器啟動 → 從 snapshot + WAL 恢復 → ingress 切換 → 舊機器排乾
- 強制 graceful shutdown：拒收新 order，處理完佇列後 snapshot 並退出
- 升級期間市場路由由 market-service 暫時凍結 reroute 決策（避免升級中誤判 node failure 觸發不必要 rebalance）

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
3. ~~訂單 ID 生成策略：snowflake vs ULID vs DB sequence？~~ ✅ **Resolved 2026-05-05**：採 **ULID** 為 canonical Order.ID。理由：
   - **無需協調**：snowflake 需 worker ID 分配（撮合引擎為 per-market goroutine，DaemonSet 部署時 worker ID 配置成本高）；DB sequence 在撮合熱路徑要往返 Postgres（破壞 single-writer in-memory 性能）。ULID 由撮合引擎本地生成，zero-coordination。
   - **時序友善**：ULID 前 48 bit 為毫秒 timestamp，字典序自然遞增，方便 log / debug / 範圍查詢；同毫秒內 monotonic flag 確保唯一順序。
   - **128-bit 無碰撞**：跨 market goroutine 同毫秒生成不會撞，撮合引擎 crash recovery 重啟後 ID 不重複（不像 in-memory uint64 sequence 需從 WAL 還原）。
   - **API / DB / log 一致**：對外 API 已用 `ord_01H...` 字串格式（§9 API 範例），與 ULID Crockford Base32 編碼天然契合。DB 儲 `bytea(16)` 或 PostgreSQL `uuid` 型別（ULID 為 UUID 相容 128-bit）。
   - **撮合 FIFO 優先序仍用 per-market `seq uint64`**：ULID 作為持久 ID 與 idempotency key；撮合引擎內部排序仍以 leader 賦予的 `seq` 為準（呼應 §4.3 `OrderResting{Seq: b.seq}`），兩者並存。
   - **落地動作**：(a) `packages/proto/v1/engine.proto` 之 `OrderID` 定義為 `bytes order_id = 1`（節省序列化成本）；(b) §4.3 `OrderID = [u8; 16]` 為 canonical Rust 型別；Display 包裝輸出 `ord_<ULID>` 字串；(c) Rust 端採 [`ulid` crate 1.x](https://docs.rs/ulid)（撮合引擎自管 entropy source，per-market `Generator` 確保 monotonic）。
4. 是否支援 stop-limit / trailing-stop 等高階訂單類型（首發版不需要）？
5. ~~DMM API 是否走獨立 binary protocol（FIX 協議）還是共用 REST？~~（已 Resolved by [SPEC-009 §3.4](./SPEC-009-dmm-liquidity.md)：自建 binary protobuf over TLS）
