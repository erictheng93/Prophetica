# ADR-0003 — 採用雙簿記帳本（Double-Entry Bookkeeping），捨棄單簿記與「balance += amount」捷徑

| 欄位 | 內容 |
|------|------|
| **Status** | Accepted |
| **Date** | 2026-05-05 |
| **Deciders** | Tech Lead, Founder |
| **Consulted** | 預定：執照地會計師、Compliance Officer、外部審計事務所 |
| **Informed** | 工程團隊（特別是 wallet-ledger-service 與 settlement-worker 開發者）、Codex 開發代理、財務主管 |
| **Tags** | ledger · finance · audit · core-architecture · invariant |
| **依賴** | SPEC-002, ADR-0002 |

---

## 1. Context and Problem Statement

prophetica 的核心是一個金融級帳本系統。每一筆玩家行為（儲值、掛單鎖倉、成交、結算、退款、推薦獎勵發放、客服補償）都會引起站內點數的位移，而這些位移必須能被：

1. **隨時稽核**：執照地監管要求「請告訴我此刻平台所有玩家點數總和、平台手續費收入總和，並證明數字一致」
2. **完整追溯**：任一筆點數異動都能回答「從哪來、往哪去、為什麼、誰授權」
3. **不可竄改**：任何人（含 DBA）不能秘密調整餘額而不留下痕跡
4. **可被反沖**：Chargeback、結算錯誤、客服補償等場景需要「以新交易反沖舊交易」而非「修改舊交易」
5. **能跨帳戶對等**：玩家錢包總和必須等於平台對玩家的負債總額，必須等於託管銀行帳戶實有金額（扣除手續費收入）

業界對於這種帳本管理有兩派做法：

- **單簿記（Single-Entry）**：直接 `UPDATE wallets SET balance = balance + ?`，配合 audit_log 記錄變動明細。簡單、易實作。
- **雙簿記（Double-Entry）**：每筆 business event 產生一個 `transaction`，含至少兩條 `entry`，分別對借方（debit）與貸方（credit）帳戶記帳，所有 entries 的金額變動代數和為零。源自 1494 年 Luca Pacioli 之會計學。

> 必要說明：SPEC-002 已實質採用雙簿記設計（§4 表結構、INV-1「entries 借貸代數和為 0」不變式、§4.4 範例），本 ADR 的角色是把這個技術決策**升格為架構原則**，並把單簿記的捷徑明確列為禁區，避免未來在性能壓力下被工程師「優化掉」。

## 2. Decision Drivers

依重要性遞減：

1. **不變式可被機械化驗證**：bug 必須自我暴露，不能仰賴人工 review 才發現
2. **監管稽核硬需求**：執照地、外部審計事務所、玩家集體訴訟都會問「所有點數的去向」
3. **歷史不可竄改性**：任何修正必須以新事件表達，不能改寫舊事件
4. **跨帳戶對帳能力**：玩家錢包 / 平台負債 / 銀行託管 / 手續費收入之間的等式必須持續成立
5. **多幣別 / 多 entity 擴展**：未來支援 USD 點數、Op Co. 切多家時帳本不需重寫
6. **支援 reversal 場景**：Chargeback、結算錯誤、客服補償、市場取消都需要反沖
7. **engineering velocity**：MVP 必須快速上線，不能因架構複雜而拖慢開發
8. **性能**：撮合產生的 entry 寫入吞吐必須跟得上撮合本身

## 3. Considered Options

### Option A — 純單簿記（Naive Single-Entry）

```sql
CREATE TABLE wallets (
    user_id BIGINT PRIMARY KEY,
    balance BIGINT NOT NULL DEFAULT 0
);

-- 儲值：
UPDATE wallets SET balance = balance + 1000 WHERE user_id = 42;
-- 鎖倉：
UPDATE wallets SET balance = balance - 500 WHERE user_id = 42;
```

無交易紀錄，只有當下狀態。

### Option B — 單簿記 + 操作日誌

