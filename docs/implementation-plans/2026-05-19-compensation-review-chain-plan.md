# 補償審批鏈路 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 讓 admin「補償審批」(`/compensation`) 從佔位頁變為可用工作區：CS / manager / admin 建立補償提案（affiliate v1 經 scoped API、UI 延至 v1.1）→ manager 或 admin 單人核准（提案者 ≠ 審核者）→ 核准後寫一筆 `cs_compensation` 帳本交易，全程以 outbox-ready 狀態機保證可重入、可對帳。每個狀態轉換強制留稽核。

**Architecture:** 補償的 DB schema 與 `POST .../compensation-proposals` 端點已存在，但語意是「單步、提案即終結」。本計畫把後端改為**兩步流程 + 入帳子狀態機**（`proposed → approved → posting → posted` / `posting_failed` / `posting_unknown` / `rejected`；`cancelled` reserved），新增 list / review / manual-retry 端點、affiliate referral-ownership 守門（呼叫 referral-service 新內部端點）、wallet-ledger 入帳（使用 SPEC-002 §4.4.6 新增之 `Expense.CustomerSupportCompensation` 科目），並在 admin app 補上 BFF、route handlers 與工作區 UI（admin/manager only）。

**Tech Stack:** Rust 2024 + axum + reqwest + sqlx（offline）；Next.js 16 / React 19（admin app，ESM、import 用 `.js` 副檔名）；Node `node:test`。

---

## ⚠️ V1 範圍邊界（請先讀，避免誤解）

| 主題 | v1 狀態 | 備註 |
|---|---|---|
| **/compensation 工作區** | 限 admin / manager | affiliate 看不到此頁；提案、審核、卡住佇列皆在此 |
| **CS 系統提案** | API 接通 | gateway `POST .../compensation-proposals` 接 signed CS service principal |
| **manager / admin 自建提案** | UI 接通 | 在 /compensation 工作區內建 |
| **affiliate 提案 backend** | 接通（含 scope 守門 + 金額 cap） | gateway 接受 `proposer_kind=affiliate` 的 signed call |
| **affiliate 提案 UI（/me/affiliate 子頁）** | **延至 v1.1 獨立 plan** | v1 affiliate 路徑為「scoped API only」 |
| **referral-service `/scope` 端點** | v1 上線 | 即使 v1 無 affiliate UI，仍需上線以驗 backend 守門邏輯；v1.1 直接接 UI |
| **`cancelled` 狀態** | schema 預留、無轉移、UI 不顯示、audit event 不發 | 看到 DB enum 有 `cancelled` 不是漏做——是 reserved。獨立 plan 補 |
| **Reversal（沖回）** | 不在範圍 | 需 proto 新增 enum + ADR-0006 追認 INV-6 carve-out；獨立 plan |
| **Reconcile worker** | 設計鎖、實作 deferred | v1 提供手動 retry 端點 + ops 卡住佇列可見 |

---

## 規格基線（contract 層，已落地於工作樹）

本計畫**不含**任何 spec 變更——規格已於前置 commit 完善並引述如下。實作層次決定（狀態機、reconcile、idempotency、入帳分錄細節、cancel 預留語意）集中於本檔，spec 只規定 contract。

| 規格段落 | 版本 | 規範內容 |
|---|---|---|
| [SPEC-017 §6.3.1](../../specs/SPEC-017-monorepo-bootstrap.html) | v0.2.10 | `/compensation` RBAC 矩陣：admin / manager 皆「提案/審核」，affiliate `403`（不進此頁） |
| [SPEC-017 §6.3.1.1](../../specs/SPEC-017-monorepo-bootstrap.html) | v0.2.10 新增 | **補償 domain 工作流契約**：兩步流程、proposer_kind 分類、單人核准 + proposer≠approver、affiliate 三道守門（金額上限 / subject-scope check / referral-service fail-closed）、不存在 member 回應分流（admin/manager → 404；affiliate → 403 防枚舉）、稽核必含 `proposer_kind` |
| [SPEC-017 §6.3.4](../../specs/SPEC-017-monorepo-bootstrap.html) | （既有） | Admin app 錯誤呈現規則：BFF 透傳 gateway HTTP 狀態 + error code，不重新對映 |
| [SPEC-012 §11](../../specs/SPEC-012-customer-support.html) | v0.1.2 | 客服系統與帳本整合：單人核准（取代原 4-eye）、proposer_kind 稽核、`Txn type = 'cs_compensation'` 附 ticket id |
| [SPEC-002 §4.2 / §4.4.6](../../specs/SPEC-002-funds-safety.html) | v0.1.10 | 新增 `Expense.CustomerSupportCompensation` 科目；`cs_compensation` 分錄為 `DR Expense.CustomerSupportCompensation / CR Player.Wallet`；v1 grant-only 不觸發 INV-6 |
| [SPEC-018](../../specs/SPEC-018-auditability-evidence-pack.html) | （既有） | Evidence lookup 固定查詢名 `compensation.proposals.by_ticket` |
| [SPEC-011](../../specs/SPEC-011-affiliate-commission.html) | （既有） | Flat-list 推薦模型；referral ownership 由 referral-service 權威 |

