# SPEC-002 — 資金安全與虛擬點數帳本

> 優先級：**P0**  
> 版本：v0.1.7 · 最後更新：2026-05-07  
> 依賴：SPEC-001  
> 被依賴：SPEC-003, SPEC-004, SPEC-005, SPEC-011

> **v0.1.7 變更**：(1) §5 INV-6 governance 標註更新 — [ADR-0006](../docs/architecture/decisions/ADR-0006-ledger-invariant-set-extension-inv6.md) 已落地（Accepted 2026-05-07），擴展 ADR-0003 §5 INV 集合為 INV-1 ~ INV-6；移除 v0.1.5 「待 ADR-0006 落地」與「migration 不得加 CHECK constraint」之過渡標註，wallet-ledger-service migration 可依 ADR-0006 §4.2.1 plpgsql template 實作 INV-6 trigger。(2) **T-3 partition 策略**：§12 OQ #2 由「Phase 1 不分區後 migrate」修正為「Phase 1 從 day 1 即採 monthly range partition by `created_at`（pg_partman）+ BRIN index + 12 個月以上 cold partition detach 至 R2 + foreign table」，避免事後 online migration 高風險操作。(3) **T-5 最終一致性窗口**：§9 新增 §9.3 「撮合 → ledger 最終一致性窗口」，明示 ~100ms eventual consistency 之語意 + 前端 optimistic UI 處理 + 後端 API 提供 `?at_seq=N` / `?consistent=true` 兩種一致性語意 + OTel span 監控（`proph_ledger_event_lag_seconds`）。
> **v0.1.6 變更（H-5）**：§8.1 chargeback 場景補「累計 ≥ 1 立即永久封號 + 黑名單」門檻，cross-ref SPEC-010 §6.2 / SPEC-016 §6.1；系統故障 reversal 補 cross-ref 至 SPEC-010 §9.3 MS-6（4-eye）。
> **v0.1.5 變更（R-1 紅線清除）**：(1) §4.2 Chart of Accounts 補新子科目 `Liability.AccountsPayable.{partner_id}`（B2B 業務應付）/ `Expense.AffiliatePartnerFee`（業務現金酬勞）/ `Bank.OperatingAccount.{currency}`（營運帳戶，禁止接收玩家儲值），三者皆與玩家錢包零交集；(2) §5 新增 INV-6：`Player.Wallet.*` outflow 目的地白名單，禁止流向應付帳款 / 營運帳戶 / 業務酬勞等任何「兌現」科目，由 DB CONSTRAINT TRIGGER 強制 + CI sentinel-scan 雙保險；(3) INV-6 ADR governance：code 實作前需先落地 ADR-0006 將 ADR-0003 §5 之 INV 集合從 INV-1 ~ INV-5 擴展為 INV-1 ~ INV-6。對應 [SPEC-000 §5 R-1](./SPEC-000-index.md) 與 SPEC-011 §6.3。
> **v0.1.4 變更**：§4.4.4 NOTE 3 加 cross-ref 至 [SPEC-000 §5 Q-7](./SPEC-000-index.md)，明示 hedge 政策已 Resolved 為「允許」；本 SPEC 既有逐 `position_id` 結算邏輯維持不變，現由 Q-7 正式背書。
> **v0.1.3 變更**：§3.2 內部儲存型別由 Go `int64` 同步為 Rust `i64`（語意等同）；對齊 SPEC-014 v0.2.0 後端切 Rust。
> **v0.1.2 變更**：§12 OQ #2 `ledger_entries` partition 策略 Resolved（Phase 1 不分區 + 觸發門檻 + monthly partition 為未來方案）；§3.1 國際化說明明示永遠單幣別 TWD，多幣別已無啟用計畫。
> **v0.1.1 變更**：(1) §3.1 點數計價 1 點 = TWD 1 由暫定改正式定案；(2) §4.2 帳戶體系新增 `Player.Frozen.{user_id}`；(3) §4.2.1 新章節定義凍結子科目流程；(4) §5 INV-2 公式擴展含 Frozen；(5) §12 OQ #1（ledger 獨立 service）與 OQ #4（凍結處理）Resolved。

---

## 1. Objective

確保玩家資金（法幣儲值）與站內虛擬點數的金流，達到金融級的精確、可稽核、雙簿記、零誤差。任何時刻系統都能回答：「現在玩家總計擁有多少點數？平台託管帳戶有多少資金？兩者是否相符？」

## 2. 資金分層架構

```text
┌────────────────────────────────────────────────────────────────────┐
│ Player                                                              │
│   ↓ 法幣（TWD）                                                     │
│ Payment Gateway (綠界 / 藍新 / Stripe / etc.)                       │
│   ↓ webhook 確認                                                    │
│ Operating Co. 託管銀行帳戶 (Segregated Trust Account)               │
│   ↓ 內部記帳                                                        │
│ Ledger Service (PostgreSQL，雙簿記)                                  │
│   ↓ 對應                                                            │
│ Player Wallet (站內虛擬點數)                                         │
│   ↓ 鎖倉 / 釋出                                                     │
│ Order Book / Position / Settlement                                  │
└────────────────────────────────────────────────────────────────────┘
```