```sql
CREATE TABLE wallets (
    user_id BIGINT PRIMARY KEY,
    balance BIGINT NOT NULL DEFAULT 0
);
CREATE TABLE wallet_logs (
    id BIGSERIAL,
    user_id BIGINT,
    delta BIGINT,
    reason TEXT,
    created_at TIMESTAMPTZ
);

-- 應用層自行確保 wallets 與 wallet_logs 同步寫入（transaction）
```

有歷史紀錄，但 wallet 餘額是**真相來源**，logs 只是「事後拼湊」。

### Option C — 雙簿記（Double-Entry）⭐

```
ledger_accounts  (每個帳戶一列：玩家錢包、玩家鎖倉、平台手續費、託管銀行、付通路費...)
ledger_transactions  (每個 business event 一列)
ledger_entries  (每筆 transaction 至少兩列，借貸金額代數和為零)
```

任何時刻 `account.balance = SUM(entries.amount * entries.direction WHERE account_id = X)`。
DB trigger 強制 `SUM(amount * direction) = 0` per transaction。

詳細實作見 SPEC-002 §4。

### Option D — 三簿記 / 區塊鏈錨定（Triple-Entry）

每筆 transaction 不僅內部雙簿記，還用密碼學簽章寫入第三方公開鏈或 timestamp service（如 OpenTimestamps）。Halpin 與 Pacioli 系列研究、加密貨幣交易所（Kraken Proof-of-Reserves）採用。

### Option E — 第三方 Ledger SaaS

採用 Modern Treasury、Increase、Fragment.io 等 ledger-as-a-service 廠商。

## 4. Decision Outcome

**選擇 Option C：雙簿記。**

> ⚠️ **同時明確記錄**：Option A、B 在本專案壽命週期內**永久禁止採用**作為錢包真相來源。任何「我們先用簡單版本上線、之後再升級」的提案，本 ADR 即為拒絕依據。理由見 §4.2。

### 4.1 為何選 C 而非 A（純單簿記）

A 的核心問題是**沒有歷史**：

- 第一次帳對不上時，無法回溯成因
- 玩家投訴「我儲值了 1000 但餘額沒增加」→ 無法證明對錯
- 監管問「過去 30 天，user_id=42 的所有金流」→ 無法回答
- 任何 bug 導致的金額誤差**永久消失於歷史**

A 不是「簡單方案」，是「不能用方案」。直接否決。

### 4.2 為何選 C 而非 B（單簿記 + 操作日誌）

B 看似有歷史紀錄，常被作為「比 A 進步、比 C 簡單」的折衷。但它有四個致命問題：

#### 4.2.1 真相來源不一致

`wallets.balance` 與 `wallet_logs` 是兩個獨立資料源，可能不同步：

- 應用層在 transaction 內同步寫入 → 看似 OK
- 但 DBA 用 admin 工具直接 UPDATE wallets → log 沒留
- 但某次 race condition 導致 wallet 寫成功 log 寫失敗 → 永遠對不上
- 但某次 deploy 中途切換版本 → 新版預期 log schema、舊版仍在寫入 → 不一致

C 中沒有「真相來源」可言：餘額**就是** entries 的累加。任何「直接修改餘額」的操作不存在 API 表面，只能透過 insert 新 entries。

#### 4.2.2 沒有跨帳戶對等性

B 模型中，`wallets` 表只記**玩家欠平台多少**（或反之）。但平台同時擁有：
- 託管銀行帳戶（asset）
- 對玩家負債（liability，即玩家錢包總和）
- 鎖倉中（liability，含撮合中、結算中）
- 手續費收入（revenue）
- 支付通路費（expense）
- 推薦獎勵費用（expense）

要回答「平台所有資產 = 所有負債 + 所有權益」，B 模型需要為**每一個帳戶類別**都建一張表，並由應用層自行維護等式。等於把 1494 年發明的會計輪子重新造一遍——而且造錯。

C 模型中，所有帳戶共用同一張 `ledger_accounts` 表，所有變動共用同一張 `ledger_entries` 表，等式由 SQL aggregation 自然成立。

#### 4.2.3 反沖（Reversal）必然破壞歷史

