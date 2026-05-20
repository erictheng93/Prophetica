# 補償審批鏈路 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 讓 admin「補償審批」(`/compensation`) 從佔位頁變為可用工作區：affiliate（及客服系統）建立補償提案 → manager / admin 單人核准或駁回 → 核准後寫一筆 `cs_compensation` 帳本交易。全程每個狀態轉換強制留稽核。

**Architecture:** 補償的 DB schema 與 `POST .../compensation-proposals` 端點已存在，但語意是「單步、提案即終結」。本計畫把後端改為**兩步流程**（`pending_approval → approved_ready_for_ledger / rejected`），新增 list / review 端點與 wallet-ledger 入帳，並在 admin app 補上 BFF、route handlers 與 UI。

**Tech Stack:** Rust 2024 + axum + reqwest + sqlx（offline）；Next.js 16 / React 19（admin app，ESM、import 用 `.js` 副檔名）；Node `node:test`。

**決策摘要（2026-05-19，使用者確認）:**

| 項目 | 決策 |
|---|---|
| 流程模型 | 兩步：提案 → 審核 |
| 提案者 | affiliate（admin app 表單）＋ 客服系統（既有 compensation API，machine-to-machine） |
| 審核者 | manager 或 admin |
| 核准人數 | **單人核准**（放棄 4-eye；提案者與審核者角色互斥，天然保留職責分離） |
| 入帳範圍 | 本期涵蓋完整鏈路，含 `TXN_TYPE_CS_COMPENSATION` 寫入 wallet-ledger |
| 稽核 | 每個狀態轉換（建立／核准／駁回／入帳）強制各寫一筆 audit event |

---

## 前置與排序

- **Phase 0（Task 1–3）是 spec PR，必須先合併**，後續實作 PR 才能引用 `SPEC-017 §6.3.1` / `SPEC-012 §11`（repo SDD 紀律）。本計畫**反轉**了 SPEC-017 §6.3.1 對 `/compensation` 的 affiliate-403、並把 SPEC-012 §11 的 4-eye 改為單人核准——這些屬規格變更，先審後做。
- Phase 1–6 為實作，依序進行；每個 Task 結束時 commit（Conventional Commits，scope = `compensation` 或對應 crate）。
- 受限於開發機算力（CLAUDE.md Compute Policy）：Rust 一律用 `-p <crate>` 標的指令，不跑 workspace 全量。SQLx 為 offline 模式，新增／改動 query 後需重新 `cargo sqlx prepare`——對 `.sqlx/` 的改動 push branch 由 CI 或強力主機驗證（CLAUDE.md）。
- `crates/proto` 已有 `TXN_TYPE_CS_COMPENSATION = 7`（標註 SPEC-012 §11），**本計畫不需改 proto、不需 `make proto`**。
- 既有 `POST .../compensation-proposals` 端點目前無任何前端消費者（admin app 無 BFF），且 `inDevelopment` 旗標已掛，重塑其語意不影響線上行為。

## 既有資產盤點（實作前先讀）

- `services/api-gateway/migrations/202605150001_create_admin_compensation.sql` — 三表已建：`admin_compensation_proposals`、`admin_compensation_proposal_approvals`（含 `ordinal CHECK IN (1,2)`、`UNIQUE(proposal_id, ordinal)` —— 為 4-eye 設計，需鬆綁）、`admin_compensation_audit_events`。
- `202605150002_admin_ops_context_and_member_actions.sql` — 加了 `idempotency_key` + 唯一索引。
- `services/api-gateway/src/admin_ops.rs` — `create_compensation_proposal` handler、`validate_compensation_proposal`、`PostgresCompensationProposalStore`、DTO（`CompensationProposalRequest/Response`、`CompensationProposal`、`CompensationProposalStatus` enum）。**目前 INSERT 寫死 `status = 'approved_ready_for_ledger'`、validation 要求恰 2 名核准人——兩者都要改。**
- `services/api-gateway/tests/admin_ops.rs` — 既有 compensation 測試，需隨語意改動更新。
- `apps/admin/lib/kyc-documents-bff.ts` + `services/api-gateway/src/internal_call_auth.rs` — signed internal-call 的參考實作，補償 BFF 比照。
- `apps/admin/lib/rbac.ts` — SPEC-017 §6.3.1 的單一事實來源。

## 檔案結構