## 3. 點數計價設計

### 3.1 計價單位選定

> 決策：**1 點 = TWD 1 等值（依儲值當下匯率鎖定）** — ✅ **正式定案 2026-05-05（業主拍板）**，此前為暫定方案；對應 [SPEC-000 §5 Q-3](./SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker) 已 Resolved。

雖然 1:1 容易與現金混淆，但抽象比例（如 100 點 = TWD 1）會增加用戶心智負擔，且國際市場匯率變動會導致「持有的點數變多變少」的詭異現象。**統一以 TWD 為計算基準，UI 顯示「點」即可，避免 $ 符號**。

> 國際化考量：當前**永遠以 TWD 為唯一計價幣別**；國際玩家以非 TWD 儲值時於**通路端**換算為 TWD 後入帳，平台 ledger 不持非 TWD 資產。多幣別 ledger（SPEC-016 §7 Phase 2）為保留設計，**目前無啟用計畫**，僅在業務需要（Op Co. 開非 TWD 計價國際版本 / 主動承擔 FX 對沖）時才啟動 schema migration。

### 3.2 內部精度與整數化（Tech Invariant）

> 為避免浮點誤差於累加 / 對帳時放大，所有金額在系統內部**一律以整數儲存與運算**。本節是後端工程之硬性約束。

| 屬性 | 規則 |
|------|------|
| 對外顯示最小單位 | `0.01 點`（兩位小數）|
| 內部儲存型別 | `BIGINT`（PostgreSQL）/ `i64`（Rust）|
| 內部單位 | `1 內部單位 = 0.01 點`（即「點 × 100」）|
| 顯示換算 | `display_value = internal_value / 100`，UI 層格式化為 `fmt.Sprintf("%.2f 點", ...)` |
| 手續費 basis points | `1 bp = 0.01%`，計算 `fee = trade_value_internal × fee_bps / 10000`（呼應 SPEC-003 §14.2）|
| 禁區 | **絕不**使用 `float64` / `decimal` SaaS / 字串相加；任何轉型為浮點之 PR 必須 reject |

#### 3.2.1 整個系統的單位一致性

| 模組 | 欄位 / 變數 | 單位 |
|------|-----------|------|
| `ledger_entries.amount` | 內部點數 × 100 | BIGINT |
| `ledger_entries.balance_after` | 內部點數 × 100 | BIGINT |
| `Order.PricePts`（SPEC-003）| 0–100，對應 0.00–1.00（= 機率 × 100）| int64 |
| `Order.QtyTotal` / `QtyRemaining`（SPEC-003）| 股數 | int64 |
| Trade Value | `PricePts × Qty`（內部點數 × 100）| int64 |
| 點數結算 payout | `1.00 × 100 × Qty = 100 × Qty`（每股中獎 1 點）| int64 |

> 此一致性確保 SPEC-002 ledger entry 與 SPEC-003 撮合引擎共用同一套金額算術，無轉型損失。

## 4. 雙簿記帳本（Double-Entry Ledger）

### 4.1 為何必須雙簿記

單簿記（單純加減 wallet 餘額）的致命問題：
- 任何 bug 導致的「憑空產生點數」無法事後察覺
- 無法回答「這筆點數從哪裡來，要往哪裡去」
- 對帳時無法定位差異點

雙簿記原則：**每一筆點數變動，必有等額相反方向的另一筆紀錄**。

### 4.2 帳戶體系（Chart of Accounts）

```text
資產（Assets）
  └─ Bank.TrustAccount.TWD       託管銀行 TWD 餘額

負債（Liabilities）
  └─ Player.Wallet.{user_id}     對玩家的點數負債（每個玩家一個帳戶）
  └─ Player.Locked.{user_id}     鎖倉中（掛單未成交、結算中）
  └─ Player.Frozen.{user_id}     已凍結（責任博弈自我排除 / Compliance 凍結 / EDD 待審）— 詳見 §4.2.1

負債 — 應付帳款（Liabilities — Accounts Payable）
  └─ Liability.AccountsPayable.{partner_id}   B2B 業務合約應付（與玩家錢包零交集，呼應 SPEC-011 §6.4）

收入（Revenue）
  └─ Revenue.TradingFee          交易手續費收入
  └─ Revenue.SettlementFee       結算手續費收入

費用（Expenses）
  └─ Expense.PaymentGatewayFee   支付通路手續費
  └─ Expense.AffiliateCommission 玩家介紹獎勵 / 交易佣金（發放至 Player.Wallet 點數，呼應 SPEC-011 §4.1 / §4.2）
  └─ Expense.AffiliatePartnerFee B2B 業務合約現金酬勞（對手方為 Liability.AccountsPayable.*，呼應 SPEC-011 §6.4）
  └─ Expense.LiquiditySubsidy    DMM 流動性補貼

資產 — 銀行（Assets — Bank）
  └─ Bank.OperatingAccount.{currency}         營運帳戶（**禁止**接收玩家儲值；專供 §6.4 業務酬勞 / 平台營運費出帳）

特殊
  └─ Suspense.Reconciliation     對帳暫掛（差異追查中）
```