B 模型的 `wallet_logs` 只能描述「這筆變動」，不能描述「這筆變動取消上一筆變動」。Chargeback 場景下：

```
T1 (10:00): 玩家儲值 1000  → balance: 0 → 1000, log: +1000
T2 (10:30): 玩家下單 500   → balance: 1000 → 500, log: -500
T3 (15:00): Chargeback     → balance: 500 → ???
```

B 必須選擇：
- (a) 直接 `UPDATE balance = -500`：餘額變負，破壞「餘額 ≥ 0」業務不變式
- (b) 把原 T1 log 修改為「已撤銷」並 UPDATE balance：**修改歷史**，違反不可竄改性
- (c) Insert 新 log `delta = -1000, reason = chargeback`，UPDATE balance = -500：與 (a) 相同問題
- (d) 客服手動處理：失敗

C 模型中：

```
T3: 寫入 reversal transaction
   Entry A: Bank.TrustAccount         -1000 (退回給 chargeback 處理通路)
   Entry B: Player.Wallet.42          +1000 (玩家負債歸零)
   注意：Player.Wallet.42 此時可能負餘額，由 application 層 freeze 帳戶處理
   原 T1, T2 entries 完全保留，反沖以新事件表達
```

歷史不變、責任歸屬清晰、可被外部審計重建。

#### 4.2.4 「先 B 再 C」的遷移成本超過直接 C

最常見的論點是「先用 B 上線，量起來再升級 C」。但實務上：

- B 上線後產生的歷史資料**沒有 entry 結構**，無法直接遷至 C
- 必須寫腳本**重建**所有歷史的雙簿記表達——對每個 transaction 都需要回答「這是哪兩個帳戶之間的交易」，原始 logs 通常不夠
- 遷移過程中如何證明「遷移後的 ledger = 遷移前的 wallet 狀態」？
- 遷移期間所有財務查詢、結算、報表都不可用

直接以 C 起步，初期工程成本約多 5–10 人天（表結構、trigger、idempotency、SDK），但**這是攤提到全專案壽命週期最便宜的選擇**。

### 4.3 為何選 C 而非 D（三簿記 / 區塊鏈錨定）

D 的核心特性是「外部不可竄改性證明」。對 prophetica 場景：

| 議題 | C 雙簿記 | D 區塊鏈錨定 |
|------|---------|-------------|
| 內部不可竄改 | DB trigger + WORM 月度快照（SPEC-002 §10）| 同 + 區塊鏈簽章 |
| 對外證明 | 第三方審計簽核月報 | 區塊鏈交易記錄 |
| 監管接受度 | 高（傳統會計）| 中（仍在演進中）|
| 成本 | 低 | 較高（鏈上 gas、簽章服務）|
| 玩家信任度 | 仰賴審計事務所 | 公鏈可獨立驗證 |
| 工程複雜度 | 標準 | 高（鍵管理、鏈互動失敗處理） |

D 在加密原生交易所（Kraken Proof-of-Reserves、Binance PoR）已是標配，因為玩家對中心化交易所的信任度低。對 prophetica：

- 玩家儲值法幣、領回的是站內點數（不可出金），「公開可驗證儲備」對玩家心智價值不高
- 監管不要求區塊鏈錨定
- 引入鏈上組件會增加密鑰管理、鏈失敗 fallback、跨境合規等不必要複雜度

> **未來若擴展支援加密貨幣儲值（極長期），D 可作為 C 之上的疊加層**，由 ADR-XXXX 記錄該擴展。本階段不採用。

### 4.4 為何選 C 而非 E（第三方 Ledger SaaS）

E（Modern Treasury / Fragment.io 等）能 outsource 帳本實作。優點：

- 帳本邏輯由 SaaS 處理，工程團隊聚焦業務
- 內建報表、對帳、合規模板
- 較少 ledger bug

缺點足以否決：