**決策摘要（2026-05-19 / 2026-05-23）:**

| 項目 | 決策 |
|---|---|
| 流程模型 | 兩步：提案 → 審核；入帳為獨立子狀態機 |
| Proposer 來源 | CS 系統（API） / manager / admin（/compensation UI） / affiliate（v1 backend-only、v1.1 加 UI） |
| Approver | manager 或 admin **單人核准**；**提案者 ≠ 審核者**（handler 守門 + 測試覆蓋） |
| Affiliate 守門 | (a) 金額上限 env；(b) subject-scope check（呼叫 referral-service）；(c) `proposer_kind=affiliate` 稽核標記 |
| Scope check 來源 | **referral-service 內部 API**；5xx → gateway **fail closed** 回 502 |
| Scope v1 範圍 | 僅認 `direct_referral`（對齊 SPEC-011 flat-list） |
| 不存在 member 回應 | admin/manager → 404；affiliate → 403（同 out-of-scope，防枚舉） |
| 帳本科目 | `Expense.CustomerSupportCompensation`（SPEC-002 §4.2，與 `ManualPointsGrant` 分離） |
| 入帳狀態機 | 8 態（`proposed` / `approved` / `posting` / `posted` / `posting_failed` / `posting_unknown` / `rejected` / `cancelled`-reserved） |
| `posting_failed` vs `posting_unknown` | 4xx → `posting_failed`；timeout / 5xx / network → `posting_unknown` |
| 入帳冪等不變式 | **任何**路徑都**永遠先**用既有 `ledger_idempotency_key` query ledger，**絕不**生新 key |
| Reconcile worker | 設計鎖死、實作 deferred 至 v1.1；v1 提供手動 retry 端點 |
| 稽核 | 每個 v1 狀態轉換強制各寫一筆 audit event；`compensation_proposal_cancelled` reserved 不發 |

---

## 前置與排序

- 規格基線已於前置 commit 落地，本計畫直接進實作；**沒有** spec PR 階段。
- Phase 1–4 為實作，依序進行；每個 Task 結束時 commit（Conventional Commits，scope = `compensation` / `referral` / 對應 crate）。
- 受限於開發機算力（CLAUDE.md Compute Policy）：Rust 一律用 `-p <crate>` 標的指令，不跑 workspace 全量。SQLx 為 offline 模式，新增／改動 query 後需重新 `cargo sqlx prepare`——對 `.sqlx/` 的改動 push branch 由 CI 或強力主機驗證。
- `crates/proto` 已有 `TXN_TYPE_CS_COMPENSATION = 7`，**v1 不需改 proto、不需 `make proto`**。
- 既有 `POST .../compensation-proposals` 端點目前無任何前端消費者，且 `inDevelopment` 旗標已掛，重塑其語意不影響線上行為。

## 既有資產盤點（實作前先讀）

- `services/api-gateway/migrations/202605150001_create_admin_compensation.sql` — 三表已建（為 4-eye 設計，需鬆綁）。
- `202605150002_admin_ops_context_and_member_actions.sql` — 加了 `idempotency_key`（請求側冪等；ledger 側另用 `ledger_idempotency_key`）。
- `services/api-gateway/src/admin_ops.rs` — `create_compensation_proposal` handler 等。**目前 INSERT 寫死 `status = 'approved_ready_for_ledger'`、validation 要求恰 2 名核准人——兩者都要改。**
- `services/api-gateway/tests/admin_ops.rs` — 既有 compensation 測試。
- `apps/admin/lib/kyc-documents-bff.ts` + `services/api-gateway/src/internal_call_auth.rs` — signed internal-call 參考實作。
- `apps/admin/lib/rbac.ts` — SPEC-017 §6.3.1 的單一事實來源。
- `packages/proto/v1/ledger.proto` — `TXN_TYPE_CS_COMPENSATION = 7`、`PostTransactionRequest` 已備。
- `services/referral-service/` — referral 領域權威；`affiliate_profiles.affiliate_id → user_id`、`referrals.referrer_id → referred_id`。

## 檔案結構

