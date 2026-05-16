# ADR-0006 — 帳本不變式集合擴展：新增 INV-6（Player.Wallet outflow 白名單）+ INV-2 / INV-4 對齊 Frozen 子科目

| 欄位 | 內容 |
|------|------|
| **Status** | Accepted |
| **Date** | 2026-05-07 |
| **Deciders** | Tech Lead, Founder |
| **Consulted** | Compliance Counsel（待法律意見書最終覆核）、預定：執照地會計師、外部審計事務所 |
| **Informed** | 全工程團隊、Codex 開發代理、財務主管 |
| **Tags** | ledger · finance · invariant · audit · compliance · core-architecture |
| **依賴** | ADR-0003, SPEC-002, SPEC-011 |
| **影響** | ADR-0003 §5（partial superseded — INV-2 / INV-4 公式擴展、新增 INV-6） |

---

## 1. Context and Problem Statement

ADR-0003 v1.0 於 2026-05-05 拍板雙簿記帳本，§5 列出 5 條核心不變式 INV-1 ~ INV-5。但其後幾輪 SPEC 演化暴露兩個問題：

### 1.1 INV-2 / INV-4 公式已被 SPEC-002 擴展但 ADR-0003 §5 未同步

| INV ID | ADR-0003 v1.0 §5 公式 | SPEC-002 v0.1.1 起實際公式 | Drift 來源 |
|--------|----------------------|---------------------------|----------|
| INV-2 | `Σ(Player.Wallet.*) + Σ(Player.Locked.*) = 平台對玩家負債總額` | `Σ(Player.Wallet.*) + Σ(Player.Locked.*) + Σ(Player.Frozen.*) = 平台對玩家負債總額` | SPEC-002 v0.1.1 新增 `Player.Frozen.{user_id}` 子科目（Compliance 凍結 / EDD 待審）|
| INV-4 | `Player.Wallet.X 任意時刻 ≥ 0` | `Player.Wallet.X` 與 `Player.Frozen.X` 任意時刻 ≥ 0 | 同上 |

ADR README §修訂規則「Accepted 後不可直接修改 ADR 主體」嚴格論已被 violated — SPEC-002 v0.1.1 變更時應該先寫 ADR 把 INV-2 / INV-4 公式擴展，再改 SPEC，但實際是先改 SPEC 後 ADR 沒同步。本 ADR 補上這個 governance gap。

### 1.2 R-1 紅線清除暴露 INV 集合對「銅幣出口」的保護不足

[SPEC-000 §5 R-1](../../../specs/SPEC-000-index.html) 業主決議 2026-05-07：銅幣**永不可回兌**，包含 B2B 應付帳款 / 外部資產 / 任何形式之兌換。SPEC-011 §6 整節重寫切斷「玩家錢包 → 應付帳款」之 application 層路徑。

但 application 層保護有以下風險：
- 任何 service code bug、admin UI 誤操作、未來新功能設計變更，可能不小心引入 `Player.Wallet → AccountsPayable` 之 ledger entry
- application check 不是 zero-trust：admin 後台繞過 application 層直連 PG 即可寫入
- ADR-0003 §5 既有 INV-1 ~ INV-5 都不能阻擋此類「銅幣出口」變動 — INV-1 借貸代數和為 0 對「Wallet 減 / AccountsPayable 加」這類 entry 完全合法

需要把紅線**下沉到 ledger 層**，由 PostgreSQL CHECK constraint + DB CONSTRAINT TRIGGER 強制執行。

## 2. Decision Drivers

依重要性遞減排序：

1. **R-1 紅線零容忍**：銅幣不可回兌必須在 ledger 層由 DB 強制，不可僅依賴 application check
2. **Frozen 子科目擴展正名**：INV-2 / INV-4 公式 drift 已存在 ~6 週，需要正式 governance 補正，避免未來 audit 時無法解釋「為何 ADR §5 與 SPEC §5 公式不一致」
3. **不引入 INV 編號斷裂**：ADR-0003 §5 從 INV-1 ~ INV-5，新增 INV-6 自然延續編號；不重編 INV-2 / INV-4 為新編號（避免破壞 SPEC-002 / 對帳腳本 / SIEM alert / runbook 既有引用）
4. **ADR governance 一致性**：ADR-0003 §5 INV 集合為「決策層級複述」，本 ADR 不取代 ADR-0003 主決策（雙簿記）僅擴展 §5 INV 集合
5. **可機械化檢查**：每條 INV 必須有對應的 mechanical check（不是文字承諾），便於 CI / 對帳腳本 / DB trigger 落地