| 議題 | 影響 |
|------|------|
| 跨境資料主權 | 玩家完整金流副本傳至第三方（多在美國），會增加資料跨境傳輸與供應商控管風險 |
| 廠商鎖定 | 帳本是核心系統，遷移成本極高 |
| API 延遲 | 撮合成交需 ledger 確認，跨網路 RTT 嚴重影響撮合 SLO |
| 客製化能力 | 推薦分潤、結算、客服補償等業務邏輯無法完全自訂 |
| 成本 | 規模上去後（>$100M annual flow）SaaS 計價飆升 |
| 法律對接 | 帳本是執照地稽核重點，借用第三方需簽資料處理協議 |

E 否決。但本決策中採納其**設計理念**：SPEC-002 §4.2 的 Chart of Accounts 直接借鏡了 Modern Treasury 的帳戶分類最佳實踐。

## 5. 雙簿記的核心不變式（Invariant Reaffirmation）

> 此節為 SPEC-002 §5 的決策層級複述，目的是讓這些不變式從「實作要求」升格為「架構紅線」。
>
> **§5 INV 集合自 v1.0.2 起由 [ADR-0006](./ADR-0006-ledger-invariant-set-extension-inv6.md) §4.1 擴展為 INV-1 ~ INV-6**：(a) INV-2 公式擴展含 `Player.Frozen.*`；(b) INV-4 範圍擴展含 `Player.Frozen.X`；(c) 新增 INV-6 `Player.Wallet.*` outflow 白名單（連動 R-1 紅線清除）。本表保留原 v1.0 形式作為 ADR-0003 之歷史紀錄；**現行有效 INV 集合請以 ADR-0006 §4.1 為準**。

| ID | 不變式（v1.0 原文，已被 ADR-0006 §4.1 擴展）| 機械化檢查方式 |
|----|--------|---------------|
| **INV-1** | 每個 ledger_transaction 之 entries 借貸代數和為 0 | DB CONSTRAINT TRIGGER（每筆 commit 時檢查）|
| **INV-2** | Σ(Player.Wallet.\*) + Σ(Player.Locked.\*) = 平台對玩家負債總額（**ADR-0006 §4.1 擴展含 Frozen**）| 每 5 分鐘對帳腳本 |
| **INV-3** | Bank.TrustAccount + Σ(Revenue.\*) - Σ(Expense.\*) = 平台對玩家負債總額 | 每日 T+1 對帳 |
| **INV-4** | Player.Wallet.X 任意時刻 ≥ 0（chargeback 反沖外，**ADR-0006 §4.1 擴展含 Player.Frozen.X**）| application + DB CHECK |
| **INV-5** | 每個 ledger_entry 一旦寫入不可 UPDATE / DELETE | DB role 權限 + audit |
| **INV-6** | （ADR-0006 §4.1 新增）`Player.Wallet.*` outflow 白名單，禁止流向 `Liability.AccountsPayable.*` / `Bank.OperatingAccount.*` 等任何兌現科目 | DB CONSTRAINT TRIGGER + CI sentinel-scan grep gate |

任何違反 → 系統進入 P0 安全模式（停盤、凍結出帳）→ 工程 + Compliance 介入。

## 6. Consequences

### 6.1 Positive

1. **bug 自我暴露**：INV-1 在 DB 層強制，任何「忘記寫對手 entry」「金額算錯」「漏更新某個帳戶」都會直接讓 transaction commit 失敗，工程師當場知道 bug
2. **監管稽核可機械化**：「請給我前月平台對玩家負債總額」一條 SQL 即可：`SELECT SUM(balance_after) FROM ledger_entries WHERE ... GROUP BY account WHERE type = 'liability'`
3. **歷史完整不可竄改**：任何 reversal 都是新事件，原事件保留，可重現任意時間點之 system state
4. **跨幣別 / 跨 entity 自然支援**：未來支援 USD 點數、Op Co. 切多家、不同手續費結構，只需新增 account，不需改帳本核心
5. **與業界對話成本低**：招募有銀行 / 證交所 / 加密交易所背景之工程師時，「我們是雙簿記」是業界共通語言
6. **自然支援多階段事件**：玩家掛單鎖倉 → 部分成交 → 撤回剩餘 → 結算，每階段有獨立 entries，可依需要拆解或合併查詢
7. **相容外部審計工具**：標準會計軟體（Quickbooks、Xero）皆能 import double-entry 格式

