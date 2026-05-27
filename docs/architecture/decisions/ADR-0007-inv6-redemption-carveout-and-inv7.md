# ADR-0007 — 帳本不變式集合再擴展：INV-6 新增 (g) UnredeemedEntitlement carve-out + INV-2 / INV-4 對齊 UnredeemedEntitlement + 新增 INV-7（redemption 同人綁定）

| 欄位 | 內容 |
|------|------|
| **Status** | Proposed |
| **Date** | 2026-05-27 |
| **Deciders** | Tech Lead, Founder |
| **Consulted** | Compliance Counsel（待法律意見書最終覆核）、財務主管、Codex 開發代理 |
| **Informed** | 全工程團隊 |
| **Tags** | ledger · finance · invariant · audit · compliance · core-architecture · redemption |
| **依賴** | ADR-0003, ADR-0006, SPEC-002, SPEC-024 |
| **影響** | ADR-0003 §5（partial — INV-2 / INV-4 公式再次擴展）、ADR-0006 §4.1（INV-6 表新增 (g) 行）、新增 INV-7 |

---

## 1. Context and Problem Statement

[SPEC-000 §5 Q-10](../../../specs/SPEC-000-index.html#cross-cutting-q10) 業主決議 2026-05-27：銅幣**不可**會員間轉移／贈送，但**可**用於購買虛擬商品或兌換特定服務。[SPEC-002 §4.4.7](../../../specs/SPEC-002-funds-safety.html) v0.1.12 把禁令明文化、§5 INV-6 補上「亦禁止任何會員間 wallet-to-wallet 主動轉點」之文字保障。

但 Q-10 的「正面用途」— 點數兌換虛擬商品 / 服務 — 在 SPEC、proto、ledger、service crate 全部都還沒有規格。本 ADR 解決其中**帳本層**的 governance 問題。實作層由 [SPEC-024](../../../specs/SPEC-024-redemption-entitlement.html) 與其 [implementation plan](../../implementation-plans/SPEC-024-redemption-entitlement-implementation-plan.html) 承擔。

### 1.1 為何 INV-6 white-list 必須擴展

ADR-0006 §4.1 之 INV-6 white-list 把 `Player.Wallet.*` outflow 之合法目的地限縮為 (a)~(f) 六類。`Liability.UnredeemedEntitlement.*` 不在其中 — 因此目前任何 redemption 嘗試都會被 DB trigger 直接 reject。要實作 SPEC-024，必須先在 INV-6 加 (g) carve-out。

### 1.2 為何僅靠 INV-6 white-list 不夠 — 需要 INV-7

假設我們僅在 INV-6 加上 `Liability.UnredeemedEntitlement.*` 為合法目的地，沒有其他保護：

```
惡意 / bug：
  DR Player.Wallet.A             +Y
  CR Liability.UnredeemedEntitlement.B  -Y       ← B 不是 A
```

形式上：balanced（INV-1 通過）、目的地 account kind 合法（INV-6 通過）、metadata 帶得齊（trigger 條件通過）。但實質是「A 的點被搬到 B 名下的 entitlement」— **這就是 Q-10 禁止的會員間轉移之偽裝**。

INV-6 是「account-kind 層」的白名單，無法表達「source 與 destination 之 `{user_id}` 必須相同」這個跨 entry 的關聯。因此必須新增 INV-7，明確把 user_id 同人綁定下沉到 ledger 層。

### 1.3 為何 INV-2 / INV-4 需要再次擴展

`Liability.UnredeemedEntitlement.*` 本質仍是平台對玩家之負債（從「欠玩家 Y 銅幣」轉成「欠玩家 Y 點價值之商品」）。
- INV-2 公式（平台對玩家負債總額）若不擴展，redemption 後該負債會「消失」於對帳腳本視野，造成 INV-3（資產 = 負債 + 權益）每 5 分鐘對帳腳本誤報。
- INV-4（非負餘額）若不擴展，UnredeemedEntitlement 可能寫成負值，等同憑空多 issue entitlement。

ADR-0006 對 Frozen 子科目擴展 INV-2 / INV-4 是事後追認；本 ADR 對 UnredeemedEntitlement 之擴展為**事前 governance**（規格與帳本變更同 ADR）。

## 2. Decision Drivers

依重要性遞減排序：

1. **Q-10 禁令零容忍 + 正面流程可用**：必須在不破壞 R-1 / Q-10 紅線的前提下，允許 redemption 走通 ledger
2. **跨 entry 約束需要 DB-level enforcement**：INV-7 之 user_id 同人綁定必須由 DB CONSTRAINT TRIGGER 強制；application-only 不夠
3. **governance 模式對齊 ADR-0006**：carve-out 加在 INV-6 第 (g) 行（延續字母編號）、INV-2 / INV-4 公式擴展、新增 INV-7（延續 INV 編號），維持與 ADR-0006 完全一致的演化模式
4. **不重編現有 INV**：INV-1 ~ INV-6 既有引用（對帳腳本 / SIEM alert / runbook / Codex prompt）無需改動
5. **可機械化檢查**：每條 INV 變更皆需對應 mechanical check（plpgsql trigger / SDK validate / CI grep）

## 3. Considered Options

### Option A — 僅在 INV-6 加 (g)，不寫 INV-7

只擴展 INV-6 white-list，user_id 同人綁定靠 application 層。

**否決理由**：見 §1.2。INV-6 為 account-kind 白名單，本質無法表達 cross-entry user_id 約束；application 層保護不是 zero-trust（admin 後台繞過 application 直連 PG 即可寫入跨 user 之 redemption），完全違背 ADR-0006 §1.2 「下沉到 ledger 層」的精神。

### Option B — 不寫 INV-7，把 user_id 綁定塞進 INV-6 第 (g) 行的條件文字

把 (g) 條款擴張為「目的地 `Liability.UnredeemedEntitlement.{X}` 且 source `Player.Wallet.{X}` 之 user_id 必須相同」。

**否決理由**：兩個問題。(1) INV-6 之 mechanical check 是「對每筆 outflow entry 看其他 credit 配對方」，不適合表達跨 entry 字串解析（regex 解 user_id）；硬塞會讓 trigger function 巨大且難以維護。(2) 違背 INV 「一條 invariant 一個概念」原則 — INV-6 是「account-kind 白名單」，INV-7 是「user-binding」，是兩個正交的不變式，分開比較清晰、未來若放寬其中一者也比較容易。

### Option C — 寫 ADR-0007，按照 ADR-0006 模式擴展 ⭐

ADR-0007 範圍：
1. INV-6 加 (g) `Liability.UnredeemedEntitlement.{user_id}`（限 `redemption_purchase` + metadata 完整）
2. INV-2 / INV-4 公式擴展含 `Liability.UnredeemedEntitlement.*`
3. 新增 INV-7：`redemption_purchase` / `entitlement_fulfillment` / `redemption_refund` / `entitlement_expiry` 系列 txn 中，所有 `Player.Wallet.{X}` 與 `Liability.UnredeemedEntitlement.{Y}` entries 之 `{user_id}` 必須相同

ADR-0003 + ADR-0006 + ADR-0007 之 union 構成最終 INV-1 ~ INV-7 集合。

### Option D — 不寫 ADR，直接在 SPEC-024 + SPEC-002 v0.1.13 加 INV-7

延續 ADR-0006 §1.1 所指出的 governance 反模式（先改 SPEC 後補 ADR）。

**否決理由**：與 ADR-0006 §3 Option A 否決理由完全相同 — ADR-0003 §5 開頭明示「修改 INV 編號或語意必須先寫新 ADR」。

## 4. Decision Outcome

**選擇 Option C**：寫 ADR-0007 為「INV 集合再擴展」，ADR-0003 / ADR-0006 維持 Accepted 但標 §5 / §4.1 由本 ADR 進一步擴展。

### 4.1 INV-6 white-list 新增 (g) 行

| ID | 不變式 | 機械化檢查 | 來源 |
|----|--------|------------|------|
| INV-6 (g) | `Liability.UnredeemedEntitlement.{user_id}` — **僅限** `txn_type = redemption_purchase`，且 metadata 必含 `order_id` / `sku_id` / `audit_log_id`；不得用於其他 txn_type 之 outflow 目的地 | DB CONSTRAINT TRIGGER（擴充 `fn_inv6_outflow_whitelist_check`）+ SDK `is_inv6_whitelisted_target` + CI sentinel-scan 三層 | **ADR-0007 新增**（呼應 SPEC-024 §4.3）|

落地細節：見 [SPEC-024 implementation plan §4](../../implementation-plans/SPEC-024-redemption-entitlement-implementation-plan.html)。

### 4.2 INV-2 / INV-4 公式再次擴展

| INV | ADR-0006 §4.1 版本 | 本 ADR 擴展後 | 動機 |
|-----|---------------------|--------------|------|
| INV-2 | `Σ(Player.Wallet.*) + Σ(Player.Locked.*) + Σ(Player.Frozen.*) = 平台對玩家負債總額` | `... + Σ(Liability.UnredeemedEntitlement.*) = 平台對玩家負債總額` | UnredeemedEntitlement 為平台對玩家「給商品 / 服務」之承諾，仍是負債 |
| INV-4 | `Player.Wallet.X` 與 `Player.Frozen.X` 任意時刻 ≥ 0 | `... 與 Liability.UnredeemedEntitlement.X` 任意時刻 ≥ 0（無 chargeback 例外） | 防止憑空 issue entitlement |

對帳腳本（每 5 分鐘 INV-2 check）與 INV-4 trigger 之 SQL 須同步更新，落地責任在 SPEC-024 implementation plan。

### 4.3 INV-7（新增）

| ID | 不變式 | 機械化檢查 | 來源 |
|----|--------|------------|------|
| **INV-7** | `txn_type ∈ {redemption_purchase, entitlement_fulfillment, redemption_refund, entitlement_expiry}` 之 ledger transaction 中，所有 `Player.Wallet.{X}` 與 `Liability.UnredeemedEntitlement.{Y}` entries 之 user_id 字尾必須相同（去重後僅 1 個）；違反視同 INV-6 等級紅線 | DB CONSTRAINT TRIGGER `fn_inv7_redemption_same_user_check`（DEFERRABLE INITIALLY DEFERRED）+ SDK `LedgerTransactionDraft::validate_redemption_same_user` + CI sentinel-scan grep gate（peer_transfer 字串） | **ADR-0007 新增**（呼應 SPEC-024 §5 / SPEC-002 §4.4.7 / Q-10）|

完整 INV-1 ~ INV-7 對齊表：

| ID | 不變式（取後） | 來源 ADR |
|----|---------------|----------|
| INV-1 | 每筆 transaction 借貸代數和為 0 | ADR-0003 |
| INV-2 | `Σ(Player.Wallet.*) + Σ(Player.Locked.*) + Σ(Player.Frozen.*) + Σ(Liability.UnredeemedEntitlement.*) = 平台對玩家負債總額` | ADR-0003（ADR-0006 + ADR-0007 擴展）|
| INV-3 | `Bank.TrustAccount + Σ(Revenue.*) − Σ(Expense.*) = 平台對玩家負債總額` | ADR-0003 |
| INV-4 | `Player.Wallet.X`、`Player.Frozen.X`、`Liability.UnredeemedEntitlement.X` 任意時刻 ≥ 0（Wallet chargeback 反沖例外） | ADR-0003（ADR-0006 + ADR-0007 擴展）|
| INV-5 | 每筆 ledger_entry 不可 UPDATE / DELETE | ADR-0003 |
| INV-6 | Player.Wallet.* outflow 目的地白名單 (a)~(g) | ADR-0006（ADR-0007 §4.1 補 (g)）|
| **INV-7** | **redemption-family txn 中 Player.Wallet 與 UnredeemedEntitlement 之 user_id 必須同人** | **ADR-0007 新增** |

### 4.4 落地策略對齊 ADR-0006 §4.2 三層保護

- **DB 層**：`fn_inv7_redemption_same_user_check` plpgsql trigger（plan §5 之 SQL template）
- **Application 層**：SDK `LedgerTransactionDraft::validate_redemption_same_user`，回傳 `LedgerError::Inv7Violation`
- **CI 層**：sentinel-scan grep gate 加 `peer_transfer` / `Player.PendingTransfer` / `Revenue.PeerTransferFee` 黑名單；同 PR 落地 ADR-0006 §4.2.3 之 gate 檔案路徑（規劃為 `.github/workflows/wallet-invariant-scan.yml`）

## 5. Consequences

### 5.1 Positive

1. **Q-10 正面流程可實作**：不破壞 R-1 紅線之前提下，redemption / fulfillment / refund / expiry 全流程之 ledger 路徑被打開
2. **跨 user 偽裝攻擊面被堵**：INV-7 把 user_id 綁定下沉到 DB 層；任何企圖借 redemption 形式做 peer transfer 的路徑都在 trigger 層被拒
3. **governance 連續性**：ADR-0003 → ADR-0006 → ADR-0007 構成 INV 集合的線性演化；新進工程師按字母 / 數字序就能理解整套
4. **三層保護一致性**：CI / SDK / DB 三層保護模式與 INV-6 完全對齊，沒有引入新的安全模式
5. **breakage 揭露透明**：`Revenue.EntitlementBreakage` 獨立子科目讓未消費過期憑證之收入認列在年報可被獨立揭露

### 5.2 Negative

1. **DB trigger overhead 再次增加**：INV-7 trigger 跑 regex + array dedup；對 redemption-family txn 之 P99 估算增加 50~150μs（單 txn 2~4 entries）。對其他 txn 之 overhead = 1 個 SELECT txn_type 短查詢
2. **per-user entitlement 子科目膨脹**：與 `Player.Wallet.{user_id}` 同樣是 per-user account，啟用 redemption 後 ledger_accounts 表會多一倍 row 數；對 PG 影響小（B-tree index 仍 O(log N)），但 wallet-balance lookup 查詢需注意 join cost
3. **breakage 政策需法律意見**：部分轄區（如美國）對未使用憑證之 breakage 收入認列有「dormancy period」要求；本 ADR 不對此做最終決議，留給商業上線前 Compliance Counsel 出具意見書
4. **CI gate 偽陽性風險**：`peer_transfer` 字串若未來有合理用途（如「介紹獎勵 ≈ referral」之變體），grep 需 narrow scope 避免誤傷

### 5.3 Mitigation Plan

針對 Negative #1：
- INV-7 trigger 在 SPEC-007 §6.4 wallet-ledger latency budget 內須壓測；若破 budget，改 statement-level trigger（如 ADR-0006 §5.2 Negative #2）
- redemption 屬使用者主動行為，吞吐量遠低於撮合 / settle 之 batch；overhead 對端到端 SLO 衝擊有限

針對 Negative #2：
- per-user `Liability.UnredeemedEntitlement.{user_id}` 採 lazy creation（plan §2 之 INSERT … ON CONFLICT DO NOTHING）
- wallet balance 查詢路徑（SPEC-002 §9.2）必須對 4 個 per-user 子科目（Wallet / Locked / Frozen / UnredeemedEntitlement）一次查回；確保 Redis 快取 key 同步擴展

針對 Negative #3：
- SPEC-024 §7 Open Question OQ-2 留 entitlement_ttl_days 之具體值待 Compliance / 法務輸入
- 在執照地正式上線前，breakage 政策必須有法律意見書 cross-ref；本 ADR 不阻擋立即落地 ledger 結構

針對 Negative #4：
- CI gate 規則精準到「`peer_transfer_` prefix」/「`Player.PendingTransfer.` prefix」/「`Revenue.PeerTransferFee` 完整字串」，避免誤傷單字 `peer` / `transfer`

## 6. Compliance &amp; Legal Boundary

INV-7 之違反代價同 INV-6：

```
任何成功 commit 違反 INV-7 之 ledger entry：
- DB CONSTRAINT TRIGGER 自動 reject + raise SQLSTATE 40004 → transaction rollback
- 同時觸發 SPEC-007 §7.2 P0 告警
- 帳本服務進入 fail-closed 模式
- 工程 + Compliance + 業主 oncall 介入；4-eye review 之事故 RCA 必須在 24h 內提交
- 若違反經由 admin 後台繞過 application 層直接 SQL 執行而觸發：相關操作員立即停權 + 法律行動
```

## 7. Validation Criteria（Pre-Execution Validation Items）

本 ADR 落地需滿足：

- [ ] [SPEC-024](../../../specs/SPEC-024-redemption-entitlement.html) v0.1.0 merge（規格層）
- [ ] [SPEC-024 implementation plan](../../implementation-plans/SPEC-024-redemption-entitlement-implementation-plan.html) v0.1.0 merge（工程層）
- [ ] `crates/sdk/src/ledger/mod.rs` 之 `LedgerTransactionDraft::validate()` 含 `validate_redemption_same_user`，並有 `LedgerError::Inv7Violation` variant
- [ ] `services/wallet-ledger-service/migrations/` 新增 INV-6 (g) carve-out + INV-7 trigger migration（命名建議 `202607NNNN_inv6_redemption_carveout.sql` + `202607NNNN_inv7_redemption_same_user.sql`）
- [ ] DB trigger 通過故意違反測試（跨 user_id redemption_purchase 必 fail，SQLSTATE = `40004`）
- [ ] DB trigger 通過合法路徑測試（同 user_id redemption_purchase / fulfillment / refund / expiry 全通過）
- [ ] CI sentinel-scan grep gate 落地（檔案路徑：`.github/workflows/wallet-invariant-scan.yml`），對 `peer_transfer_` 字串之故意 PR 必 block
- [ ] 對帳腳本（每 5 分鐘 INV-2 check）擴展含 `Liability.UnredeemedEntitlement.*`；staging 1M entries 驗證 5 分鐘內完成
- [ ] SPEC-002 v0.1.13 changelog 加 cross-ref 至 SPEC-024 + 本 ADR
- [ ] ADR-0006 v1.0.3 Revision History 標註 INV-6 (g) 由本 ADR 補
- [ ] SPEC-007 §6.4 wallet-ledger latency budget 壓測：INV-7 trigger overhead 不破 2ms P99

## 8. References

### 8.1 內部文件

- [ADR-0003](./ADR-0003-double-entry-ledger.md) — 雙簿記主決策（本 ADR §4.3 為 §5 之再擴展）
- [ADR-0006](./ADR-0006-ledger-invariant-set-extension-inv6.md) — INV-6 white-list governance 基礎（本 ADR §4.1 對其 §4.1 補 (g) 行）
- [SPEC-000 §5 Q-10](../../../specs/SPEC-000-index.html#cross-cutting-q10) — 業主決議：禁會員轉點，允許購買虛擬商品 / 兌換服務
- [SPEC-002 §4.4.7 / §5](../../../specs/SPEC-002-funds-safety.html) — 禁令文字 + INV-6 表
- [SPEC-024](../../../specs/SPEC-024-redemption-entitlement.html) — 本 ADR governance 對應之規格層
- [SPEC-024 implementation plan](../../implementation-plans/SPEC-024-redemption-entitlement-implementation-plan.html) — SQL / SDK / CI 落地細節

### 8.2 外部參考

- FASB ASC 606 — Revenue from Contracts with Customers（breakage 認列指引）
- AICPA TPA 5100.92 — Definition of Breakage（未使用憑證收入認列）
- PostgreSQL Documentation: [DEFERRABLE Constraints](https://www.postgresql.org/docs/16/sql-set-constraints.html)
- "Building Reliable Distributed Systems" — Pat Helland 之 ledger invariant 模式

## 9. Notes for Codex / Future Engineers

當你在 SPEC-024 對應 task 上動手時：

1. 寫 redemption-family ledger transaction 前**永遠**先呼叫 `LedgerTransactionDraft::validate()`，特別注意 `validate_redemption_same_user` 之 user_id 配對檢查
2. 任何引入新 `txn_type` 將 `Player.Wallet.*` 與 `Liability.UnredeemedEntitlement.*` 出現在同 transaction 之 PR**必須**標 `redemption-family` label，由 redemption-service 與 wallet-ledger-service 兩個 module owner 雙 review；CI 自動加 label
3. 若你認為 INV-7 之 txn_type 列表應擴增（例如新增 `entitlement_transfer` 之路徑）：**寫一份 ADR-XXXX 標記本 ADR §4.3 之 INV-7 為 partial superseded，附上業主 + Compliance + Legal 三方簽核** — 因為這等同於開放「會員間 entitlement 轉移」，必須一併補 KYC tier 升級 / SPEC-010 AML 偵測 / Entitlement 二級市場政策三項評估
4. INV-7 trigger 之 plpgsql function 修改視同 ledger 核心模組變更，落入 SPEC-010 §9.3 MS-10 4-eye review
5. **不要**為「方便測試」把 INV-7 應用範圍縮小到 `redemption_purchase` 一個 txn_type — 完整 4 個 txn_type（purchase / fulfillment / refund / expiry）都必須 enforce，否則 refund / expiry 路徑可被用來繞過

## 10. Revision History

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-27 | v0.1 | Tech Lead | 首版 Proposed；INV-6 white-list 補 (g) `Liability.UnredeemedEntitlement` carve-out；INV-2 / INV-4 對齊含 UnredeemedEntitlement；新增 INV-7（redemption-family txn 同人綁定）。狀態 = Proposed，待業主 + Compliance Counsel 覆核後改 Accepted。 |