## 3. Considered Options

### Option A — 不寫 ADR，直接在 SPEC-002 §5 加 INV-6

延續 SPEC-002 v0.1.1 / v0.1.5 之做法，先改 SPEC 不寫 ADR。實際上 SPEC-002 v0.1.5 已先 publish INV-6 並標「待 ADR-0006 落地」。

**否決理由**：ADR-0003 §5 開頭明示「INV 編號或語意必須先寫一份新 ADR」。Option A 與該 governance 直接衝突；長期看會讓 ADR 失去權威。

### Option B — 寫 ADR-0006 並把 ADR-0003 標為 Superseded by ADR-0006

把 ADR-0003 整份標 Superseded，ADR-0006 重新定義雙簿記 + INV-1 ~ INV-6。

**否決理由**：ADR-0003 主決策（雙簿記 vs 單簿記）未變，Option B 過度。新進工程師看到「ADR-0003 Superseded」會誤以為雙簿記決策有變，溝通成本高。

### Option C — 寫 ADR-0006 為「INV 集合擴展」⭐

ADR-0006 範圍嚴格限定為「ADR-0003 §5 INV 集合擴展」：
- 正式追認 INV-2 / INV-4 公式擴展含 `Player.Frozen.*`（事後 governance 補正）
- 新增 INV-6（Player.Wallet outflow whitelist）
- ADR-0003 標為「Accepted（§5 INV 集合由 ADR-0006 擴展）」，主決策維持 Accepted
- 本 ADR 與 ADR-0003 並列，雙簿記架構 = ADR-0003 + ADR-0006 之 union

### Option D — 把 INV-6 拆為獨立 ADR，INV-2 / INV-4 對齊另寫 ADR

把兩件事拆為 ADR-0006（INV-6）+ ADR-0007（INV-2 / INV-4 對齊）。

**否決理由**：兩件事都涉及「ADR-0003 §5 INV 集合」演化，拆兩份 ADR 增加閱讀成本；且 Frozen 子科目的擴展與 INV-6 outflow whitelist 邏輯上獨立但都屬於「INV 集合擴展」主題，合併單一 ADR 處理是 ADR README §修訂規則「一個 ADR 處理一個決策」的合理詮釋（決策 = INV 集合擴展）。

## 4. Decision Outcome

**選擇 Option C：寫 ADR-0006 為「INV 集合擴展」，ADR-0003 維持 Accepted 但標 §5 由本 ADR 擴展。**

### 4.1 ADR-0003 §5 INV 集合的擴展形式

擴展後完整集合（v1.0 對齊 SPEC-002 v0.1.5）：