### 6.2 Negative

1. **每筆業務事件至少兩條 entry**：撮合 1 筆成交 = 6 條 entries（買方 wallet/locked、賣方 wallet/locked、雙方手續費），寫入吞吐成倍
2. **schema 紀律要求高**：Chart of Accounts 命名、帳戶類型、direction 規則都需嚴格遵循；新工程師上手有 1–2 週學習曲線
3. **無法直接 UPDATE balance**：客服補償、行銷活動、月底調整都必須走 transaction，比直接改 balance 慢
4. **Idempotency 強制**：每筆 transaction 必須帶 idempotency_key，否則 retry 會重複扣款；增加 application 層複雜度
5. **大量小額交易場景**：若未來支援 micro-bet（< 0.01 點），單筆事件成本可能不經濟
6. **報表查詢需 aggregation**：取「user 42 當前餘額」需 SUM entries（雖可用 cache + balance_after 欄位）
7. **DB trigger 性能影響**：CONSTRAINT TRIGGER DEFERRABLE 在 commit 時才檢查，但仍有 CPU 成本

### 6.3 Mitigation Plan

針對 Negative #1（寫入吞吐）：

- 撮合事件**先寫 Kafka**，由 ledger worker batch consume（SPEC-002 §9.1）
- ledger 寫入採 100ms 一批，每批 ~1000 events
- ledger_entries 表按月 partition，避免單表過大

針對 Negative #2（schema 紀律）：

- 建立 `packages/go-sdk/ledger` 共用函式庫，封裝常用 transaction 模板（deposit、order_lock、trade、settle、reversal）
- 業務 service 不直接寫 entries，呼叫 ledger SDK
- code review 4-eye 強制（SPEC-010 §9.2）

針對 Negative #3（客服補償）：

- 提供 `compensation` 專屬 transaction type，附 4-eye 簽核 metadata
- UI 工具讓客服可「以結構化方式」提交補償提案，背後仍走標準 ledger transaction

針對 Negative #4（idempotency）：

- HTTP API 層強制要求 `Idempotency-Key` header（SPEC-002 §6.1）
- Kafka consumer 採 event_id 為 idempotency_key
- 7 天過期清理機制

針對 Negative #6（讀取性能）：

- `ledger_entries` 加 `balance_after BIGINT` 欄位，每筆 entry 寫入時計算當下該帳戶餘額
- 查詢餘額直接 `SELECT balance_after FROM ledger_entries WHERE account_id = X ORDER BY id DESC LIMIT 1`
- Redis cache `wallet:{user_id}` → 毫秒級查詢

## 7. Compliance & Legal Boundary

本決策強制以下 codebase 紅線：

```
1. wallet 服務禁止暴露 "SetBalance" / "AdjustBalance" 等直接餘額操作 API
   所有餘額變動必須走 PostTransaction 介面，產生 ledger_transaction + entries

2. 禁止 SQL UPDATE / DELETE on ledger_entries
   - DB role 設定：ledger_app 角色僅有 INSERT 權限，無 UPDATE/DELETE
   - 唯一例外：每月歸檔流程的特定 partition 切換

3. 禁止應用層繞過 DB trigger
   - 不允許以 superuser 身份執行業務邏輯
   - 不允許 SET session_replication_role = 'replica' 跳過 trigger

4. 禁止 wallet 餘額有「真相來源」表
   - 不可建立 wallets.balance 列作為主要查詢來源
   - balance_after 欄位僅作為 cache，與 entries SUM 不一致即觸發告警
```

任何違反 → INV-1 至 INV-5 失效 → 帳本不可信 → 平台對玩家負債無法證明 → 執照地稽核失敗 → 牌照撤銷風險。

## 8. Validation Criteria

本決策被視為「成功落地」需滿足：