| 檔案 | 動作 | 責任 |
|---|---|---|
| `services/referral-service/src/api.rs` | 改 | 新增 `GET /internal/referrals/affiliates/{aff_id}/subjects/{subj_id}/scope` |
| `services/referral-service/src/referrals.rs`（或對應） | 改 | scope 判定 query（v1：direct_referral only） |
| `services/api-gateway/migrations/202605230001_compensation_state_machine.sql` | 建 | 鬆綁 4-eye 約束、status 擴為 8 態、補 outbox 欄位、加 `proposer_kind` |
| `services/api-gateway/src/admin_ops.rs` | 改 | 重塑 create、新增 list / detail / review / retry-posting 端點、wallet-ledger 入帳、affiliate 守門 |
| `services/api-gateway/src/referral_client.rs` | 建 | gateway→referral-service 的 signed internal-call client |
| `services/api-gateway/tests/admin_ops.rs` | 改 | 更新既有測試 + 新增 list/review/入帳/失敗重入/稽核/affiliate 守門/member 不存在分流測試 |
| `apps/admin/lib/compensation-bff.ts` | 建 | BFF：signed call → gateway（list / create / review / retry） |
| `apps/admin/app/api/compensation/route.ts` | 建 | route handler：GET list / POST create（限 admin/manager） |
| `apps/admin/app/api/compensation/[proposalId]/review/route.ts` | 建 | route handler：POST review |
| `apps/admin/app/api/compensation/[proposalId]/retry-posting/route.ts` | 建 | route handler：POST manual retry（限 admin） |
| `apps/admin/lib/rbac.ts` | 改 | `/api/compensation` allowlist（admin/manager）；**`/compensation` 不加 affiliate** |
| `apps/admin/app/compensation/page.tsx` | 改 | 真實 UI；移除 inDevelopment 佔位 |
| `apps/admin/components/compensation-workspace.tsx` | 建 | 提案表單（manager/admin）+ 待審佇列 + 卡住佇列（三態視覺區分）+ 歷史 |
| `apps/admin/lib/admin-copy.ts` | 改 | 補償文案；移除 `compensation.inDevelopment*` |
| `apps/admin/lib/ops-facade.ts` | 改 | 移除 `/compensation` 的 `inDevelopment: true` |
| `apps/admin/tests/` | 建 | BFF / route handler 純函式單元測試（`node --test`） |

---

## Phase 1 — DB migration

### Task 1: 補償狀態機 + outbox 欄位 migration
- [ ] 新增 `services/api-gateway/migrations/202605230001_compensation_state_machine.sql`：

  **a) `admin_compensation_proposals` 改動**
  - 既有資料先 `UPDATE`：`pending_approval` → `proposed`、`approved_ready_for_ledger` → `approved`。
  - `status` CHECK 由 `('pending_approval','approved_ready_for_ledger')` 擴為 `('proposed','approved','posting','posted','posting_failed','posting_unknown','rejected','cancelled')`——**8 態**。
  - **migration 註解明確寫**：`cancelled` v1 為 reserved，無任何 v1 路徑可進入；保留給未來「提案者撤回」獨立 plan。讀者見此 enum 不必懷疑是漏做。
  - 新增欄位：
    - `proposer_kind TEXT NOT NULL CHECK (proposer_kind IN ('affiliate','manager','admin','cs_system'))`
    - `ledger_idempotency_key TEXT UNIQUE`（nullable；於進入 `posting` 前生成；建議 `cs-cmp-{proposal_id}`，呼應 SPEC-002 §4.4.6）
    - `posting_attempt_count INTEGER NOT NULL DEFAULT 0`
    - `last_posting_error TEXT`（含分類 tag：`ledger_4xx` / `ledger_timeout` / `ledger_5xx` / `network`）
    - `approved_at TIMESTAMPTZ`
    - `posted_at TIMESTAMPTZ`
    - `cancelled_at TIMESTAMPTZ`（v1 永遠 NULL）
  - `ledger_transaction_posted` / `ledger_transaction_id` 保留（後者為 `posted` 狀態的事實欄位，與 SPEC-002 §4.4.6 之雙向綁定要求對齊）。
  - 索引：`(status, created_at DESC)`（佇列）、`(status, approved_at DESC) WHERE status IN ('posting','posting_failed','posting_unknown')`（卡住佇列）。

  **b) `admin_compensation_proposal_approvals` 改動**
  - `DROP CHECK (ordinal IN (1,2))`、`DROP UNIQUE (proposal_id, ordinal)`；`ordinal` 改 nullable。
  - 新增 `decision TEXT NOT NULL CHECK (decision IN ('approved','rejected'))`、`review_note TEXT`、`reviewed_at TIMESTAMPTZ NOT NULL DEFAULT now()`。
  - 保留 `UNIQUE(proposal_id, approver_user_id)`。
  - 註解：proposer≠approver 由 application-layer 守門（跨表 CHECK 不可行，handler + 測試保證）。對應 SPEC-017 §6.3.1.1 規定。

- [ ] migration 歸屬 api-gateway migrator set（[[core-api-postgres-migration-collision]] 注意）。
- **驗證**：`make migrate` 套用無誤；`make down && make dev-infra && make migrate` 乾淨重跑；既有測試資料正確轉態。
- 規模：M