| ID | 不變式 | 機械化檢查方式 | 來源 |
|----|--------|---------------|------|
| **INV-1** | 每個 ledger_transaction 之 entries 借貸代數和為 0 | DB CONSTRAINT TRIGGER（每筆 commit 時檢查）| ADR-0003 v1.0 §5 |
| **INV-2** | `Σ(Player.Wallet.*) + Σ(Player.Locked.*) + Σ(Player.Frozen.*) = 平台對玩家負債總額` | 每 5 分鐘對帳腳本 | ADR-0003 v1.0 §5（公式由 ADR-0006 擴展含 Frozen） |
| **INV-3** | `Bank.TrustAccount + Σ(Revenue.*) − Σ(Expense.*) = 平台對玩家負債總額` | 每日 T+1 對帳 | ADR-0003 v1.0 §5 |
| **INV-4** | `Player.Wallet.X` 與 `Player.Frozen.X` 任意時刻 ≥ 0（chargeback 反沖外）| application + DB CHECK | ADR-0003 v1.0 §5（範圍由 ADR-0006 擴展含 Frozen） |
| **INV-5** | 每個 ledger_entry 一旦寫入不可 UPDATE / DELETE | DB role 權限分離 + audit | ADR-0003 v1.0 §5 |
| **INV-6** | `Player.Wallet.{user_id}` 之 outflow（direction = +1，債務減少）目的地白名單，僅限：(a) `Player.Locked.{user_id}`、(b) `Player.Frozen.{user_id}`、(c) `Revenue.TradingFee` / `Revenue.SettlementFee`、(d) `Player.Wallet.{other_user_id}`（**僅限** `txn_type ∈ {trade, settle}`）、(e) `Bank.TrustAccount.*`（**僅限** `txn_type = chargeback`，呼應 SPEC-002 §8.1 deposit 反向沖正；非「銅幣變現」路徑，玩家不取得外部資產）、(f) `Expense.ManualPointsGrant`（**僅限** `txn_type = manual_points_adjustment` 且 `metadata.adjustment_direction = 'deduct'` 且 `metadata.actor_role ∈ {'admin','manager'}`，此為 MVP 手動發點成本沖回，非對外付款）；**禁止**流向 `Liability.AccountsPayable.*` / `Bank.OperatingAccount.*` / `Expense.AffiliatePartnerFee` / 任何其他銀行 / 支付通路 / 外部錢包科目 | DB CONSTRAINT TRIGGER（拒絕 commit）+ CI sentinel-scan grep gate | **ADR-0006 新增**（呼應 SPEC-011 §6.3 R-1 紅線；chargeback carve-out 對齊 SPEC-002 §8.1；manual adjustment carve-out 對齊 SPEC-002 v0.1.9）|

### 4.2 INV-6 的 mechanical check 落地策略

INV-6 需要 application 層、DB 層、CI 層三層保護：

#### 4.2.1 DB 層（強制）

```sql
-- migrations/wallet-ledger/202605xx_inv6_player_wallet_outflow_whitelist.sql

CREATE OR REPLACE FUNCTION fn_inv6_outflow_whitelist_check()
RETURNS TRIGGER AS $$
DECLARE
    v_account_code     TEXT;
    v_account_type     TEXT;
    v_pair_account_code TEXT;
BEGIN
    -- 取本 entry 對應之 account
    SELECT code, type INTO v_account_code, v_account_type
    FROM ledger_accounts WHERE id = NEW.account_id;

    -- INV-6 僅檢查「Player.Wallet.* 之 outflow（debit, direction = +1）」
    IF v_account_code NOT LIKE 'Player.Wallet.%' OR NEW.direction <> 1 THEN
        RETURN NULL;
    END IF;

    -- 檢查同 transaction 內其他 entries（即配對科目），目的地必須在白名單
    -- 白名單對齊 §4.1 表 INV-6 行 (a)-(e) 五類允許目的地
    PERFORM 1
    FROM ledger_entries e2
    JOIN ledger_accounts a2 ON e2.account_id = a2.id
    WHERE e2.txn_id = NEW.txn_id
      AND e2.id <> NEW.id
      AND e2.direction = -1     -- credit / 增加之配對方
      AND NOT (
          a2.code LIKE 'Player.Locked.%'
          OR a2.code LIKE 'Player.Frozen.%'
          OR a2.code = 'Revenue.TradingFee'
          OR a2.code = 'Revenue.SettlementFee'
          OR (a2.code LIKE 'Player.Wallet.%'
              AND (SELECT txn_type FROM ledger_transactions WHERE id = NEW.txn_id) IN ('settle', 'trade'))
          -- (e) Bank.TrustAccount.* 限 chargeback reversal（SPEC-002 §8.1）
          OR (a2.code LIKE 'Bank.TrustAccount.%'
              AND (SELECT txn_type FROM ledger_transactions WHERE id = NEW.txn_id) = 'chargeback')
          -- (f) Expense.ManualPointsGrant 限 manual_points_adjustment deduct（SPEC-002 v0.1.9）
          OR (a2.code = 'Expense.ManualPointsGrant'
              AND EXISTS (
                  SELECT 1
                  FROM ledger_transactions t
                  WHERE t.id = NEW.txn_id
                    AND t.txn_type = 'manual_points_adjustment'
                    AND t.metadata->>'adjustment_direction' = 'deduct'
                    AND t.metadata->>'actor_role' IN ('admin', 'manager')
                    AND COALESCE(t.metadata->>'audit_log_id', '') <> ''
              ))
      );

    IF FOUND THEN
        RAISE EXCEPTION
            'INV-6 violation: Player.Wallet outflow targets non-whitelisted account in txn %',
            NEW.txn_id
        USING ERRCODE = '40004',
              HINT = 'See ADR-0006 §4.1 / SPEC-002 §5 for outflow whitelist; SPEC-011 §6.3 for R-1 red line.';
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER trg_inv6_outflow_whitelist
AFTER INSERT ON ledger_entries
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION fn_inv6_outflow_whitelist_check();
```