| 檔案 | 動作 | 責任 |
|---|---|---|
| `specs/SPEC-017-monorepo-bootstrap.html` | 改 | §6.3.1 矩陣 `/compensation` 列：affiliate 改可提案、manager/admin 改可審核 |
| `specs/SPEC-012-customer-support.html` | 改 | §11：4-eye → 單人核准；補述 affiliate 為提案來源之一 |
| `docs/implementation-plans/SPEC-012-customer-support-implementation-plan.html` | 改 | §11 段落同步，指向本計畫檔 |
| `services/api-gateway/migrations/2026052000XX_compensation_single_approver.sql` | 建 | 鬆綁 4-eye 約束、`status` 加 `rejected`、approvals 表加 `decision` |
| `services/api-gateway/src/admin_ops.rs` | 改 | 重塑 create、新增 list / detail / review 端點、wallet-ledger 入帳 |
| `services/api-gateway/tests/admin_ops.rs` | 改 | 更新既有測試 + 新增 list/review/入帳/稽核測試 |
| `apps/admin/lib/compensation-bff.ts` | 建 | BFF：signed call → gateway（list / create / review） |
| `apps/admin/app/api/compensation/route.ts` | 建 | route handler：GET list / POST create |
| `apps/admin/app/api/compensation/[proposalId]/review/route.ts` | 建 | route handler：POST review（核准／駁回） |
| `apps/admin/lib/rbac.ts` | 改 | `/compensation` + `/api/compensation` allowlist、`domainCapabilityByRole` |
| `apps/admin/app/compensation/page.tsx` | 改 | 真實 UI：佇列 + 提案表單 + 審核動作（取代 inDevelopment 佔位） |
| `apps/admin/components/compensation-workspace.tsx` | 建 | 補償工作區元件（RBAC 過濾 CTA） |
| `apps/admin/lib/admin-copy.ts` | 改 | 補償文案；移除 `compensation.inDevelopment*` |
| `apps/admin/lib/ops-facade.ts` | 改 | 移除 `/compensation` 的 `inDevelopment: true` |
| `apps/admin/tests/` | 建 | BFF / route handler 純函式單元測試（`node --test`） |

---

## Phase 0 — Spec PR（先合併）

### Task 1: 修訂 SPEC-017 §6.3.1 RBAC 矩陣
- [ ] `/compensation 補償審批` 列由 `admin ✅終審 / manager ✅提案 / affiliate 403` 改為：
  - **affiliate**：`✅ 提案`（可建立補償提案；先前為 403）
  - **manager**：`✅ 審核`（可核准／駁回）
  - **admin**：`✅ 審核`（可核准／駁回）
- [ ] 補一句說明：提案者（affiliate / 客服系統）與審核者（manager / admin）角色互斥，故單人核准仍保有職責分離；同一人不得審自己的提案。
- [ ] §6.3.1 既有註記「『✅ 提案』與 admin 共用同一頁面 component，差異由 RBAC server-side 過濾 CTA」仍適用——補償工作區是單一 component。
- **驗收**：矩陣與本計畫決策摘要一致；§6.3.3 nav IA 不變（`/compensation` 維持 top-level 單項）。

### Task 2: 修訂 SPEC-012 §11 客服系統與帳本整合
- [ ] 將「需 4-eye 簽核（T3 + Compliance）」改為「需單人核准（manager 或 admin）；核准者不得為提案者」。
- [ ] 補述提案來源：客服經 compensation API 提案，affiliate 經 admin app `/compensation` 表單提案；兩者皆落為 `pending_approval`。
- [ ] `Txn type = 'cs_compensation'`、附工單 id 為 correlation —— 不變。
- **驗收**：§11 與 SPEC-017 §6.3.1 不再矛盾。

### Task 3: 同步 implementation-plan HTML 與決策表
- [ ] `SPEC-012-customer-support-implementation-plan.html` §11 段落同步單人核准用語，加一行指向本計畫檔。
- [ ] 檢查 `SPEC-000-index.html` §5 跨文件決策表是否有補償／4-eye 相關列需更新；有則一併改。
- **驗收**：spec 與 implementation-plan 用語一致。

---

## Phase 1 — DB migration

