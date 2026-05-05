# SPEC-002 — 資金安全與虛擬點數帳本

> 優先級：**P0**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-001
> 被依賴：SPEC-003, SPEC-004, SPEC-005, SPEC-011

---

## 1. Objective

確保玩家資金（法幣儲值）與站內虛擬點數的金流，達到金融級的精確、可稽核、雙簿記、零誤差。任何時刻系統都能回答：「現在玩家總計擁有多少點數？平台託管帳戶有多少資金？兩者是否相符？」

## 2. 資金分層架構

```
┌────────────────────────────────────────────────────────────────────┐
│ Player                                                              │
│   ↓ 法幣（TWD / USD / JPY / VND）                                    │
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

### 2.1 託管銀行帳戶（Segregated Trust Account）強制要求

| 要求 | 說明 |
|------|------|
| 帳戶名義 | `Operating Co. - Player Funds Trust Account`，與營運帳戶分離 |
| 銀行類別 | Tier-1 國際銀行（DBS / HSBC / Standard Chartered 等執照地分行）|
| 動用權限 | 只能用於玩家儲值收款、手續費結算劃轉、退費 |
| 不得動用於 | 行銷、員工薪資、稅費、董監酬金 |
| 對帳頻率 | 每日 T+1 自動對帳，月度第三方審計 |
| 餘額警示 | 銀行餘額 < 預期玩家點數負債 95% 即觸發 P0 告警 |

## 3. 點數計價設計

### 3.1 計價單位選定

> 決策：**1 點 = TWD 1 等值（依儲值當下匯率鎖定）**

雖然 1:1 容易與現金混淆，但抽象比例（如 100 點 = TWD 1）會增加用戶心智負擔，且國際市場匯率變動會導致「持有的點數變多變少」的詭異現象。**統一以 TWD 為計算基準，UI 顯示「點」即可，避免 $ 符號**。

替代方案（保留供討論）：

- 方案 A（推薦）：`1 點 = TWD 1`，國際玩家儲值時依當下牌告匯率換算
- 方案 B：`1 點 = USD 0.01`，與 Kalshi 接軌，缺點是台灣玩家心智換算
- 方案 C：完全抽象單位「Prophet Coin」，每月由平台公告匯率，缺點是匯率風險集中於平台

### 3.2 精度

- 點數最小單位：`0.01 點`（兩位小數）
- 系統內部儲存：`BIGINT`，以 `1 點 = 100` 為內部單位（避免浮點誤差）
- 顯示給用戶：`fmt.Sprintf("%.2f 點", value/100.0)`

## 4. 雙簿記帳本（Double-Entry Ledger）

### 4.1 為何必須雙簿記

單簿記（單純加減 wallet 餘額）的致命問題：
- 任何 bug 導致的「憑空產生點數」無法事後察覺
- 無法回答「這筆點數從哪裡來，要往哪裡去」
- 對帳時無法定位差異點

雙簿記原則：**每一筆點數變動，必有等額相反方向的另一筆紀錄**。

### 4.2 帳戶體系（Chart of Accounts）

```
資產（Assets）
  └─ Bank.TrustAccount.TWD       託管銀行 TWD 餘額
  └─ Bank.TrustAccount.USD       託管銀行 USD 餘額
  └─ Bank.OperatingAccount       營運帳戶（手續費收入轉入）

負債（Liabilities）
  └─ Player.Wallet.{user_id}     對玩家的點數負債（每個玩家一個帳戶）
  └─ Player.Locked.{user_id}     鎖倉中（掛單未成交、結算中）

收入（Revenue）
  └─ Revenue.TradingFee          交易手續費收入
  └─ Revenue.SettlementFee       結算手續費收入

費用（Expenses）
  └─ Expense.PaymentGatewayFee   支付通路手續費
  └─ Expense.AffiliateCommission 代理分潤
  └─ Expense.LiquiditySubsidy    DMM 流動性補貼

特殊
  └─ Suspense.Reconciliation     對帳暫掛（差異追查中）
```

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

```
Txn: deposit, idempotency_key='gateway-tx-abc123'

  Entry 1: Bank.TrustAccount.TWD                +100,000  (1000 TWD * 100)
  Entry 2: Player.Wallet.42                     -100,000