> 採 `DEFERRABLE INITIALLY DEFERRED` 與 INV-1 一致，在 commit 時集中檢查；若違反，整個 transaction rollback 並 raise SQLSTATE `40004`（integrity constraint violation 之自定義子碼）。

#### 4.2.2 Application 層（早期捕捉）

`crates/sdk/ledger` 之 `LedgerTxn::validate()` 在 transaction 建構時先 check INV-6（不依賴 DB error 才發現），返回 `LedgerError::Inv6Violation` 含明確錯誤訊息。

#### 4.2.3 CI 層（程式碼禁字）

[SPEC-017 §8](../../../specs/SPEC-017-monorepo-bootstrap.html) sentinel-scan job 加 R-1 grep gate：

```bash
rg -n -e 'Player\.Wallet.*->\s*AccountsPayable' \
   -e 'Player\.Wallet.*->\s*OperatingAccount' \
   -e 'wallet_to_payable|cash_out_player_pts|redeem_points_to_cash' \
   --type rust services/ crates/ \
   && echo "::error::INV-6 / R-1 紅線：禁止 Player.Wallet → 應付/外部資產 路徑" && exit 1
```

### 4.3 INV-2 / INV-4 對齊 Frozen 之追認

ADR-0003 v1.0 §5 之 INV-2 公式「`Σ(Wallet) + Σ(Locked)`」與 INV-4 範圍「`Player.Wallet.X`」於 SPEC-002 v0.1.1 起實際運作為包含 `Player.Frozen.*`。本 ADR §4.1 表格為**事後 governance 追認**，無「重新決定」性質。落地動作：

- ADR-0003 §5 INV 表標註「公式由 ADR-0006 §4.1 擴展，含 Frozen 子科目」
- 對帳腳本（每 5 分鐘 INV-2 check）已於 SPEC-002 v0.1.1 implementation 時包含 Frozen，無需改 code
- ADR-0003 維持 Status: Accepted，於 Revision History 加 v1.0.2 條目指向本 ADR

## 5. Consequences

### 5.1 Positive

1. **R-1 紅線下沉到 ledger 層**：admin 後台 / application bug / 未來新功能變更皆無法繞過 INV-6；DB CONSTRAINT TRIGGER 是 zero-trust 保護
2. **governance 補正**：ADR-0003 §5 與 SPEC-002 §5 之間 ~6 週的 drift 正式對齊；未來 audit 不會出現「為何 ADR §5 公式與 SPEC §5 不一致」之尷尬
3. **INV 編號連續**：INV-1 ~ INV-6 線性編號，對帳腳本 / SIEM alert / runbook 既有引用無需改動
4. **三層保護**：CI grep（早期攔截 PR）+ application validate（早期回應 client）+ DB trigger（最終強制）
5. **與業界對話成本不變**：「我們是雙簿記」仍是業界共通語言，新增的 INV-6 可解釋為「我們在帳本層強制玩家銅幣不可回兌之合規邊界」

### 5.2 Negative

1. **DB trigger 增加 commit overhead**：每筆 ledger entry insert 多一次 sub-query 檢查同 transaction 內其他 entries；估算 P99 增加 50-100μs，對 SPEC-007 §6.4 wallet-ledger budget（2ms）有 buffer 但不可忽略
2. **DEFERRABLE TRIGGER 在大 transaction 中放大檢查時間**：500-batch settlement transaction 之 INV-6 check 時間 O(N²) — 需要 trigger function 加速（如 INV-6 改 statement-level trigger）
3. **migration 需要小心**：既有 ledger_entries 資料若不符合 INV-6（理論上不應該有，但 audit 一定要跑一次驗證腳本）需在 trigger 啟用前一次性掃描
4. **CI grep gate 偽陽性**：未來若有合理用途（如「業務酬勞 cash-out」是合法的，從 `Liability.AccountsPayable` → `Bank.OperatingAccount`），grep 容易誤報；需精確劃定 grep 範圍避免影響開發迭代