### 4.2.1 凍結子科目（Player.Frozen.{user_id}）

> 決議 2026-05-05：採 **(b) 帳本層轉子科目** 而非 metadata flag 或純 application freeze。理由：

- **Audit Trail（SPEC-001 §1.1 ③）**：永久封號 / 自我排除為不可逆事件，必須在帳本留下明確 entry，metadata flag 無法滿足「不可竄改事件流」要求。
- **隔離 INV-1 / INV-2 計算**：凍結資產仍是平台對玩家之負債（不能歸 0），但不應出現在「可用餘額」對帳；以子科目分流可避免日常 wallet 查詢誤觸。
- **可逆操作**：若凍結觸發條件解除（例如自我排除期滿、Compliance 釋出），由反向 entry 從 `Player.Frozen.{user_id}` 轉回 `Player.Wallet.{user_id}`，全程留痕。

#### 凍結流程分錄

```text
觸發：玩家 42 自我排除 / Compliance 凍結 / EDD 待審
凍結金額 = Player.Wallet.42 + Player.Locked.42（先撤掉所有掛單，鎖倉退回 wallet 後再轉凍結）

  Entry 1: Player.Wallet.42       +X       (負債從 wallet 減少 X)
  Entry 2: Player.Frozen.42       -X       (負債轉到 frozen 子科目)
  metadata: { reason: 'self_exclusion' | 'compliance_freeze' | 'edd_pending', triggered_by, ref_id }
```

#### 解凍流程

```text
  Entry 1: Player.Frozen.42       +X
  Entry 2: Player.Wallet.42       -X
  metadata: { reason: 'self_exclusion_expired' | 'compliance_cleared', approved_by, ref_id }
```

> Compliance 凍結 / EDD 凍結之解凍須走 SPEC-010 §9 4-eye review；自我排除期滿解凍由 SPEC-006 §5 排程自動觸發但仍寫入 `approved_by = 'system:rg-scheduler'`。

#### INV 影響

- **INV-2**：「平台對玩家負債總額」公式擴展為 `Σ(Player.Wallet.*) + Σ(Player.Locked.*) + Σ(Player.Frozen.*)`，§5 與 §11 對帳腳本須同步更新。
- **INV-4**：`Player.Frozen.X` 同樣強制 ≥ 0。
- **凍結帳戶不可下單**：matching-engine 撤單流程（SPEC-003）須檢查 user 凍結狀態，禁止任何 `Player.Wallet → Player.Locked` 之 entry。

### 4.3 帳本表結構（PostgreSQL）

```sql
-- 帳戶表（每個帳戶一列）
CREATE TABLE ledger_accounts (
    id              BIGSERIAL PRIMARY KEY,
    code            VARCHAR(128) NOT NULL UNIQUE,        -- 例: 'Player.Wallet.42'
    type            VARCHAR(32)  NOT NULL,               -- asset/liability/revenue/expense
    currency        VARCHAR(8)   NOT NULL DEFAULT 'PTS', -- 點數內幣別
    parent_id       BIGINT       REFERENCES ledger_accounts(id),
    metadata        JSONB        NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    closed_at       TIMESTAMPTZ
);
CREATE INDEX idx_accounts_type    ON ledger_accounts(type);
CREATE INDEX idx_accounts_code    ON ledger_accounts(code);

-- 交易表（每個 business event 一列，以 idempotency_key 防重）
CREATE TABLE ledger_transactions (
    id                 BIGSERIAL PRIMARY KEY,
    txn_type           VARCHAR(64) NOT NULL,             -- deposit/order_lock/trade/settle/...
    idempotency_key    VARCHAR(128) NOT NULL UNIQUE,     -- 上游事件的唯一 ID
    correlation_id     VARCHAR(128),                     -- 關聯 trace
    description        TEXT,
    metadata           JSONB NOT NULL DEFAULT '{}',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    posted_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_txn_type ON ledger_transactions(txn_type);
CREATE INDEX idx_txn_corr ON ledger_transactions(correlation_id);

-- 分錄表（每個交易至少兩列：借方 + 貸方）
CREATE TABLE ledger_entries (
    id              BIGSERIAL PRIMARY KEY,
    txn_id          BIGINT NOT NULL REFERENCES ledger_transactions(id),
    account_id      BIGINT NOT NULL REFERENCES ledger_accounts(id),
    direction       SMALLINT NOT NULL CHECK (direction IN (1,-1)),  -- 1=借方/+; -1=貸方/-
    amount          BIGINT  NOT NULL CHECK (amount > 0),             -- 內部單位（百分點）
    currency        VARCHAR(8) NOT NULL,
    balance_after   BIGINT NOT NULL,                                 -- 該帳戶累計餘額
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_entries_txn     ON ledger_entries(txn_id);
CREATE INDEX idx_entries_account ON ledger_entries(account_id, created_at);

-- 不變式：每個 transaction 的所有 entries amount * direction 之和必為 0
-- 由 trigger 強制
CREATE OR REPLACE FUNCTION fn_ledger_balance_check()
RETURNS TRIGGER AS $$
DECLARE total BIGINT;
BEGIN
    SELECT COALESCE(SUM(amount * direction), 0) INTO total
    FROM ledger_entries
    WHERE txn_id = NEW.txn_id;
    IF total <> 0 THEN
        RAISE EXCEPTION 'Ledger imbalance for txn %: %', NEW.txn_id, total;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- DEFERRABLE 在 commit 時檢查，允許 transaction 內逐筆插入
CREATE CONSTRAINT TRIGGER trg_ledger_balance
AFTER INSERT ON ledger_entries
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION fn_ledger_balance_check();
```