(從會計角度：銀行帳戶資產增加 1000，平台對玩家負債增加 1000)
```

> 註：`Player.Wallet.42` 為負債類，貸方記負（direction = -1）。當玩家擁有點數時，平台**對玩家負債**，玩家提走則平台負債減少。

#### 4.4.2 玩家掛單鎖倉 500 點

```
Txn: order_lock, idempotency_key='order-789'

  Entry 1: Player.Wallet.42                     +50,000  (鎖倉等同於玩家可用負債轉移)
  Entry 2: Player.Locked.42                     -50,000
```

#### 4.4.3 兩玩家成交（A 買 YES@0.65 × 100，B 賣 YES@0.65 × 100）

```
A 買 YES @ 0.65 → A 付 65 點，獲得 100 股 YES
B 賣 YES @ 0.65 → B 收 65 點，賣出 100 股 YES
平台手續費：2% of trade value = 1.3 點 × 2 邊 = 2.6 點

Txn: trade, idempotency_key='trade-001'

  Entry 1: Player.Locked.A    +6,500   (A 鎖倉釋出 65 點)
  Entry 2: Player.Wallet.A    -6,370   (A 實付 65 - 1.3 手續費後 = 63.7)
  Entry 3: Revenue.TradingFee -130     (平台收 A 手續費)

  Entry 4: Player.Locked.B    +6,500
  Entry 5: Player.Wallet.B    -6,370
  Entry 6: Revenue.TradingFee -130

(每筆 entry 借貸總和為 0)
```

#### 4.4.4 結算（YES 中獎，A 持 100 股）

```
Txn: settle, idempotency_key='market-X-settle'

  A 100 股 YES 結算為 100 點:
  Entry 1: Player.Locked.A    +10,000   (持倉鎖倉釋出)
  Entry 2: Player.Wallet.A    -10,000   (進入 A 可用餘額)

  B 100 股 NO 結算為 0:
  Entry 3: Player.Locked.B    +0        (持倉鎖倉釋出)
  Entry 4: Player.Wallet.B    -0
  → B 對應持倉直接歸零，無實質帳本變動，但要記錄 0-amount entry 以供稽核
```

> 實作注意：上述「Player.Locked.B 釋出 0」的邏輯中，amount > 0 的約束需處理。建議改為：結算時直接從持倉表扣減，零價值結算不產生 ledger entry，但於 settlement_logs 記錄。

## 5. 不變式（Invariants）

系統任何時刻必須維持以下等式：

| ID | 等式 | 檢查頻率 | 違反處理 |
|----|------|---------|---------|
| INV-1 | `Σ(Player.Wallet.*) + Σ(Player.Locked.*) = 對玩家負債總額` | 每筆交易後 | 拋例外，rollback |
| INV-2 | `Bank.TrustAccount + Revenue.PendingPayout = 對玩家負債總額 + 手續費收入` | 每日 T+1 | 凍結出帳，啟動稽核 |
| INV-3 | `每個 ledger_transaction 的 entries 借貸和為 0` | 每筆 commit | DB trigger 強制 |
| INV-4 | `每個 Player.Wallet.X 任意時刻 ≥ 0` | 每筆變動 | 拋 INSUFFICIENT_FUNDS |
| INV-5 | `每個 Player.Locked.X 任意時刻 ≥ 0` | 每筆變動 | 同上 |

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

```
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
| 玩家刷卡爭議（Chargeback）| 點數 reverse + 錢包負餘額允許至銀行確認後回正 |
| 市場取消 | 所有未結算持倉 reverse，回到 Wallet |
| 系統故障結算錯誤 | 由 admin 提 reversal txn，需雙人複核 |

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
- [ ] DB trigger 強制 INV-3 通過驗證測試（嘗試插入不平衡 transaction 必須 fail）
- [ ] 模擬 1000 並發儲值，最終 wallet 餘額與 transaction count 完全相符
- [ ] Idempotency 測試：相同 key 提交 100 次，僅一筆 transaction
- [ ] 對帳腳本可在 5 分鐘內處理 1M 筆 entries
- [ ] hash chain 驗證腳本可在 30 分鐘內驗證 30 天歷史
- [ ] 第三方安全審計通過（Trail of Bits 或同等級）

## 12. Open Questions

1. 跨幣別處理：玩家以 USD 儲值要不要在 ledger 內以 USD 帳戶記錄，還是入時即轉 PTS？
2. 託管銀行帳戶可否取得 API？綠界 / 藍新是否提供？
3. WORM 儲存使用 R2 Object Lock 還是 AWS S3 Glacier？
4. ledger 是否拆獨立 microservice？還是與 wallet service 同一個？
5. Player Wallet 帳戶在玩家自我排除（永久封號）後如何處理？