### 5.3 Mitigation Plan

針對 Negative #1 / #2：
- 撮合 ledger 寫入路徑（SPEC-002 §9.1 Kafka batch consume）保持 100ms / 1000-event 批次，INV-6 trigger overhead 可被 batch amortize
- Settlement 批次降為 500 筆（SPEC-004 §7.4，已於 v0.1.2 落地）降低單 transaction 內 INV-6 check 次數
- nightly mutation testing 涵蓋 INV-6 trigger（呼應 SPEC-017 §3.3 R-4），caught rate ≥ 75%

針對 Negative #3：
- migration 落地前 ledger 並未啟用 INV-6 因此既有資料應為空（PR #1 之前 ledger 不寫 production 資料）；若未來在已運行系統中啟用，需先跑驗證腳本掃 100% 既有 entries

針對 Negative #4：
- CI grep 規則限縮在「`Player.Wallet → 應付/銀行/外部` 之 ledger 寫入語法」，不誤傷「業務酬勞之獨立帳本軌道」（後者操作的是 `Liability.AccountsPayable.*` 與 `Bank.OperatingAccount.*` 之間的 entry，與 `Player.Wallet.*` 無關）

## 6. Compliance & Legal Boundary

本 ADR 之 INV-6 為「銅幣不可回兌」原則的技術落地。違反 INV-6 之代價：

```
任何成功 commit 違反 INV-6 之 ledger entry：
- DB CONSTRAINT TRIGGER 自動 reject + raise SQLSTATE 40004 → transaction rollback
- 同時觸發 SPEC-007 §7.2 P0 告警（critical level）
- 帳本服務進入 fail-closed 模式（停止接受新 transaction，等待人工介入）
- 工程 + Compliance + 業主 oncall 介入；4-eye review 之事故 RCA 必須在 24h 內提交
- 若違反經由 admin 後台繞過 application 層直接 SQL 執行而觸發：相關操作員立即停權 + 法律行動
```

## 7. Validation Criteria（Pre-Execution Validation Items）

> ADR README §修訂規則：Accepted 狀態之 ADR 此節命名為 Pre-Execution Validation Items（驗證項目，非待決議題）。

本 ADR 落地需滿足：

- [ ] `crates/sdk/ledger` 之 `LedgerTxn::validate()` 實作 INV-6 application-level check，含 `LedgerError::Inv6Violation` variant
- [ ] `services/wallet-ledger-service/migrations/` 新增 INV-6 trigger migration，與 INV-1 trigger 並列
- [ ] DB trigger 通過故意違反測試（嘗試 commit `Player.Wallet → AccountsPayable` 之 entry 必須 fail，SQLSTATE = `40004`）
- [ ] DB trigger 通過合法路徑測試（trade settlement / fee deduction / lock 流程必須通過）
- [ ] CI sentinel-scan grep gate 落地，PR #1 通過所有預設 happy path 測試（無誤報）
- [ ] 對帳腳本（每 5 分鐘 INV-2 check）在 staging 環境驗證 1M 筆 entries 含 Frozen 子科目仍可在 5 分鐘內完成
- [ ] SPEC-002 v0.1.5 既有 INV-6 條目維持「pending ADR」標註於 ADR-0006 落地後移除
- [ ] SPEC-002 v0.1.9 manual_points_adjustment carve-out 已同步到 SDK invariant 與 DB trigger；affiliate 扣點與未帶 audit_log_id 的扣點必須 fail
- [ ] ADR-0003 v1.0.2 Revision History 加 cross-ref 至本 ADR
- [ ] SPEC-007 §6.4 wallet-ledger latency budget 壓測驗證 INV-6 trigger overhead 不破 2ms P99

## 8. References

### 8.1 內部文件