---

## Phase 2 — referral-service + api-gateway：兩步流程 + 入帳 + scope 守門

### Task 2: referral-service 新增 subject-scope 端點
- [ ] 新增端點（規範來源：SPEC-017 §6.3.1.1）：
  ```
  GET /internal/referrals/affiliates/{affiliate_id}/subjects/{subject_user_id}/scope
  ```
  回傳：
  ```json
  {
    "affiliate_id": "AFF-001",
    "affiliate_user_id": "user_aff_001",
    "subject_user_id": "user_123",
    "in_scope": true,
    "relationship": "direct_referral"
  }
  ```
- [ ] v1 SQL（僅認 direct referral，對齊 SPEC-011 flat-list 推薦模型；**不**預留多層語意避免 MLM 紅線）：
  ```sql
  SELECT 1
  FROM affiliate_profiles ap
  JOIN referrals r
    ON r.referrer_id = ap.user_id
  WHERE ap.affiliate_id = $1
    AND r.referred_id = $2
  LIMIT 1;
  ```
  有列 → `in_scope=true, relationship="direct_referral"`；無列 → `in_scope=false, relationship=null`（仍回 200）。
- [ ] **內部 auth 守門**：
  - 走 signed internal call（同 `internal_call_auth.rs` 模式）；簽章不對 → 401
  - 只允許 gateway / trusted internal caller，**非該名單者一律拒絕**（即使簽章對也拒絕未列入白名單的 service principal）
  - **每次呼叫寫 referral-service 內部 audit log**（actor service principal、affiliate_id、subject_user_id、correlation_id、in_scope 結果）；audit 為內部觀測用、不影響 response
- [ ] **防 enumeration**（SPEC-017 §6.3.1.1 規定）：affiliate 不存在 / subject 不存在皆視為 `in_scope=false`（response 三種情況不可區分）。
- **驗收**：
  - direct_referral 關係 → `in_scope=true`
  - 無關係 / affiliate 不存在 / subject 不存在 → `in_scope=false`（三者 response 不可區分）
  - 簽章不對 → 401；簽章對但 caller 不在白名單 → 401/403（擇一一致）
  - 每次呼叫產生一筆內部 audit log
- **驗證**：`cargo nextest run -p referral-service scope`
- 規模：M

### Task 3: 重塑 `create_compensation_proposal`（提案即 `proposed` + 多層守門）
- [ ] `CompensationProposalRequest` 移除 `approvals` 欄位；保留 `ticket_id` / `reason` / `amount_minor_units`。
- [ ] `validate_compensation_proposal` 移除「恰 2 名核准人」檢查，只驗 ticket / reason 非空、amount > 0。
- [ ] 由 request context（signed internal call 帶入的 actor_role）決定 `proposer_kind`（SPEC-017 §6.3.1.1 規定的四種類別）。
- [ ] **守門順序（依此順序執行，每步通過才到下一步；對齊 SPEC-017 §6.3.1.1 分流規則）：**
  1. **Member 存在性檢查**（所有 proposer_kind 皆執行）：查 user-service / member view 確認 `subject_user_id` 存在。
     - 不存在 + `proposer_kind ∈ {admin, manager, cs_system}` → 404 `ERROR_CODE_MEMBER_NOT_FOUND`（語意明確，配合 SPEC-017 §6.3.4）
     - 不存在 + `proposer_kind = affiliate` → **403 `ERROR_CODE_COMPENSATION_SUBJECT_OUT_OF_SCOPE`**（與 out-of-scope 同碼，防 affiliate 把補償提案端點當成 member 枚舉工具）
  2. **Affiliate 守門**（僅 `proposer_kind = affiliate` 執行）：
     - amount 超過 env `AFFILIATE_COMPENSATION_MAX_MINOR_UNITS` → 422 `ERROR_CODE_COMPENSATION_AMOUNT_OVER_LIMIT`
     - 呼叫 referral-service `/scope` 端點：
       - `in_scope=false` → 403 `ERROR_CODE_COMPENSATION_SUBJECT_OUT_OF_SCOPE`
       - referral-service 不可達 / 5xx → **fail closed**，回 502 `ERROR_CODE_REFERRAL_SCOPE_UNAVAILABLE`
- [ ] INSERT：status → `proposed`、寫 `proposer_kind`、不寫 approvals 列。
- [ ] audit `compensation_proposal_created`（metadata 含 `proposer_kind`、amount、ticket_id；affiliate 案另含 referral-service 回傳的 `relationship`）。
- **驗收**：
  - 提案落為 `proposed`
  - admin/manager + 不存在 member → 404；affiliate + 不存在 member → 403（與 out-of-scope 同碼）
  - affiliate 超 cap → 422；out-of-scope → 403；referral-service 5xx → 502
  - 既有 idempotency 行為（請求側 `idempotency_key`）不變