### 4.4 典型交易範例（分錄）

#### 4.4.1 玩家儲值 1000 TWD

```text
Txn: deposit, idempotency_key='gateway-tx-abc123'

  Entry 1: Bank.TrustAccount.TWD                +100,000  (1000 TWD * 100)
  Entry 2: Player.Wallet.42                     -100,000

(從會計角度：銀行帳戶資產增加 1000，平台對玩家負債增加 1000)
```

> 註：`Player.Wallet.42` 為負債類，貸方記負（direction = -1）。當玩家擁有點數時，平台**對玩家負債**，玩家提走則平台負債減少。

#### 4.4.2 玩家掛單鎖倉 500 點

```text
Txn: order_lock, idempotency_key='order-789'

  Entry 1: Player.Wallet.42                     +50,000  (鎖倉等同於玩家可用負債轉移)
  Entry 2: Player.Locked.42                     -50,000
```

#### 4.4.3 兩玩家成交（A 買 YES@0.65 × 100，B 買 NO@0.35 × 100，opening trade）

> 本範例採 Kalshi 對稱 escrow 模型，與 §4.4.4 結算範例的**前置狀態 100% 對齊**。
> A 與 B 各自從掛單階段（§4.4.2 等同邏輯）已鎖入各自的 max loss，撮合成交時**僅扣手續費**，鎖倉不釋出 — 因為 escrow 必須留至結算才釋出（呼應 §4.4.4）。

語意：

- A 在掛單階段 `Player.Locked.A` 鎖入 `0.65 × 100 × 100 = 6,500`（A 的 max loss = 0.65 × 100 股）
- B 在掛單階段 `Player.Locked.B` 鎖入 `0.35 × 100 × 100 = 3,500`（B 的 max loss = 0.35 × 100 股）
- 撮合成交時 trade_value = `price × qty = 65 × 100 = 6,500`（內部單位）
- 平台手續費 2%，雙邊各 `6,500 × 200 / 10000 = 130`（= 1.3 點）
- 手續費**從各自 wallet 扣**（不動 escrow）

```text
Txn: trade, idempotency_key='trade-001', metadata: {trade_value: 6500, fee_bps: 200}

  Entry 1: Player.Wallet.A    +130     (從 A 可用餘額扣 1.3 點手續費 → 負債減少)
  Entry 2: Revenue.TradingFee -130     (平台 fee 收入增加)

  Entry 3: Player.Wallet.B    +130
  Entry 4: Revenue.TradingFee -130

  借貸代數和：130 + (-130) + 130 + (-130) = 0 ✓
```

> 同時由 application 層產生 Position 紀錄（呼應 SPEC-015 outcomes / position 表）：
>
> - `Position{user: A, market: X, outcome: YES, qty: 100, locked_amount: 6500}`
> - `Position{user: B, market: X, outcome: NO,  qty: 100, locked_amount: 3500}`
>
> 這些 position 之 `locked_amount` 對應的 `Player.Locked.{A,B}` 餘額**不在本 trade transaction 變動**；其釋放發生於 §4.4.4 settle。

#### 4.4.3.1 closing trade（玩家賣出已持倉）

> 上述 §4.4.3 為**兩位玩家同時 open 反向倉位**之新成交。若其中一方為**已持倉者賣出 close 倉位**（如 A 已持 100 YES，現以 0.70 賣給新買家 C），則 A 的 escrow 在賣出成交時釋放回 wallet，C 鎖入新 escrow。完整範例見 SPEC-015 §10 銜接點與 SPEC-003 §14.2 撮合公式。

#### 4.4.4 結算（YES 中獎，A 持 100 股 YES、B 持 100 股 NO）

本平台採 Kalshi / 標準預測市場的對稱 escrow 設計。當 A 以 0.65 買 YES 100 股、B 以 0.35 買 NO 100 股（亦即 B 以 0.65 賣 YES 100 股）對撮成交時：

- A 在 trade 階段鎖入 `0.65 × 100 = 6,500`（A 的 max loss）。
- B 在 trade 階段鎖入 `0.35 × 100 = 3,500`（B 的 max loss）。
- 該配對 escrow 總額為 `10,000`（= 1.00 × 100），必有一方獨拿全額。