- [ADR-0001 §6 v1.0.2](./ADR-0001-order-book-with-dmm-over-lmsr.md) — 平台不下場紅線（連動 R-2）
- [ADR-0002](./ADR-0002-three-company-corporate-structure.md) — 三公司架構（INV-3 之 Bank.TrustAccount 即 Op Co. 託管）
- [ADR-0003](./ADR-0003-double-entry-ledger.md) — 雙簿記主決策（本 ADR §4.1 為 §5 之擴展）
- [SPEC-000 §5 R-1](../../../specs/SPEC-000-index.html) — 銅幣出口紅線追蹤
- [SPEC-002 §4.2.1 / §5 / §8.1](../../../specs/SPEC-002-funds-safety.html) — Frozen 子科目 / INV 表 / Chargeback 處理
- [SPEC-011 §6.3](../../../specs/SPEC-011-affiliate-commission.html) — INV-6 在業務分潤路徑的 application 層引用
- [SPEC-000 §5 R-1](../../../specs/SPEC-000-index.html) — R-1 紅線清除追蹤

### 8.2 外部參考

- PostgreSQL Documentation: [DEFERRABLE Constraints](https://www.postgresql.org/docs/16/sql-set-constraints.html)
- "Building Reliable Distributed Systems" — Pat Helland 之 ledger invariant 模式
- AICPA SAS 145 (2024) — Auditing Standard on Identifying and Assessing Risks of Material Misstatement（雙簿記 invariant 之 audit 對齊參考）

## 9. Notes for Codex / Future Engineers

當你在 SPEC-002 / SPEC-011 對應 task 上動手時：

1. 寫 ledger transaction 前**永遠**先呼叫 `LedgerTxn::validate()`，不要假設 DB trigger 會兜底（trigger 兜底是最後一道，application 提早報錯使開發迭代快）
2. 任何 `Player.Wallet.*` 出現在 `direction = 1` 之 entry 的 PR**必須**標 `wallet-outflow` label 並由 wallet-ledger-service module owner review；CI 自動加 label
3. 業務酬勞 / 營運費出帳走獨立帳本軌道（`Liability.AccountsPayable.*` ↔ `Bank.OperatingAccount.*`），**永遠**與 `Player.Wallet.*` 零交集
4. 若你認為 INV-6 白名單應該擴增（例如新增「玩家銅幣捐贈合作戒癮機構」之路徑）：**寫一份 ADR-XXXX 標記本 ADR §4.1 之 INV-6 為 partial superseded，附上業主 + Compliance + Legal 三方簽核**——不要直接改 trigger function 或 application validate
5. INV-6 trigger 之 plpgsql function 修改視同 ledger 核心模組變更，落入 SPEC-010 §9.3 MS-10 4-eye review

## 10. Revision History

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-07 | v1.0 | Tech Lead | 首版 Accepted；ADR-0003 §5 INV 集合擴展為 INV-1 ~ INV-6；INV-2 / INV-4 公式對齊含 Frozen 子科目（事後 governance 追認）；新增 INV-6 outflow whitelist（連動 R-1 紅線清除）|
| 2026-05-07 | v1.0.1 | Tech Lead | INV-6 白名單補 chargeback carve-out（v1.0 落地過程中發現遺漏）：(a)-(d) 之外新增 (e) `Bank.TrustAccount.*`（**僅限** `txn_type = chargeback`，呼應 SPEC-002 §8.1 deposit 反向沖正）。理由：chargeback 為「玩家信用卡爭議 → 銀行抽走原儲值 → 平台對玩家負債歸零 + 託管帳餘額減少退回卡組織」之合法 reversal，玩家**不**取得外部資產；§4.1 INV-6 行 + §4.2.1 plpgsql trigger 同步更新。發現過程：跑 wallet-ledger-service `chargeback_can_project_wallet_above_zero` test 時觸發 INV-6 偽陽性。屬於 v1.0 落地補正（typo / erratum 級），主決策方向未變動。 |
| 2026-05-11 | v1.0.2 | Tech Lead | INV-6 白名單補 SPEC-002 v0.1.9 MVP `manual_points_adjustment` carve-out：(f) `Expense.ManualPointsGrant` 僅允許 `manual_points_adjustment` 扣點、`metadata.adjustment_direction = 'deduct'`、`metadata.actor_role ∈ {'admin','manager'}` 且帶 `audit_log_id`。此為手動發點成本沖回，不是銅幣回兌或對外付款；affiliate 扣點仍為 forbidden。 |