- **驗證**：`cargo nextest run -p api-gateway compensation`（含 mock referral-service 與 mock member 查詢的所有分支）
- 規模：M

### Task 4: 新增 list / detail 端點
- [ ] `GET /api/internal/admin/compensation-proposals`（全域），支援 `?status=`（含 8 態 + `all_active`）、`?proposer_kind=`、分頁。
- [ ] `GET /api/internal/admin/compensation-proposals/:proposal_id`（detail，含 approvals / 最近 audit 摘要 / `posting_attempt_count` / `last_posting_error`）。
- [ ] `AdminOpsStore` trait + Postgres / mock 同步補實作。
- **驗收**：list 可依 status 與 proposer_kind 過濾；detail 完整；卡住佇列查詢用上新索引。
- **驗證**：`cargo nextest run -p api-gateway compensation`
- 規模：M

### Task 5: 新增 review 端點（核准／駁回）
- [ ] `POST /api/internal/admin/compensation-proposals/:proposal_id/review`，body `{ decision: "approved" | "rejected", note?: string }`。
- [ ] 守門（對齊 SPEC-017 §6.3.1.1 工作流契約）：
  - 審核者 role ∈ {manager, admin}
  - **審核者 user_id ≠ 提案者 user_id** → 403 `ERROR_CODE_COMPENSATION_SELF_REVIEW`
  - 只有 `proposed` 可被審核；其他狀態 → 409 `ERROR_CODE_COMPENSATION_NOT_PENDING`（冪等：重送已 reviewed 案回原結果，附 `replayed=true`）
- [ ] `approved`：UPDATE status → `approved`、`approved_at = now()`；寫 approvals 列；audit `compensation_proposal_approved`；接著進 Task 6 入帳。
- [ ] `rejected`：UPDATE status → `rejected`；寫 approvals 列（`decision='rejected'` + `review_note`）；audit `compensation_proposal_rejected`。
- **驗收**：核准 / 駁回各正確轉態並留稽核；自審被擋；重複審核冪等。
- **驗證**：`cargo nextest run -p api-gateway compensation`
- 規模：M

### Task 6: 核准後寫 wallet-ledger（outbox-ready，區分 failed / unknown）
- [ ] **先調查**（Task 內第一步）：確認 wallet-ledger-service POST transaction HTTP route（admin_ops.rs 已有 `wallet_ledger_url` 範式）；玩家錢包貸方 `account_code` 編碼規則。借方使用 `Expense.CustomerSupportCompensation`（規範來源：SPEC-002 §4.4.6）。
- [ ] 在 review handler 核准成功後（同一請求生命週期內）：
  1. 生成 `ledger_idempotency_key = "cs-cmp-{proposal_id}"`（SPEC-002 §4.4.6 規定格式）；UPDATE：status → `posting`、`ledger_idempotency_key = $key`、`posting_attempt_count += 1`。audit `compensation_posting_attempt_started`。
  2. 呼叫 wallet-ledger `PostTransactionRequest`：
     - `type = TXN_TYPE_CS_COMPENSATION`
     - `idempotency_key = $key`
     - `correlation_id` = 提案的 `correlation_id`（已內嵌 ticket_id，呼應 SPEC-012 §11）
     - `entries = [DR Expense.CustomerSupportCompensation +Y, CR Player.Wallet.{user_id} -Y]`（規範來源：SPEC-002 §4.4.6）
  3. **2xx**：UPDATE → `posted`、`posted_at = now()`、`ledger_transaction_id = $txn_id`、`last_posting_error = NULL`。audit `compensation_ledger_transaction_posted`。同時滿足 SPEC-002 §4.4.6 之「`ledger_transaction_id` 雙向綁定」要求。
  4. **4xx**（ledger 明確拒絕）：UPDATE → `posting_failed`、`last_posting_error = "ledger_4xx: {msg}"`。audit `compensation_posting_attempt_failed`（`error_class="ledger_4xx"`）。
  5. **timeout / 5xx / network**：UPDATE → `posting_unknown`、`last_posting_error = "ledger_timeout|5xx|network: {msg}"`。audit `compensation_posting_attempt_failed`（對應 `error_class`）。
  6. 不論 4/5：核准已成立；回 client `202 Accepted` 附當前狀態，**不**回 5xx（否則 client 重試會觸發 409）。
- [ ] **冪等不變式（plan-level invariant；SPEC-002 §4.4.6 已載明）**：
  - 任何路徑（同步、worker、手動 retry）對同一 proposal 的入帳嘗試，**必須**使用既有 `ledger_idempotency_key`
  - **絕不**為同一 proposal 生成第二個 `ledger_idempotency_key`
  - `posting_unknown` 的 retry 風險高於 `posting_failed`（前者 ledger 可能已 commit，只是 gateway 沒收到結果），但**演算法一致**：永遠先用既有 key query ledger