結算時，escrow 全額流向勝方，敗方鎖倉**就地清零**（鎖倉直接結束、不退還）。如此**無零金額 entry**，符合 schema `CHECK (amount > 0)`。

YES 中獎時：

```text
Txn: settle, idempotency_key='market-X-settle:position-A-001'

  Entry 1: Player.Locked.A    +6,500    (A 鎖倉釋出 — 勝方應退還的 escrow)
  Entry 2: Player.Locked.B    +3,500    (B 鎖倉釋出 — 敗方 escrow 流向勝方)
  Entry 3: Player.Wallet.A    -10,000   (勝方收下 escrow 總額 = 6,500 + 3,500)

  借貸代數和：6,500 + 3,500 + (-10,000) = 0 ✓
```

NO 中獎時：

```text
Txn: settle, idempotency_key='market-X-settle:position-B-001:NO'

  Entry 1: Player.Locked.A    +6,500
  Entry 2: Player.Locked.B    +3,500
  Entry 3: Player.Wallet.B    -10,000   (NO 勝出，B 收下 escrow 總額)
```

**實作注意：**

1. 本範例為**單一配對成交（trade pair）**之結算。實務上同一玩家可能持有多筆來自不同對手方的 trade pair；結算批次以 `position_id` 為單位逐筆處理，每筆三條 entry：`Locked.winner` / `Locked.loser` / `Wallet.winner`。`amount > 0` constraint 永遠成立。
2. 若市場進入 `CANCELLED`（VOID），則退回為「各方退回原 escrow」：每個 position 寫入兩條 entry（`Locked.X +x`、`Wallet.X -x`）。
3. 對於**同一玩家同時持 YES 與 NO**（hedge）情境，結算批次內仍以 `position_id` 為單位逐筆處理，避免 cross-position 抵銷導致 entry 數量驟減而難以稽核。**Hedge 政策已於 [SPEC-000 §5 Q-7](./SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker) Resolved 為「允許」**（撤除 SPEC-015 §8.1 原禁令，SPEC-003 §12 風控前置層不加 `HEDGE_NOT_ALLOWED` 檢查）；潛在自我對沖刷量行為由 SPEC-010 §4.2.2 post-trade 偵測攔截。
4. `settlement_logs` 額外記錄每筆 settle 的對應 `trade_id` / `position_id` / `oracle_submission_id` 來源，供爭議裁決回溯（呼應 SPEC-004 §6.3）。
5. §4.4.3 已對齊本節對稱 escrow 模型（A 鎖 6,500 / B 鎖 3,500），不再為簡化版本。完整 trade 撮合與鎖倉公式詳見 SPEC-003 §14.2 與 SPEC-015 §10 之銜接點。

## 5. 不變式（Invariants）

> **單一真相來源**：以下不變式**編號與內容**對齊 [ADR-0003 §5](../docs/architecture/decisions/ADR-0003-double-entry-ledger.md)。任何修改 INV 編號或語意必須先寫一份新 ADR，並標記 ADR-0003 為 Superseded。

系統任何時刻必須維持以下等式：

| ID | 等式 | 檢查頻率 | 違反處理 |
|----|------|---------|---------|
| INV-1 | `每個 ledger_transaction 的 entries 借貸代數和為 0` | 每筆 commit | DB CONSTRAINT TRIGGER 強制（見 §4.3 `fn_ledger_balance_check`）|
| INV-2 | `Σ(Player.Wallet.*) + Σ(Player.Locked.*) + Σ(Player.Frozen.*) = 平台對玩家負債總額` | 每 5 分鐘對帳腳本 | 拋例外、寫入 Suspense.Reconciliation、P0 告警 |
| INV-3 | `Bank.TrustAccount + Σ(Revenue.*) − Σ(Expense.*) = 平台對玩家負債總額` | 每日 T+1 對帳 | 凍結出帳、啟動人工稽核 |
| INV-4 | `每個 Player.Wallet.X 任意時刻 ≥ 0`（chargeback 反沖外） | 每筆變動 | 拋 INSUFFICIENT_FUNDS；chargeback 例外見 §8.2 |
| INV-5 | `每個 ledger_entry 一旦寫入不可 UPDATE / DELETE` | 持續 | DB role 權限分離（`ledger_app` 僅 INSERT）+ audit |
| INV-6 | `Player.Wallet.{user_id}` 之 outflow 目的地白名單：僅限 `Player.Locked.{user_id}` / `Player.Frozen.{user_id}` / `Revenue.TradingFee` / `Revenue.SettlementFee` / `Player.Wallet.{other_user_id}`（限 trade settlement，需 `settle_txn_id` 不空）。**禁止**流向 `Liability.AccountsPayable.*` / `Bank.OperatingAccount.*` / `Expense.AffiliatePartnerFee` / 任何銀行 / 支付通路 / 外部錢包科目 | 每筆 commit | DB CONSTRAINT TRIGGER 強制（拒絕 commit，SQLSTATE `40004`）+ CI sentinel-scan grep gate；呼應 SPEC-001 §1.1 ① 「點數不可回兌」與 SPEC-011 §6.3 R-1 紅線；落地 [ADR-0006 §4.2](../docs/architecture/decisions/ADR-0006-ledger-invariant-set-extension-inv6.md) |