### Task 4: 補償 schema 調整 migration
- [ ] 新增 `services/api-gateway/migrations/2026052000XX_compensation_single_approver.sql`（編號取當下日期序）：
  - `admin_compensation_proposals.status` CHECK 由 `('pending_approval','approved_ready_for_ledger')` 擴為 `('pending_approval','approved_ready_for_ledger','rejected')`。
  - `admin_compensation_proposal_approvals`：`DROP CONSTRAINT` 掉 `ordinal` 的 `CHECK (ordinal IN (1,2))` 與 `UNIQUE(proposal_id, ordinal)`；`ordinal` 欄位改為 nullable（或移除——擇一，記錄於 migration 註解）。保留 `UNIQUE(proposal_id, approver_user_id)`。
  - 同表新增 `decision TEXT NOT NULL CHECK (decision IN ('approved','rejected'))` 與 `review_note TEXT`、`reviewed_at TIMESTAMPTZ NOT NULL DEFAULT now()`——此表自此記錄「審核決定」，單人核准下每提案至多一列。
- [ ] migration 歸屬 api-gateway migrator set（注意 [[core-api-postgres-migration-collision]]：core-api 共用 DB、勿與其他 migrator 衝突）。
- **驗證**：`make migrate` 於本機 dev stack（Postgres 持久模式，見 [[dev-stack-startup-postgres-mode]]）套用無誤；`make down && make dev-infra && make migrate` 乾淨重跑。
- 規模：S（1 檔）

---

## Phase 2 — api-gateway：兩步流程 + 入帳

### Task 5: 重塑 `create_compensation_proposal`（提案即 pending）
- [ ] `CompensationProposalRequest` 移除 `approvals` 欄位；保留 `ticket_id` / `reason` / `amount_minor_units`。
- [ ] `validate_compensation_proposal` 移除「恰 2 名核准人」檢查，只驗 ticket / reason 非空、amount > 0。
- [ ] `PostgresCompensationProposalStore::create_compensation_proposal` 的 INSERT 由 `'approved_ready_for_ledger'` 改為 `'pending_approval'`；不再寫 approvals 列。
- [ ] 仍寫 `compensation_proposal_created` audit event（actor = 提案者）。
- **驗收**：建立後 proposal 為 `pending_approval`、`ledger_transaction_posted = FALSE`；既有 idempotency 行為不變。
- **驗證**：`cargo nextest run -p api-gateway compensation`
- 規模：M（1 檔 + 測試）

### Task 6: 新增 list / detail 端點（補償佇列）
- [ ] 新增 `GET /api/internal/admin/compensation-proposals`（非 member-scoped，全域佇列），支援 `?status=` 過濾（pending / approved / rejected）與分頁。
- [ ] 新增 `GET /api/internal/admin/compensation-proposals/:proposal_id`（detail，含 approvals / audit 摘要）。
- [ ] `AdminOpsStore` trait + `PostgresCompensationProposalStore` 加對應 query 方法；mock store 與「production source not connected」stub 同步補實作／回錯。
- **驗收**：list 可依 status 過濾；detail 回完整提案 + 審核紀錄。
- **驗證**：`cargo nextest run -p api-gateway compensation`
- 規模：M

### Task 7: 新增 review 端點（核准／駁回）
- [ ] 新增 `POST /api/internal/admin/compensation-proposals/:proposal_id/review`，body `{ decision: "approved" | "rejected", note?: string }`。
- [ ] 守門：審核者 role ∈ {manager, admin}（由 request context / 簽章中的 operator role 判定）；只有 `pending_approval` 可被審核（狀態守門，重送回 409/冪等）；**審核者 user_id ≠ 提案者 user_id**（防自審）。
- [ ] `approved`：status → `approved_ready_for_ledger`，寫 approvals 列（`decision='approved'`），audit `compensation_proposal_approved`，接著進 Task 8 入帳。
- [ ] `rejected`：status → `rejected`，寫 approvals 列（`decision='rejected'` + `review_note`），audit `compensation_proposal_rejected`，不入帳。
- **驗收**：核准／駁回各正確轉態並留稽核；非 pending 提案不可重複審核。
- **驗證**：`cargo nextest run -p api-gateway compensation`
- 規模：M