- [ ] **mock**：mock wallet-ledger client 可注入 success / 4xx / 5xx / timeout 四種行為。
- **驗收**：
  - 核准 → wallet-ledger 出現一筆 `cs_compensation`，分錄與金額正確、`correlation_id` 含 ticket id、`idempotency_key` 防重複入帳
  - ledger 4xx → `posting_failed`；ledger timeout/5xx/network → `posting_unknown`
  - **`posting_unknown` 案件當 ledger 實際已 commit 時，重試（同 key）會被 ledger 偵測為 replay 並回原 txn_id**（不雙重入帳）—— 此為硬性驗收測試案例
  - 重複核准走 409 冪等
- **驗證**：`cargo nextest run -p api-gateway compensation`（含 4 種 mock 行為 + `posting_unknown` 已 commit 的 replay 案例）
- 規模：L

### Task 7: Reconcile 設計鎖定 + 手動 retry 端點
- [ ] 在 `services/api-gateway/src/admin_ops.rs` 或專屬模組註解內鎖死 reconcile 演算法（v1.1 worker 與 v1 手動 retry 端點共用此演算法）：
  ```
  for proposal in SELECT * FROM admin_compensation_proposals
                  WHERE status IN ('posting','posting_failed','posting_unknown')
                    AND approved_at < now() - interval '5 minutes':
    1. query wallet-ledger by ledger_idempotency_key  (NEVER skip this)
    2a. ledger 已存在該 txn:
        UPDATE proposal SET status='posted', posted_at=ledger.created_at,
                            ledger_transaction_id=ledger.txn_id;
        audit compensation_reconcile_marked_posted
    2b. ledger 不存在:
        以同一 ledger_idempotency_key 重送 PostTransactionRequest;
        依 Task 6 step 3/4/5 處理結果（posting_attempt_count += 1）。
        絕不生新 key。
  ```
  `posting_failed` 與 `posting_unknown` 走同一演算法；差別只在 UI 與監控警示分流。
- [ ] **v1 即提供手動 retry 端點**：`POST /api/internal/admin/compensation-proposals/:proposal_id/retry-posting`（限 admin），執行同一演算法。audit `compensation_posting_manual_retry`。
  - **明列守門**：retry 端點**讀**既有 `ledger_idempotency_key`；若 NULL（理論上不該發生於非 `proposed`/`approved` 狀態）→ 500 內部錯誤並 audit。**絕不**為 retry 生新 key。
- [ ] Worker 實作 deferred 至 v1.1 PR；本 plan 範圍外段註明。
- **驗收**：
  - 演算法以註解 / docstring 形式固化於程式碼
  - 手動 retry 對 `posting`/`posting_failed`/`posting_unknown` 皆可用
  - retry 對已 posted 案冪等（直接走 2a 標 posted，不重送）
  - retry 對 `posting_unknown` 且 ledger 實際已 commit 的案：走 2a，不雙重入帳
- **驗證**：`cargo nextest run -p api-gateway compensation_retry`
- 規模：S

---

## Phase 3 — admin app：BFF + route handlers（admin/manager 路徑）

### Task 8: `compensation-bff.ts`
- [ ] 比照 `kyc-documents-bff.ts` 建立 signed internal-call BFF：`listCompensationProposals({status?, proposerKind?, page?})`、`createCompensationProposal(...)`、`reviewCompensationProposal(proposalId, decision, note?)`、`retryPostingForProposal(proposalId)`。
- [ ] 透傳 gateway 的 HTTP 狀態碼與 error code，不重新對映（SPEC-017 §6.3.4）。
- [ ] 型別定義（`CompensationProposal`、`CompensationProposalStatus`（v1 顯示 7 態，`cancelled` 隱藏）、`CompensationProposerKind`、`CompensationReviewDecision`）放共用模組。
- **驗證**：`pnpm --filter @prophetica/admin lint`
- 規模：M

### Task 9: API route handlers
- [ ] `app/api/compensation/route.ts`：GET（list）/ POST（create）；**RBAC：限 admin / manager**（affiliate 不從這條路徑進入；SPEC-017 §6.3.1 矩陣）。
- [ ] `app/api/compensation/[proposalId]/review/route.ts`：POST review；限 admin / manager。
- [ ] `app/api/compensation/[proposalId]/retry-posting/route.ts`：POST manual retry；限 admin。
- **驗證**：`pnpm --filter @prophetica/admin lint`
- 規模：M

---

## Phase 4 — admin app：補償工作區 UI（admin / manager only）