> **Player.Locked.X ≥ 0**：邏輯上隱含於 INV-1（鎖倉釋出與鎖入必為對沖），由 application 層在下單 / 成交路徑強制檢查。不再單獨列為頂層 INV，避免與 ADR-0003 編號衝突。
>
> **INV 集合 ADR governance**：INV-1 ~ INV-5 由 [ADR-0003 §5](../docs/architecture/decisions/ADR-0003-double-entry-ledger.md) v1.0 拍板；INV-2 / INV-4 公式擴展含 `Player.Frozen.*`（v0.1.1）+ INV-6 outflow 白名單（v0.1.5）由 [ADR-0006 §4.1](../docs/architecture/decisions/ADR-0006-ledger-invariant-set-extension-inv6.md) 正式 governance 追認與新增。INV-6 trigger 之 plpgsql function 範本見 ADR-0006 §4.2.1，wallet-ledger-service migration 可依此實作。任何進一步擴展（例如未來新增「合作戒癮機構捐贈」白名單目的地）須先寫新 ADR 標記 ADR-0006 §4.1 之 INV-6 為 partial superseded，附業主 + Compliance + Legal 三方簽核，**不可**直接改 trigger function。

## 6. Idempotency 與重試

### 6.1 強制要求

所有寫入帳本的 API **必須**接受 `Idempotency-Key` header，後端：

1. 收到請求 → 計算 key
2. 查 `idempotency_records` 表
3. 若存在：返回先前的結果（即使是錯誤）
4. 若不存在：執行交易、寫入結果、返回

```sql
CREATE TABLE idempotency_records (
    key         VARCHAR(128) PRIMARY KEY,
    txn_id      BIGINT REFERENCES ledger_transactions(id),
    response    JSONB NOT NULL,
    status_code INT   NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at  TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_idempo_expires ON idempotency_records(expires_at);
```

保存期：**7 天**（避免重複請求穿越時間）。每日 cron 清理過期 key。

### 6.2 Webhook 處理

支付 Gateway webhook 必須：

1. 驗證 HMAC 簽章
2. 以 gateway 的 `transaction_id` 為 idempotency_key
3. 即使 webhook 觸發 5 次，玩家錢包也只增加一次

## 7. 對帳機制

### 7.1 多層對帳

```text
Layer 1 (即時)：每筆 ledger_transaction 借貸和為 0  ← DB trigger
Layer 2 (5min)：Σ(Wallet) + Σ(Locked) ≈ 對玩家負債帳戶總和
Layer 3 (1hr) ：Bank.TrustAccount 銀行 API 餘額 vs ledger 預期銀行餘額
Layer 4 (T+1) ：玩家所有金流事件 vs 支付 Gateway 對帳檔
Layer 5 (月度)：第三方審計事務所抽樣對帳
```

### 7.2 差異處理

當 Layer 2-4 出現差異時：

1. **不**自動修正帳本（避免錯誤被掩蓋）
2. 寫入 `Suspense.Reconciliation` 暫掛帳戶
3. 觸發 P0 告警，財務 + 工程 oncall 介入
4. 人工釐清後，**以新的調整 transaction** 修正（保留原始紀錄）

## 8. 退款與爭議款處理

### 8.1 退款場景

| 場景 | 退款路徑 |
|------|---------|
| 玩家刷卡爭議（Chargeback）| 點數 reverse + 錢包負餘額允許至銀行確認後回正；**累計 chargeback ≥ 1 立即永久封號 + 黑名單**（呼應 SPEC-016 §6.1 / SPEC-010 §6.2，H-5 跨檔統一門檻） |
| 市場取消 | 所有未結算持倉 reverse，回到 Wallet |
| 系統故障結算錯誤 | 由 admin 提 reversal txn，需雙人複核（4-eye，呼應 SPEC-010 §9.3 MS-6）|

### 8.2 玩家錢包負餘額處理

- 一般情境**禁止**負餘額（INV-4）
- 例外：Chargeback 反沖時短暫允許，但帳戶**立即凍結**至釐清

```sql
-- 一般 wallet 餘額 check
ALTER TABLE ledger_entries
  ADD CONSTRAINT chk_wallet_nonneg
  CHECK (
    NOT (account_id IN (SELECT id FROM ledger_accounts WHERE type='liability' AND code LIKE 'Player.Wallet.%')
         AND balance_after < 0)
  ) NOT VALID;  -- 例外情境由 application 層特例放行
```

## 9. 性能考量

### 9.1 寫入瓶頸

每筆撮合會產生 6 條 entry，TPS 50,000 撮合 = 300,000 entry inserts/sec。

**單一 PostgreSQL 寫不下**。解決方案：