### Task 8: 核准後寫 wallet-ledger `cs_compensation` 交易
- [ ] **先調查**：確認 wallet-ledger-service 的 POST transaction HTTP route（既有 `GET /internal/ledger/wallets/:user_id/balance` 為參考；admin_ops.rs 已有 `wallet_ledger_url` 與 `freeze_available_wallet_balance` 的呼叫範式）。
- [ ] **先調查**：確認 `cs_compensation` 雙分錄的兩個 `account_code`——玩家錢包貸方帳戶（由 `subject_user_id` 推得）與借方支出帳戶（依 SPEC-002 §4.4 會計科目表）。此為 **Open Question**，需與規格／wallet-ledger owner 對齊後填入。
- [ ] review handler 在核准成功後，呼叫 wallet-ledger `PostTransactionRequest`：
  - `type = TXN_TYPE_CS_COMPENSATION`
  - `idempotency_key` = 由 `proposal_id` 衍生（同提案重送不重複入帳，SPEC-002 §6.1）
  - `correlation_id` = 提案的 `correlation_id`（已內嵌 ticket_id，滿足 SPEC-012 §11）
  - `entries` = 平衡雙分錄（借支出 / 貸玩家錢包，Σ = 0）
- [ ] 成功：`UPDATE admin_compensation_proposals SET ledger_transaction_posted = TRUE, ledger_transaction_id = $id`，audit `compensation_ledger_transaction_posted`（metadata 含 txn_id）。
- [ ] **失敗路徑**：跨服務不可同一 DB transaction。若入帳失敗，proposal 維持 `approved_ready_for_ledger` + `ledger_transaction_posted = FALSE`，回可重試錯誤（502/503 語意）；以 `ledger_transaction_posted` 旗標 + idempotency_key 作為對帳錨點。重試機制（worker 或重試端點）見「範圍外」。
- **驗收**：核准 → wallet-ledger 出現一筆 `cs_compensation`、金額與 correlation 正確；入帳失敗時提案不卡死、可重試且不重複入帳。
- **驗證**：`cargo nextest run -p api-gateway compensation`（含 mock wallet-ledger 的入帳成功 / 失敗 / 重送案例）
- 規模：L（最高風險 Task；入帳失敗路徑與帳戶科目務必先釐清）

---

## Phase 3 — admin app：BFF + route handlers

### Task 9: `compensation-bff.ts`
- [ ] 比照 `kyc-documents-bff.ts` 建立 signed internal-call BFF：`listCompensationProposals(status?)`、`createCompensationProposal(...)`、`reviewCompensationProposal(proposalId, decision, note?)`。
- [ ] 透傳 gateway 的 HTTP 狀態碼與 error code，不重新對映（SPEC-017 §6.3.4：BFF 不得壓平錯誤）。
- [ ] 型別定義（`CompensationProposal`、`CompensationProposalStatus`、`CompensationReviewDecision` 等）放共用模組。
- **驗證**：`pnpm --filter @prophetica/admin lint`
- 規模：M

### Task 10: API route handlers
- [ ] `app/api/compensation/route.ts`：GET（list）/ POST（create）。
- [ ] `app/api/compensation/[proposalId]/review/route.ts`：POST（review）。
- [ ] 每個 handler 以 `rbac.ts` 守門：create 限 affiliate（＋視 §6.3.1 定案是否含 manager/admin）；review 限 manager/admin。
- **驗證**：`pnpm --filter @prophetica/admin lint`
- 規模：M

---

## Phase 4 — admin app：補償工作區 UI

### Task 11: `rbac.ts` 對齊 §6.3.1
- [ ] `routeRules`：`/compensation` 的 `allow` 加 `affiliate`；新增 `/api/compensation`（isPrefix）規則。
- [ ] `domainCapabilityByRole`：`affiliate` 加 `compensation-reviews: "propose"`；`manager` 的 `compensation-reviews` 由 `"propose"` 改 `"approve"`；`admin` 維持 `"approve"`。
- **驗證**：`pnpm --filter @prophetica/admin lint` + `test`（rbac 既有測試）
- 規模：S

### Task 12: 補償工作區元件 + 頁面
- [ ] 建 `components/compensation-workspace.tsx`：單一 component，依 `capabilityFor(role, "compensation-reviews")` 過濾 CTA：
  - `propose`（affiliate）：補償提案表單（subject member、ticket、reason、amount）＋ 自己已送出的提案清單與狀態。
  - `approve`（manager/admin）：`pending_approval` 佇列 + 每筆的核准／駁回動作（駁回需填 note）＋ 已處理歷史。