### Task 10: `rbac.ts` 對齊 SPEC-017 §6.3.1
- [ ] `routeRules`：`/compensation` 的 `allow` **維持 `["admin","manager"]`，不加 affiliate**。
- [ ] `routeRules`：新增 `/api/compensation`（isPrefix）規則 `allow: ["admin","manager"]`。
- [ ] `domainCapabilityByRole`：`manager` 的 `compensation-reviews` 由 `"propose"` 改 `"approve"`（manager v1 起也可審核，配合 SPEC-017 §6.3.1 矩陣調整）；`admin` 維持 `"approve"`；**affiliate 不加 `compensation-reviews`**（v1 無 admin-app 路徑）。
- **驗證**：`pnpm --filter @prophetica/admin lint` + `test`
- 規模：S

### Task 11: 補償工作區元件 + 頁面（admin / manager）
- [ ] 建 `components/compensation-workspace.tsx`，依 viewer user_id 過濾 CTA：
  - **提案區**（admin / manager）：補償提案表單（subject_user_id、ticket、reason、amount）；submit 後該案進入「待審佇列」。
  - **待審佇列**（admin / manager）：`proposed` 列表；對每筆顯示 approve / reject 按鈕；**若該案 proposer = viewer，按鈕 disabled 並標示「不可審自己的提案」**（前端 UX 提示；後端為真正守門，SPEC-017 §6.3.1.1 規定）。
  - **卡住佇列**（admin / manager，唯讀；admin 多一個 retry 按鈕）：合併 `posting` / `posting_failed` / `posting_unknown`，**視覺區分**：
    - `posting`：中性 / 進行中徽章
    - `posting_failed`：警示徽章「Ledger 拒絕」
    - `posting_unknown`：注意徽章「結果待確認（可能已入帳）」
    - 各顯示 `posting_attempt_count`、`last_posting_error`、`approved_at`
    - admin 按 retry → 呼叫 `/api/compensation/:id/retry-posting`；後端走 reconcile 演算法
  - **歷史區**：`posted` / `rejected` 唯讀清單。**v1 不顯示 `cancelled`**（無資料；UI 也不為 cancelled 預留欄位，避免讀者誤以為功能未做）。
- [ ] `app/compensation/page.tsx`：以 `RoleGate allow={["admin","manager"]}` 包住，渲染工作區；移除 inDevelopment 佔位橫幅。
- [ ] 後端錯誤呈現遵 SPEC-017 §6.3.4（404 / 422 / 502-503 / 5xx 分別呈現）。
- **驗收**：
  - admin / manager 看到三區工作流；affiliate 進入 → 403（既有 RBAC）
  - 自己提的案不能審；admin 可手動重試入帳
  - 卡住佇列三態視覺區分清楚
- **驗證**：`pnpm --filter @prophetica/admin lint`；Playwright e2e 三條主路徑：
  1. manager 提案 → admin 核准 → posted
  2. manager 提案 → manager 點自己案的 approve（被擋；後端 403）
  3. manager 提案 → 模擬 ledger timeout → posting_unknown → admin retry → posted
- 規模：L

### Task 12: 移除 inDevelopment 標記
- [ ] `ops-facade.ts`：移除 `/compensation` 的 `inDevelopment: true`。
- [ ] `admin-copy.ts`：移除 `compensation.inDevelopmentTitle/Body`。
- [ ] 確認 layout.tsx 的 top-level badge 機制對其他路由無副作用。
- **驗證**：`pnpm --filter @prophetica/admin lint` + `test`
- 規模：XS

---

## 橫切要求 — 稽核（每個 Task 都須遵守）

使用者要求：**所有核准記錄、操作記錄都要被記錄，系統要能 log 所有操作。**對齊 SPEC-012 §11 與 SPEC-017 §6.3.1.1 之稽核必含規定。

- 每個狀態轉換在同一 DB transaction 內寫 `admin_compensation_audit_events`，v1 事件類型：
  - `compensation_proposal_created`
  - `compensation_proposal_approved`
  - `compensation_proposal_rejected`
  - `compensation_posting_attempt_started`
  - `compensation_ledger_transaction_posted`
  - `compensation_posting_attempt_failed`（metadata 含 `error_class`：`ledger_4xx` / `ledger_timeout` / `ledger_5xx` / `network`）
  - `compensation_posting_manual_retry`
  - `compensation_reconcile_marked_posted`（v1.1 worker 觸發；v1 手動 retry 走 reconcile 演算法時若命中 ledger 已存在亦發此 event）