1. **撮合事件先寫 Kafka / NATS**（持久化 + 順序保證）
2. **Ledger Worker 批次消費**（100ms 一批，每批 ~1000 events）
3. **PostgreSQL 用 partition by time**（每月一個 partition）
4. **熱資料用 logical replication 同步到 read-replica** 供查詢

### 9.2 讀取瓶頸

玩家錢包餘額讀取頻繁。

1. **Redis 快取最新餘額**（key: `wallet:{user_id}`）
2. 寫入時 invalidate
3. cache miss 從 ledger 計算最新 balance（取最後一筆 entry 的 balance_after）

### 9.3 撮合 → ledger 最終一致性窗口（T-5）

> 撮合事件先寫 Kafka 後由 ledger worker 批次消費（§9.1 第 1-2 點），意味著「成交發生」與「玩家錢包餘額更新」之間存在 ~100ms 之最終一致性窗口。前後端必須對此明示處理，否則玩家會在「下單成功」後立即查餘額看不到變化，誤以為系統故障。

#### 9.3.1 一致性語意

| 路徑 | 一致性 | 玩家可觀察延遲 P99 |
|------|--------|------------------|
| 撮合 ingressCh → 撮合事件 publish 至 Kafka | strong（in-process）| < 1ms |
| Kafka publish → wallet-ledger-service consume | strong（Kafka acks=all）| < 5ms |
| ledger worker batch（100ms / 1000 events） → DB commit | **eventual**（受 batch window 限制）| **≤ 100ms** |
| ledger DB commit → Redis cache invalidate | strong（同步 invalidate）| < 5ms |
| Redis cache invalidate → 玩家 GET /wallet 看到新餘額 | strong（cache miss fallback to DB）| < 10ms |
| **總端到端**（撮合成交 → 玩家查餘額）| **eventual**（≤ 100ms）| **≤ 130ms P99** |

#### 9.3.2 前端 UX 處理（呼應 SPEC-014 §3.2）

- **不依賴**「下單成功 → 立即 GET /wallet」之強一致路徑（會看到 stale balance）
- **採 optimistic UI**：
  1. 玩家下單 → API 回 `OrderAccepted` event（含本次預期之 balance delta）
  2. 前端 local state 立即更新（樂觀 UI）
  3. WS 訂閱 `wallet.events` topic（呼應 SPEC-003 §10.4 / SPEC-014 §6.2）：當 ledger commit 完成後 server push 真實 balance
  4. 若 WS push 與 local state 不一致 → 以 server 為準 + UI flash 動畫提示
- **拒絕模式**：若 risk pre-check 已 reject，立即 API 回 `OrderRejected`，前端不做 optimistic update

#### 9.3.3 後端 API 處理

- `POST /api/v1/orders` 之 response body **不**包含「最終 ledger balance」欄位（避免假裝強一致）
- 提供 `GET /api/v1/me/wallet?at_seq=N`（撮合 seq）讓客戶端可選擇等到某 seq 之後再取（適用於 reconciliation）
- 對帳 / 客服場景強制使用 `GET /wallet?consistent=true` flag，後端會等到該玩家之最近未消費 ledger event 全部 commit 才回（trade off latency 換 strong consistency）

#### 9.3.4 不一致窗口監控

呼應 SPEC-007 §6.4，OTel span 量測：

```
proph_ledger_event_lag_seconds  histogram, labels: stage
  - stage="kafka_publish"        撮合 ingress → Kafka publish 完成
  - stage="kafka_consume"        Kafka publish → ledger worker consume 開始
  - stage="batch_wait"           consume 開始 → batch 觸發
  - stage="db_commit"            batch 觸發 → DB transaction commit 完成
  - stage="cache_invalidate"     DB commit → Redis cache invalidate 完成
  - stage="end_to_end"           撮合 ingress → cache invalidate 完成
```

P99 端到端 lag > 500ms 持續 5 分鐘 → P1 告警；> 1s → P0。

## 10. 第三方審計準備

### 10.1 月度審計工作流

1. T+5 提供前月完整 ledger dump（PostgreSQL pg_dump）
2. 提供託管銀行對帳單
3. 提供支付 Gateway 對帳檔
4. 審計事務所抽樣比對：
   - 隨機抽 100 筆 deposit 是否帳戶到帳
   - 隨機抽 100 筆 withdrawal（內部劃轉）是否合理
   - 月初 / 月末平台負債餘額 vs 銀行託管餘額

### 10.2 不可竄改性證明

- 每日將該日 ledger_entries 之 hash 鏈寫入 R2 Object Lock 桶
- hash chain：`H(day_N) = SHA256(H(day_N-1) || sorted_entries_of_day_N)`
- 審計時驗證 hash chain 連續性

## 11. 驗收條件

- [ ] PostgreSQL schema 全部建立並通過 migration 測試
- [ ] DB trigger 強制 INV-1 通過驗證測試（嘗試插入不平衡 transaction 必須 fail）
- [ ] 模擬 1000 並發儲值，最終 wallet 餘額與 transaction count 完全相符
- [ ] Idempotency 測試：相同 key 提交 100 次，僅一筆 transaction
- [ ] 對帳腳本可在 5 分鐘內處理 1M 筆 entries
- [ ] hash chain 驗證腳本可在 30 分鐘內驗證 30 天歷史