- [ ] PostgreSQL schema 完成（ledger_accounts / ledger_transactions / ledger_entries）
- [ ] DB CONSTRAINT TRIGGER 強制 INV-1，並通過故意失衡測試（嘗試 commit 不平衡 transaction 必須 fail）
- [ ] DB role 權限分離（ledger_app 僅 INSERT，ledger_admin 不存在 UPDATE/DELETE 預設權限）
- [ ] `packages/go-sdk/ledger` SDK 完成，業務 service 不直接寫 entries
- [ ] 對帳腳本驗證 INV-2、INV-3，模擬 1M 筆 entries 可在 5 分鐘內檢查完成
- [ ] 撮合 + ledger 端到端壓測：1 筆 trade → 6 entries → 全部 commit 成功，吞吐達 SPEC 標準
- [ ] WORM 月度快照（hash chain）寫入 R2 Object Lock 桶，並通過驗證腳本
- [ ] 第三方安全 / 財務審計通過（Trail of Bits 程式碼審計 + 國際會計師事務所 ledger 設計 review）
- [ ] 30 天連續對帳零誤差（INV-2、INV-3 全綠）

## 9. References

### 9.1 內部文件

- [SPEC-002 §4](../../../specs/SPEC-002-funds-safety.md) — 雙簿記實作（表結構、trigger、典型範例）
- [SPEC-002 §5](../../../specs/SPEC-002-funds-safety.md) — 不變式清單
- [SPEC-002 §6](../../../specs/SPEC-002-funds-safety.md) — Idempotency 機制
- [SPEC-002 §7](../../../specs/SPEC-002-funds-safety.md) — 對帳機制
- [ADR-0001](./ADR-0001-order-book-with-dmm-over-lmsr.md) — 撮合決策（消費 ledger 之主要事件流）
- [ADR-0002](./ADR-0002-three-company-corporate-structure.md) — 三公司架構（ledger 跨 entity 設計依據）

### 9.2 外部參考

- Pacioli, Luca (1494). *Summa de arithmetica, geometria, proportioni et proportionalità* — 雙簿記原始文獻
- Martin Kleppmann (2017). *Designing Data-Intensive Applications* Ch. 11（事件溯源）
- Stripe Engineering Blog — "How Stripe's Ledger Works"
- Modern Treasury Documentation — "Ledger Architecture Best Practices"
- Square Engineering — "Ledger: Stripe's Tool for Payments Infrastructure"
- "Triple-entry Accounting" — Ian Grigg (2005)（D 方案的學術根源，本案不採用）

## 10. Notes for Codex / Future Engineers

當你在 wallet-ledger-service 或 settlement-worker 動手時：

1. **任何餘額變動必須走 `PostTransaction` 函式**，禁止 `db.Exec("UPDATE wallet ...")`。如需臨時補償，由客服系統提交 compensation transaction（SPEC-002 §8）
2. **`ledger_entries` 表沒有 UPDATE / DELETE 路徑**：你的 SQL 中不應出現對該表的 UPDATE 語句。發現任何遷移腳本或 admin tool 嘗試 UPDATE 該表 → 立即停止並通知 Tech Lead
3. **Idempotency 是合約，不是建議**：每個 PostTransaction 必須有 idempotency_key，重複 key 必須返回原結果。撮合事件 → trade_id；webhook → gateway_txn_id；client 提交 → header `Idempotency-Key`
4. **每個 transaction 必須有對應的 transaction_type enum**：禁止 `txn_type = 'misc'` 之類的萬用值。每個業務場景對應一個 type
5. **新增帳戶類別（accounts.type）需 ADR**：當前有 asset / liability / revenue / expense / suspense 五類，新增需論證
6. **balance_after 欄位寫入時計算**：不可在查詢時 lazy 計算，避免並發下不一致
7. **跨服務 transaction**：不存在「跨服務的 ACID transaction」，跨服務的業務事件靠 Kafka outbox + saga pattern 處理（SPEC-008 §4）
8. **若你認為某次性能優化值得「先 update wallet 再 entries」**：請先閱讀本 ADR §4.2.4 與 §7。性能優化的合法路徑是 batch consume / cache，不是繞過雙簿記
9. **若你發現對帳差異**：禁止「自動修正」。寫入 `Suspense.Reconciliation` 暫掛、觸發 P0 告警、由人工以新 reversal transaction 修正
10. **若你認為本 ADR 的決策應該被推翻**：寫一份 ADR-XXXX 標記本 ADR 為 Superseded，附上替代方案的論證、遷移計畫、Compliance 簽核——不要直接改 schema