- **Reserved（v1 不發）**：`compensation_proposal_cancelled` —— 預留給未來「提案者撤回」獨立 plan。**enum / DB CHECK 預留，code 不發此 event。**
- 每筆 audit event 必含 `actor_user_id`、`actor_role`、`subject_user_id`、`ticket_id`、`correlation_id`、`metadata`（金額、`proposer_kind`、decision、note、ledger_idempotency_key、ledger_transaction_id、attempt #、error_class 視情況；affiliate 案另含 `relationship`）。
- referral-service `/scope` 端點每次呼叫**亦**寫一筆內部 audit log（service-level，與 compensation audit 分離；用以排錯）。
- 對齊 SPEC-018 evidence lookup：補償事件可經固定查詢名 `compensation.proposals.by_ticket` 追蹤。
- **驗收測試**：建立 / 核准 / 駁回 / posting 起 / posting 成 / posting 4xx 敗 / posting 結果不明 / 手動 retry / reconcile 命中 各情境，斷言對應 audit row 恰寫一筆且欄位完整。

---

## 驗收條件（v1 完成定義）

- [ ] referral-service `/scope` 端點上線，direct_referral 判定正確；簽章 + 白名單守門有效；每次呼叫產生內部 audit。
- [ ] CS / manager / admin 可建立提案，落為 `proposed`；affiliate backend 路徑 ready（gateway 接受 signed call、scope 守門完整），但 admin-app UI v1 不開放。
- [ ] member 不存在分流正確：admin/manager → 404；affiliate → 403（同 out-of-scope）。
- [ ] manager / admin 可在 `/compensation` 佇列核准或駁回；非 `proposed` 不可重複審核；不可審自己的提案。
- [ ] 核准後 wallet-ledger 出現一筆 `TXN_TYPE_CS_COMPENSATION`，分錄為 `DR Expense.CustomerSupportCompensation / CR Player.Wallet.{user_id}`（符合 SPEC-002 §4.4.6），`correlation_id` 含 ticket id，`ledger_idempotency_key` 防重複入帳。
- [ ] Ledger 4xx → `posting_failed`；ledger timeout / 5xx → `posting_unknown`；UI 卡住佇列三態視覺區分清楚；admin 手動 retry 走 reconcile 演算法（先 query ledger by idempotency_key）。
- [ ] `posting_unknown` 案件當 ledger 實際已 commit，retry 不雙重入帳（硬性驗收測試）。
- [ ] 八類 v1 稽核事件齊備且欄位完整；每筆含 `proposer_kind`。`compensation_proposal_cancelled` 保留 enum 但未被觸發。
- [ ] `/compensation` 的 inDevelopment 標記與「開發中」badge 已移除。
- [ ] `cargo nextest run -p api-gateway compensation` + `cargo nextest run -p referral-service scope` 全綠；`pnpm --filter @prophetica/admin lint` + `test` 全綠；Playwright e2e 三條主路徑通過。
- [ ] SQLx offline metadata（`.sqlx/`）已重新產生並提交（由 CI / 強力主機驗證，記錄主機）。

## 範圍外（後續任務）

- **Affiliate UI（/me/affiliate/compensation-requests）**：v1.1 獨立 plan，內容：
  - `/me/affiliate/compensation-requests` 子頁（提案表單 + 自己提案的狀態清單）
  - `app/api/me/affiliate/compensation-requests/route.ts`（affiliate-scoped route handler）
  - 共用 v1 已建好的 `compensation-bff.ts`（但呼叫時帶 affiliate 身份）
  - 既有 backend 守門（scope check、cap、`proposer_kind=affiliate` 稽核）v1 已 ready，v1.1 直接接 UI
- **客服系統（Zendesk / Freshdesk）→ compensation API webhook 接線**（SPEC-012 §11 既有，本 plan 範圍外）。
- **Cancel（提案者撤回）**：schema 已 reserved `cancelled` + `cancelled_at` + `compensation_proposal_cancelled` audit name。獨立 plan 補：
  - 轉移：`proposed → cancelled`（僅提案者本人，僅在任何審核動作前）
  - 端點：`POST .../compensation-proposals/:id/cancel`
  - audit `compensation_proposal_cancelled`
  - UI 提案區加「撤回」按鈕；歷史區顯示 cancelled
- **Reconcile worker 實作**（v1.1）：演算法已於 Task 7 鎖定，僅缺 worker 殼。建議 cron 5 min 掃一次，per-proposal 指數退避防雪崩。
- **Reversal（沖回）**：需新增 proto enum `TXN_TYPE_CS_COMPENSATION_REVERSAL`、ADR-0006 追認 INV-6 加 (g) `Expense.CustomerSupportCompensation` carve-out（SPEC-002 §4.4.6 已預告此擴展路徑）、wallet-ledger handler 強制 `parent_txn_id` 與累計餘額守門；admin app 補 reversal 兩步工作流（獨立 cap）。
- **多層 referral scope**：v1 僅 direct_referral；若 SPEC-011 將來開放多層 attribution，scope 端點可擴 `relationship` 值，compensation 守門依此分級給不同 cap。
- **多幣別補償**（`PostEntry.currency`，SPEC-016 Phase 2）。
- **SLA / 時效告警**（`posting_failed` / `posting_unknown` 超過 N 分鐘 → on-call alert）。