## 12. Open Questions

> 跨檔決策已收斂至 [SPEC-000 §5](./SPEC-000-index.md) 中央追蹤表（含 Q-3 點數計價單位、Q-6 數據主權）。本節僅保留 SPEC-002 自身範圍內議題。

1. ~~**ledger 服務拓撲**：拆獨立 microservice（wallet-ledger-service）vs 與 wallet 同 service 不同 module？~~ ✅ **Resolved 2026-05-05**：**拆為獨立 microservice `wallet-ledger-service`**（呼應 SPEC-014 §4.1 之「3+ replicas + leader」）。理由：
   - **Single-writer leader 模式**：雙簿記 INV-1 / INV-2 / hash chain 需要單寫者保證順序，獨立 service 可清楚劃出 leader 邊界（透過 Postgres advisory lock 或 etcd lease），與 user-service 的多寫者模式不同。
   - **Idempotency 中央化**：所有需要寫帳本的操作（user-service 儲值、matching-engine 鎖倉/成交、settlement-worker 結算、payment-service webhook）統一走 `wallet-ledger-service` 之 idempotency key，符合 SPEC-002 §6。
   - **攻擊面隔離**：帳本是平台金流核心，與認證 / KYC / 通知等 lower-trust 邏輯共處同一 binary 會放大攻擊面（SPEC-010 §9 4-eye review 對 ledger 變動要求高於 user CRUD）。
   - **獨立擴容**：對帳腳本（§11）與 hash chain 驗證對 IO 與 CPU 需求顯著高於 user-service。
   - 落地：服務名 `wallet-ledger-service`，binary 位於 `services/wallet-ledger-service/`（已在 SPEC-014 §4.2 monorepo 結構落地）；對外僅暴露 gRPC（PostTransaction / GetBalance / Reconcile / GetEntries），不開 public REST。
   - 同步更新 [ADR-0003 §11 V-1](../docs/architecture/decisions/ADR-0003-double-entry-ledger.md) 為 Resolved。
2. ~~**`ledger_entries` partition 策略**：每月 / 每週 / 每年？~~ ✅ **Resolved 2026-05-07（v0.1.7 修正 T-3）**：**Phase 1 直接採 monthly range partition by `created_at`**（透過 `pg_partman`），**不留**「先不分區後 migrate」的技術債。具體規則：
   - **Phase 1 從 day 1 啟用 monthly partition**：呼應 §9.1 「TPS 50,000 = 300,000 entry inserts/sec — 單一 PostgreSQL 寫不下」之自我估算 — 即便 1k orders/sec 之 baseline 流量，每日 ~5.2 億 entries × 6 entries/order，~3 天即達 100M 門檻；事後做 online partition migration 是高風險操作（鎖表、主從切換、index rebuild），動工前直接 partition 成本與不 partition 相當。
   - **Partition 鍵與粒度**：`PARTITION BY RANGE (created_at)`，每月一個 partition；採 `pg_partman` 自動建立未來 3 個月之 partition + 自動 detach 12 個月以前之 cold partition。
   - **冷分區（> 12 個月）**：detach 後 dump 至 R2 Object Lock（WORM）+ Postgres foreign table（postgres_fdw）保留可查詢性，呼應 SPEC-008 §6.1 雙寫策略。
   - **`ledger_entries.created_at` index**：每個 partition 內走 BRIN（block range index）— 撮合事件之 created_at monotonic 增加，BRIN 對 range query 高效；不採 B-tree（避免每 partition × N 次寫入之 index overhead）。
   - **Migration governance**：Phase 1 schema migration 由 wallet-ledger-service 之 sqlx migration 直接建立 partitioned table（非「先建普通表後改 partitioned」）；ADR-0003 §11 V-2 同步更新為 Resolved。
   - **原 v0.1.2 / v0.1.3 之「Phase 1 不分區 + 觸發門檻」設計已撤回**（T-3）：經算量盤點該設計在熱市場下撐不到一週，事後 migration 風險高於前期 partition 設計成本。
3. **WORM 儲存**：R2 Object Lock vs AWS S3 Glacier（呼應 SPEC-008 §6.1 已部分定案 R2 + S3 雙寫；本 SPEC §10.2 hash chain 寫入 R2 Object Lock 桶為定案，僅 cold archive 介質保留討論）。
4. ~~**Player Wallet 帳戶在玩家自我排除（永久封號）後如何處理**~~ ✅ **Resolved 2026-05-05**：採 **(b) 轉 sub-account**，新增 `Player.Frozen.{user_id}` 子科目，詳見 §4.2.1；INV-2 / INV-4 同步擴展。
5. **客服 compensation transaction 介面**：手動表單 vs Workflow 工具（Temporal / Airflow）？追蹤於 ADR-0003 §11 V-3。