## 11. Pre-Execution Validation Items（動工前驗證項目）

> **MADR 體裁註記**：本節非「待重新決定的問題」（ADR 已 Accepted，雙簿記為架構定案）。以下為動工前必須驗證的實作細節，由 wallet-ledger-service 第一個 PR 之前釐清。所有跨 SPEC 的決策議題改由 [SPEC-000 §5](../../../specs/SPEC-000-index.md) 中央追蹤表收斂。

| ID | 待驗證項目 | 驗證對象 | 影響 |
|----|----------|---------|------|
| V-1 | ledger 是否拆獨立 microservice（呼應 SPEC-014 服務清單）或與 wallet 同 service 不同 module | Tech Lead + 撮合 / 結算工程團隊 | 部署拓撲、跨服務 transaction 邊界 |
| V-2 | `ledger_entries` partition 策略：每月 / 每週 / 每年 | DBA + SRE | 查詢性能、歸檔成本 |
| V-3 | 客服 `compensation` transaction 的 4-eye 簽核介面：自建 vs Workflow 工具（如 Temporal / Airflow） | Engineering + Compliance | 建置時程、稽核可被監管接受度 |
| V-4 | 跨 Operating Co.（未來 ADR-0002 升級 Option E 雙執照）的 ledger 是否完全分離，或共用 ledger 用 `entity_id` 區分 | Tech Lead + Compliance | 多執照擴展時的遷移路徑 |
| V-5 | 點數計價單位敲定後（追蹤於 SPEC-000 §5）對 ledger schema 是否需變更 | Tech Lead | 若決定 1 點 = USD 0.01 則 currency 列預設值需變更 |

V-1 ~ V-2 標記為 **must-resolve**（影響第一個 migration script），需於 wallet-ledger-service 第一個 PR 之前釐清。V-3 ~ V-5 標記為 **defer-acceptable**（可在第一次客服補償 / 雙執照需求出現前處理）。

## 12. Revision History

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-05 | v1.0 | Tech Lead | 首版 Accepted |
| 2026-05-05 | v1.0.1 | Tech Lead | typo-level：§1 對 SPEC-002 不變式的引用由「INV-3」修正為「INV-1」，以對齊本 ADR §5 之編號（SPEC-002 §5 同步重排）。決策內容無變動。 |
| 2026-05-05 | v1.0.2 | Tech Lead | typo-level：§11「Open Questions」改名為「Pre-Execution Validation Items」並重組為驗證表格，將跨檔決策議題改由 SPEC-000 §5 中央追蹤表收斂。決策內容無變動。 |
| 2026-05-07 | v1.0.3 | Tech Lead | **§5 INV 集合 partial superseded by [ADR-0006](./ADR-0006-ledger-invariant-set-extension-inv6.md)**：(a) INV-2 公式擴展含 `Player.Frozen.*`（事後 governance 追認 SPEC-002 v0.1.1 自我排除 / Compliance 凍結 / EDD 待審子科目）；(b) INV-4 範圍擴展含 `Player.Frozen.X`（同上）；(c) 新增 INV-6 `Player.Wallet.*` outflow 白名單（連動 [SPEC-000 §5 R-1](../../../specs/SPEC-000-index.md) 紅線清除）。本表保留原 v1.0 形式作為歷史紀錄，現行有效 INV 集合請以 ADR-0006 §4.1 為準。本次屬於「INV 集合擴展」而非「決策反轉」（雙簿記主決策維持 Accepted），故不採 Superseded 狀態；嚴格論本次已超出「typo-level」範疇但屬於「事後 governance 追認 + 紅線強化」之 boundary case，比照 ADR-0001 v1.0.2 處理。 |