- [ ] `app/compensation/page.tsx`：以 `RoleGate allow={["admin","manager","affiliate"]}` 包住，渲染工作區；移除 inDevelopment 佔位橫幅。
- [ ] 後端錯誤呈現遵 SPEC-017 §6.3.4（404 / 502-503 / 5xx 分別呈現）。
- **驗收**：affiliate 看到表單、manager/admin 看到審核佇列；核准後該提案離開 pending 佇列。
- **驗證**：`pnpm --filter @prophetica/admin lint`；Playwright e2e（提案 → 核准 → 狀態更新）
- 規模：L

### Task 13: 移除 inDevelopment 標記
- [ ] `ops-facade.ts`：移除 `/compensation` 的 `inDevelopment: true`（nav「開發中」badge 隨之消失）。
- [ ] `admin-copy.ts`：移除 `compensation.inDevelopmentTitle/Body`。
- [ ] 確認 layout.tsx 的 top-level badge 機制對其他路由無副作用。
- **驗證**：`pnpm --filter @prophetica/admin lint` + `test`
- 規模：XS

---

## 橫切要求 — 稽核（每個 Task 都須遵守）

使用者要求：**所有核准記錄、操作記錄都要被記錄，系統要能 log 所有操作。**

- 每個狀態轉換在同一 DB transaction 內寫 `admin_compensation_audit_events`：`compensation_proposal_created` / `compensation_proposal_approved` / `compensation_proposal_rejected` / `compensation_ledger_transaction_posted`。
- 每筆 audit event 必含 `actor_user_id`、`actor_role`、`subject_user_id`、`ticket_id`、`correlation_id`、`metadata`（金額、decision、note、ledger_transaction_id 視情況）。
- 對齊 SPEC-018 evidence lookup：補償事件可經固定查詢名 `compensation.proposals.by_ticket` 追蹤（見 `ops-facade.ts`）。
- **驗收測試**：建立 / 核准 / 駁回 / 入帳各情境，斷言對應 audit row 恰寫一筆且欄位完整。

---

## 驗收條件（完成定義）

- [ ] Phase 0 spec PR 已合併；SPEC-017 §6.3.1 與 SPEC-012 §11 不再矛盾。
- [ ] affiliate 可在 `/compensation` 建立提案，提案落為 `pending_approval`。
- [ ] manager / admin 可在 `/compensation` 佇列核准或駁回；非 pending 提案不可重複審核；不可審自己的提案。
- [ ] 核准後 wallet-ledger 出現一筆 `TXN_TYPE_CS_COMPENSATION`，金額正確、`correlation_id` 含 ticket id、`idempotency_key` 防重複入帳。
- [ ] 建立／核准／駁回／入帳四類操作各留一筆稽核 event，欄位完整。
- [ ] 入帳失敗時提案停在 `approved_ready_for_ledger`/未入帳、可重試、不重複入帳。
- [ ] `/compensation` 的 inDevelopment 標記與「開發中」badge 已移除。
- [ ] `cargo nextest run -p api-gateway compensation` 全綠；`pnpm --filter @prophetica/admin lint` + `test` 全綠；Playwright e2e 提案→核准流程通過。
- [ ] SQLx offline metadata（`.sqlx/`）已重新產生並提交（由 CI / 強力主機驗證，記錄主機）。

## Open Questions（實作前須釐清）

1. **入帳會計科目**（Task 8）：`cs_compensation` 雙分錄的借方支出帳戶 `account_code` —— 依 SPEC-002 §4.4 會計科目表決定，需與 wallet-ledger owner 對齊。
2. **manager / admin 是否也可建立提案**：決策摘要的提案者為 affiliate / 客服；Phase 0 §6.3.1 PR 若認為營運上 manager/admin 也需能建提案，於該 PR 一併定案（届時防自審守門更顯重要）。
3. **入帳失敗的重試機制**：本計畫採同步入帳 + 旗標對帳；是否需獨立 reconcile worker，視營運頻率決定（見範圍外）。

## 範圍外（後續任務）

- 客服系統（Zendesk / Freshdesk）→ compensation API 的 webhook 整合接線（SPEC-012 領域，外部系統）。
- 入帳失敗的自動 reconcile worker / 重試端點。
- 多幣別補償（`PostEntry.currency`，SPEC-016 Phase 2）。
- 補償提案的 SLA / 時效告警。
