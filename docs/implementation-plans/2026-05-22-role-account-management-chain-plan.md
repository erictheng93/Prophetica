# 角色帳號分權管理鏈路 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地 `SPEC-017 §6.3.5` 的自上而下帳號管理鏈 `admin → manager → affiliate（代理）`：admin 對**全部** manager 帳號可增刪改查；manager 僅對**自己旗下**（`managing_manager_id` = 本人）的 affiliate 帳號可增刪改查；並於 server 端硬性禁止一切反向／橫向管理與提權。刪除一律軟刪除，每筆 mutation 強制留稽核。

**Architecture:** 拆兩層授權。**路由層**（既有 `SPEC-017 §6.3.1`，`apps/admin/lib/rbac.ts`）決定誰能進哪個頁面；**資料列層**（本計畫新增）決定 manager 只看得到／改得動自己旗下的代理。歸屬以 `affiliate_profiles` 新增的 `managing_manager_id` 表達，與既有 `owner_sponsor_id`（SPEC-011 推薦掛靠）語意分離。actor（`user_id` + `role`）由 api-gateway 經 `x-actor-id` / actor-role header 注入下游服務（既有機制，見 `referral-service/src/lib.rs:1899`），下游據此做 row-level 過濾。manager 帳號本身存於 `user-service`（login credentials + `role`），由 admin 專屬管理。**referral-service 是 affiliate 管理歸屬與範圍判斷的唯一事實來源；user-service 不保存、不複製、不自行判斷 `managing_manager_id`。**

**Tech Stack:** Rust 2024 + axum + sqlx（offline）；Next.js 16 / React 19（admin app，ESM、import 用 `.js` 副檔名）；Node `node:test`。

**決策摘要（2026-05-22，使用者確認）:**

| 項目 | 決策 |
|---|---|
| 代理定義 | 沿用既有 `affiliate` 角色與 `affiliate_profiles`，不新增帳號類型 |
| 管理範圍 | manager 只能管自己旗下（`managing_manager_id` = 本人）的代理 |
| 規格落點 | 擴充 `SPEC-017 §6.3.5`（已起草，本計畫 Phase 0） |
| 歸屬欄位 | 新增 `managing_manager_id`，與 `owner_sponsor_id` 語意分離；指派值須為存在且未停用的 manager，`NULL` 表 admin 池 |
| 刪除語意 | 軟刪除（停用／封存），不物理刪除（SPEC-018 稽核留存） |
| 歸屬重指派 | 僅 admin 可改 `managing_manager_id`；manager 不得移交歸屬。執法採 **deny-by-presence**：manager 之 PATCH payload 出現此欄位即 403（不比對現值）。BFF 在 manager 視角 strip 此欄位以避免表單回填誤拒 |
| 角色指派 | 僅 admin 可指派 `admin`／`manager` 角色；manager 建帳號強制 `affiliate` |
| MLM 邊界 | 純行政管理權，不沿管理鏈分潤；與 SPEC-011 §2.1 下線金字塔切割 |
| 跨服務建立 | affiliate 建立由 api-gateway saga/outbox 編排，必須冪等且可補償，避免 user credential 與 affiliate profile 半提交 |
| 範圍事實來源 | `managing_manager_id` 只在 referral-service 執法；user-service 僅管 credential / role / disabled 狀態 |
| 抽佣費率 | manager 可調自己旗下代理之 `commission_rate_bps`（單層佣金率，既有 `:1982` 不擋）；不得改信用控制 `credit_*`、`role`、`managing_manager_id`；屬 SPEC-011 單層佣金、非多層分潤 |

---

## 前置與排序

- **Phase 0（spec）必須先合併**，後續實作 PR 才能引用 `SPEC-017 §6.3.5`（repo SDD 紀律）。§6.3.5 規格段落已起草於 `specs/SPEC-017-monorepo-bootstrap.html`（v0.2.9），本計畫 Phase 0 僅作收尾與交叉引用同步。
- Phase 1–6 為實作，依序進行；每個 Task 結束時 commit（Conventional Commits，scope = 對應 crate / `admin`）。
- 受限於開發機算力（CLAUDE.md Compute Policy）：Rust 一律用 `-p <crate>` 標的指令，不跑 workspace 全量。SQLx 為 offline 模式，新增／改動 query 後需重新 `cargo sqlx prepare`——對 `.sqlx/` 與 migration 的改動 push branch 由 CI 或強力主機驗證（CLAUDE.md），並註明驗證主機。
- **不需改 proto、不需 `make proto`**：本計畫無新增 gRPC 訊息。

## 既有資產盤點（實作前先讀）

- `services/referral-service/migrations/202605150001_create_affiliate_profiles.sql` — `affiliate_profiles` 主表，已有 `owner_sponsor_id` 與 `idx_affiliate_profiles_owner`；**無 `managing_manager_id`**。
- `services/referral-service/migrations/202605150003_affiliate_admin_crud_audit.sql` — `affiliate_operation_audit_logs`（`actor_id` / `actor_role` CHECK IN (`admin`,`manager`) / `reason` / `authorization_basis` / `before_summary` / `after_patch`）；`status` 已含 `archived`。**稽核軌道現成，沿用。**
- `services/referral-service/src/lib.rs:430` — `AdminAffiliateActor`（由 `x-actor-id` header 解析，見 `:1899`）。
- `services/referral-service/src/lib.rs:1921-1993` — `validate_admin_affiliate_mutation`：**已有** `admin|manager` 角色閘（`:1926`）與「manager 不得改信用控制」（`:1982`）；**缺** `managing_manager_id` 範圍強制。
- `services/referral-service/src/lib.rs:1285` / `:1353` / `:1372` / `:3170` / `:3237` / `:3327` — affiliate create / update / list / get 之 store 方法，需加範圍條件。
- `services/user-service/src/members.rs:283-353` — `internal_members_router` 與 handlers：**handler 無 actor 角色閘**；`create_member` 建 login credential 時 `role` 寫死 `player`（供裝邏輯約 `:705-738`）。
- `services/user-service/src/auth.rs:992-994` — role 驗證 `admin|manager|affiliate`。
- `apps/admin/lib/rbac.ts:3` — `ALL_ROLES`；`:27-49` — 路由 × 角色矩陣（單一事實來源）。
- `apps/admin/lib/affiliate-management-bff.ts` — affiliate BFF（manager 目前唯讀）。
- `apps/admin/app/affiliate/actions.ts:75` — 已送 `ownerSponsorId`（勿與 `managingManagerId` 混用）。
- `apps/admin/lib/member-management-bff.ts` + `services/api-gateway/src/internal_call_auth.rs` — signed internal-call 參考實作，manager-帳號 BFF 比照。

## 檔案結構

| 檔案 | 動作 | 責任 |
|---|---|---|
| `specs/SPEC-017-monorepo-bootstrap.html` | 改（已起草） | §6.3.5 分權 CRUD（v0.2.9） |
| `docs/implementation-plans/SPEC-017-monorepo-bootstrap-implementation-plan.html` | 改 | 補指向本計畫檔的段落 |
| `services/referral-service/migrations/202605220001_add_managing_manager_id.sql` | 建 | `affiliate_profiles` 加 `managing_manager_id TEXT NULL` + 索引 |
| `services/referral-service/migrations/202605220002_affiliate_denied_audit.sql` | 建 | affiliate domain denied audit sink（nullable target，支援 denied create attempted payload） |
| `services/referral-service/src/lib.rs` | 改 | DTO 加 `managing_manager_id`；list/get/mutate 加範圍條件；manager-create 強制 self；admin-only 重指派與角色閘 |
| `services/referral-service/tests/` | 改／建 | 範圍強制、提權禁制、歸屬重指派、稽核測試 |
| `services/user-service/migrations/202605220001_member_account_audit.sql` | 建 | manager 帳號操作稽核表（比照 affiliate_operation_audit_logs） |
| `services/user-service/migrations/202605220002_member_account_denied_audit.sql` | 建 | user-service handler 自身拒絕事件的 denied audit sink（actor_role 不限 admin） |
| `services/user-service/src/members.rs` | 改 | handler 接 actor（`user_id`/`role`）；manager 帳號 CRUD 限 admin；建帳號可指派 `manager`/`affiliate` 角色；軟刪除前孤兒檢查 |
| `services/user-service/tests/` | 改／建 | 角色閘、角色指派、提權禁制、孤兒處理測試 |
| `services/api-gateway/migrations/202605220003_role_account_denied_audit.sql` | 建 | gateway 粗篩與 saga orchestration denied audit sink |
| `services/api-gateway/migrations/202605220004_affiliate_account_sagas.sql` | 建 | affiliate account create saga/outbox 狀態表與 idempotency unique key |
| `services/api-gateway/src/*` | 改 | manager 帳號管理端點透傳；注入 actor header；路由授權 |
| `services/api-gateway/tests/` | 改／建 | gateway 授權、透傳、affiliate 建立 saga/補償/idempotency 測試 |
| `apps/admin/lib/rbac.ts` | 改 | manager-帳號管理路由 admin-only allowlist；affiliate domain capability 升級為範圍內可寫 |
| `apps/admin/lib/manager-accounts-bff.ts` | 建 | BFF：signed call → gateway（manager 帳號 list/create/update/disable） |
| `apps/admin/app/settings/accounts/actions.ts` | 建 | server actions（admin-only）呼叫 BFF，避免新增 client-side manager-account API surface |
| `apps/admin/app/settings/accounts/page.tsx` | 建 | manager 帳號管理頁（admin 專屬） |
| `apps/admin/lib/affiliate-management-bff.ts` | 改 | manager 由唯讀升級為範圍內 CRUD；送 `managingManagerId`（admin 建立時） |
| `apps/admin/app/affiliate/**` | 改 | manager 視角 CTA／資料範圍依 §6.3.5.3 過濾 |
| `apps/admin/tests/` | 建 | BFF / route handler 純函式單元測試（`node --test`） |

---

## Phase 0 — Spec PR（先合併）

### Task 1: 確認 SPEC-017 §6.3.5 規格段落
- [x] 複核 `specs/SPEC-017-monorepo-bootstrap.html` v0.2.9 §6.3.5（管理權矩陣、歸屬模型、範圍強制、反向禁制、API 缺口、孤兒處理、路由銜接）內容完整且自洽。
- [x] 確認 §6.3.5 與 §6.3.1 不衝突：§6.3.1 `/affiliate` 對 manager 必須標為「✅ 範圍內可寫（資料列授權見 §6.3.5）」，不得保留「唯讀」文字。

### Task 2: 同步交叉引用
- [x] `docs/implementation-plans/SPEC-017-monorepo-bootstrap-implementation-plan.html` 補一段指向本計畫檔。
- [x] 確認 SPEC-011 §2.1（MLM 紅線）與本節的「行政管理 vs 抽佣下線」切割敘述一致；如 SPEC-011 需回指 §6.3.5，於該 PR 一併補。

**Phase 0 驗收：** spec PR 通過審查並合併；PR 描述引用 `SPEC-017 §6.3.5`。

---

## Phase 1 — 資料模型（歸屬欄位 + 稽核）

### Task 3: affiliate 歸屬欄位
- [x] 新 migration：`ALTER TABLE affiliate_profiles ADD COLUMN managing_manager_id TEXT NULL;` + `CREATE INDEX idx_affiliate_profiles_managing_manager ON affiliate_profiles(managing_manager_id);`
- [x] **不**動 `owner_sponsor_id`（語意分離）。
- [x] 既有列 backfill 策略：留 `NULL`（= admin 池），由 admin 後續指派。於 migration 註解說明。
- [x] 不建立跨服務 foreign key；`managing_manager_id` 的有效性由 api-gateway / service 層查 user-service，必須是存在且未停用的 `manager`，或為 `NULL`。

### Task 4: manager 帳號稽核表
- [x] 新 migration（user-service）：`member_account_audit_logs`，欄位比照 `affiliate_operation_audit_logs`（`actor_id` / `actor_role` / `action` ∈ (`create`,`update`,`disable`,`reassign`) / `reason` / `authorization_basis` / `before_summary` / `after_patch` / `created_at`）。
- [x] `actor_role` CHECK 限 `admin`（manager 帳號僅 admin 可動）。
- [x] 新 migration（user-service）：`member_account_denied_audit_logs`，存 user-service handler 自己判定的 denied role/account 操作，例如 manager 修改自身 role、manager/affiliate 修改 admin 或 manager 帳號。`actor_role` 不得限 admin；至少允許 `admin` / `manager` / `affiliate` / `player`，並支援 nullable `target_user_id` 與 attempted payload。

### Task 4b: denied operation audit sink
- [x] referral-service 新 migration 建立 affiliate domain denied audit sink，存 referral-service 自己判定的 range、reassign、credit-control、denied-create 等拒絕事件。欄位至少包含 `correlation_id`、`target_type`、`target_id NULL`、`action`、`actor_id`、`actor_role`、`reason_or_error_code`、`attempted_payload`、`request_id`、`ip`、`user_agent`、`created_at`。
- [x] api-gateway 新 migration 建立 gateway denied audit sink，存 gateway 粗篩、active-manager 驗證、saga orchestration / idempotency conflict 等 gateway 層拒絕事件。
- [x] 不把 denied create 寫入既有 `affiliate_operation_audit_logs`：該表 `affiliate_id TEXT NOT NULL REFERENCES affiliate_profiles(affiliate_id)`，被拒的 create 可能沒有 target row，會讓稽核自身失敗。
- [x] update/delete 類成功 mutation 繼續寫既有成功 audit 表；denied mutation 寫該拒絕決策所在服務的 denied sink。

**Phase 1 驗收：** migration 於本地 Postgres 套用成功；`cargo sqlx prepare` 後 `.sqlx/` 更新，push branch 交 CI 驗證（記錄驗證主機）。

---

## Phase 2 — referral-service：代理範圍強制

**Source of truth:** referral-service 是 `managing_manager_id` 與 affiliate row-level scope 的唯一執法點。user-service 不得新增 `managing_manager_id` 欄位，也不得用自己的資料推導 affiliate scope。

### Task 5: DTO 與 store 加歸屬
- [x] `AdminAffiliateMutationRequest` / summary DTO 加 `managing_manager_id: Option<String>`（serde `managingManagerId`）。
- [x] create / update / list / get SQL（`lib.rs` 約 `:3170`/`:3237`/`:3327`/list query）讀寫 `managing_manager_id`。

### Task 6: 範圍強制（§6.3.5.3）
- [x] list / get / update / soft-delete：當 `actor.role == "manager"`，SQL 附加 `AND managing_manager_id = :actor_id`（`actor_id` 來自 `AdminAffiliateActor`）。範圍外目標 → `403 Forbidden`（清單則不可見）。
- [x] `actor.role == "admin"` 不加範圍條件。

### Task 7: 建立與歸屬規則（§6.3.5.2 / §6.3.5.4）
- [x] manager create affiliate：server **強制** `managing_manager_id = actor.actor_id`，忽略／拒絕呼叫端帶入的他人值。
- [x] admin create / reassign affiliate：可帶任一 `managing_manager_id` 或留 `NULL`；非 `NULL` 時必須由 api-gateway 先查 user-service 驗證該 user 是存在且未停用的 manager，再呼叫 referral-service。referral-service 不直接依賴 user-service。
- [x] **歸屬重指派限 admin（deny-by-presence）**：`validate_admin_affiliate_mutation` 中，`actor.role == "manager"` 且 request **出現** `managing_manager_id` 欄位（無論值是否變動）→ `Forbidden`，並寫 denied audit。實作見 `referral lib.rs:1510 / 3644` 之 `.is_some()` 檢查，對齊 SPEC §6.3.5.2 / §6.3.5.4 之 deny-by-presence。
- [x] 沿用既有 `:1982` manager 不得改信用控制之限制（不回歸）。
- [x] **抽佣費率（決策）**：range 內 manager **可**改 `commission_rate_bps`——不得把它加進 `:1982` 禁改清單。費率變更須寫 `affiliate_rate_changes`（`changed_by` / `reason`）＋ operation audit。屬 SPEC-011 單層佣金，不沿管理鏈分潤。

### Task 8: referral-service 測試
- [x] manager 讀／改自己旗下代理 → 成功；讀／改他人代理 → 403。
- [x] manager create → `managing_manager_id` 被強制為自身。
- [x] manager 試圖重指派歸屬 → 403；admin 重指派 → 成功。
- [x] admin 指派不存在或已停用的 manager → 400/403，並留 denied audit。
- [x] 每筆 mutation 寫入 `affiliate_operation_audit_logs`（actor、reason、authorization_basis 齊備）；範圍外、重指派、提權等被拒操作也要寫 denied audit。
- [x] manager 改自己旗下代理 `commission_rate_bps` → 成功，且寫入 `affiliate_rate_changes` ＋ operation audit；改 `credit_*` 信用控制 → 403。
- [x] `cargo nextest run -p referral-service --locked`

---

## Phase 2.5 — api-gateway：affiliate 建立 saga / outbox

### Task 8b: affiliate account orchestration
- [x] 新增 affiliate account create orchestration endpoint / service flow：api-gateway 作為 orchestrator，依序協調 user-service 建立 login credential（`role=affiliate`）與 referral-service 建立 `affiliate_profiles`（含 `managing_manager_id`）。
- [x] admin app / BFF 建立 affiliate 帳號只能呼叫此 saga endpoint；既有 referral-service affiliate create endpoint 只可作為 saga 內部 profile-create step 或 profile-only 維護 API，不得由 UI 直接當作帳號建立入口。
- [x] admin 帶入非 `NULL managing_manager_id` 時，api-gateway 在 saga 開始前查 user-service 驗證 active manager；失敗寫 gateway denied audit，且不得呼叫 referral-service。
- [x] 要求 `Idempotency-Key`；相同 key + 相同 payload 重送回同一結果，相同 key + 不同 payload 回 idempotency conflict。
- [x] 建立 saga/outbox 表記錄 `idempotency_key`、`request_hash`、`actor_id`、`actor_role`、`target_user_id`、`affiliate_id`、`status`（例如 `credential_created`、`profile_created`、`completed`、`compensating`、`failed`）、`last_error`、`attempt_count`、`created_at`、`updated_at`。
- [x] 半提交補償（目前採 retry repair path）：若 user-service credential 已建立但 referral-service profile 失敗，saga durable 記錄 `credential_created` 與 `last_error`，回非成功狀態；同一 idempotency key 重送時跳過 credential create 並重試 profile 建立，不得對外回完整成功。
- [x] 反向半提交補償：目前 saga 採 credential-first，credential 建立成功前不呼叫 referral-service，因此正常路徑不會產生「有 profile 登不進」；若未來加入 profile-first repair，需另補 profile 封存/重試 endpoint 與 audit。
- [x] saga 狀態與補償動作必須可查詢、可告警、可人工修復；目前提供 durable saga status query 與 admin repair context，失敗狀態含 `last_error` 與 idempotency key，可用同 key retry profile 建立。

### Task 8c: affiliate account orchestration tests
- [x] user-service 成功、referral-service 失敗 → 回非成功狀態，saga 記錄 `credential_created/failed`，重送同 key 可完成 profile。
- [x] referral-service 成功、user-service 確認失敗 → 目前 credential-first sequencing 使此狀態不可達；測試覆蓋 gateway 在 credential 前的 manager 預檢與 credential-created/profile-failed retry path。
- [x] 同 idempotency key 重送同 payload → replay；同 key 不同 payload → conflict。
- [x] `cargo nextest run -p api-gateway --locked`

---

## Phase 3 — user-service：manager 帳號 CRUD 與閘門

### Task 9: handler 接 actor 並設角色閘
- [x] `internal_members_router` mutation handlers 改為接收 actor（`x-actor-id` + `x-operator-role`，由 gateway header 注入）；目前已套用 create/update/delete member mutations。
- [x] **manager/member 帳號 mutation 閘門**（建立／編輯／停用）→ 限 `actor.role == "admin"`，否則 403 並寫 `member_account_denied_audit_logs`。
- [x] **affiliate credential create 操作** → user-service 只驗證 actor role 與 requested role；不得判斷 `managing_manager_id`。affiliate scope 一律由 api-gateway 呼叫 referral-service 查驗後再進入 user-service mutation。
- [x] 既有 member（player）流程不回歸。

### Task 10: 角色指派與提權禁制（§6.3.5.4）
- [x] 建帳號可指派 `role ∈ {manager, affiliate}`（移除寫死 `player`，保留 `player` 為公開註冊預設）。
- [x] 角色指派為 admin 專屬：非 admin 試圖建立 `admin`／`manager` 角色 → 403。
- [x] manager 建立 affiliate 帳號時，server 僅允許 `role = affiliate`，不得由 payload 覆寫成其他角色。
- [x] 任何 actor 不得修改自身 `role` → 403。

### Task 11: 軟刪除與孤兒處理（§6.3.5.6）
- [x] manager 帳號刪除 = 軟刪除（`disabled`）。
- [x] 停用 manager 前檢查其旗下 affiliate（跨服務查 referral-service：`managing_manager_id = target_manager`）；若存在未重指派之代理 → 拒絕並回明確錯誤（要求 admin 先重指派或移入 admin 池）。
- [x] 每筆 manager 帳號操作寫 `member_account_audit_logs`。

### Task 12: user-service 測試
- [x] 非 admin 對 member/manager 帳號 mutation → 403，並寫入 user-service denied audit sink。
- [x] manager 建帳號帶 `role=manager`／`role=admin` → 403；帶 `role=affiliate` → 成功。
- [x] manager 對自身 role 修改、對其他 manager/admin 修改 → 403，並寫入 `member_account_denied_audit_logs`（不得寫入 `member_account_audit_logs`，因成功 audit 的 `actor_role` CHECK 限 admin；目前已先覆蓋非 admin mutation denial）。
- [x] user-service 測試不得假設或新增 `managing_manager_id`；scope 測試放 referral-service / api-gateway orchestration。
- [x] 停用尚有旗下代理的 manager → 拒絕；重指派後 → 成功。
- [x] `cargo nextest run -p user-service --locked`

---

## Phase 4 — api-gateway：授權與透傳

### Task 13: actor 注入與路由授權
- [x] manager/member 帳號 mutation 端點：gateway 自 session 注入 `x-actor-id` + actor-role header 至 user-service（比照既有 affiliate 路徑）。
- [x] gateway 層對 manager 帳號管理路由做 admin-only 粗篩（縱深防禦；最終真實來源仍為 user-service handler）。
- [x] affiliate 管理路徑：確認 actor header 已含 `actor_id`（供 referral-service 範圍判斷），補齊 list/get 缺漏。

### Task 14: api-gateway 測試
- [x] 缺 actor／角色不符 → 401/403。
- [x] actor header 正確透傳至下游。
- [x] `cargo nextest run -p api-gateway --locked`

---

## Phase 5 — admin app：UI 與 BFF

### Task 15: RBAC 矩陣與 capability
- [x] `apps/admin/lib/rbac.ts`：新增 `/settings/accounts` + `/api/manager-accounts` 為 **admin-only**（manager/affiliate 403）。
- [x] affiliate domain capability：manager 由唯讀升級為「範圍內可寫」（CTA 顯示，實際範圍由 server 過濾）。

### Task 16: manager 帳號管理（admin 專屬）
- [x] `apps/admin/lib/manager-accounts-bff.ts`（signed call → gateway）。
- [x] `apps/admin/app/settings/accounts/actions.ts` server actions（admin-only；不新增 client-side manager-account API surface）。
- [x] `apps/admin/app/settings/accounts/page.tsx`：manager 帳號清單 + 建立 / 編輯 / 停用 + 重指派代理歸屬。

### Task 17: 代理管理 manager 範圍寫入
- [x] `apps/admin/lib/affiliate-management-bff.ts`：manager 由唯讀改為範圍內 CRUD；admin 建立 affiliate 帳號時呼叫 api-gateway saga endpoint 並送 `managingManagerId`（勿覆寫 `ownerSponsorId`），不得直接呼叫 referral-service profile create 作為帳號建立入口。
- [x] 目前不提供 profile-only create / repair UI；若後續需要，必須明確標為 saga repair / profile maintenance，並只供具備權限的 admin 使用。
- [x] 表單與 payload 明確拆分：`ownerSponsorId` 只代表推薦／掛靠來源；`managingManagerId` 只代表行政管理歸屬。兩者不得互相 fallback 或覆寫。
- [x] **manager 視角 deny-by-presence strip**：`updateAdminAffiliate` 於 manager viewer 時呼叫 `stripManagingManagerIdForManager` 從 PATCH payload 移除 `managingManagerId`（spec §6.3.5.2 / §6.3.5.4）；admin viewer 保留欄位以支援重指派（呼應 `apps/admin/app/settings/accounts/actions.ts`）。對應測試見 `affiliate-management-bff.test.ts`「strips managingManagerId from PATCH payload for manager viewer」。
- [x] `apps/admin/app/affiliate/**`：manager 視角僅見／可改自己旗下代理；信用控制 CTA 對 manager 隱藏（呼應 server `:1982`）。

### Task 18: admin app 測試
- [x] BFF / route handler 純函式單元測試（`node --test`）：角色閘、payload 映射（`managingManagerId` vs `ownerSponsorId`）。
- [x] `pnpm --filter @prophetica/admin lint`（tsc）+ `test`

---

## Phase 6 — 端到端驗收

### Task 19: 驗收清單（對照 §6.3.5）
- [x] admin 可增刪改查全部 manager 帳號。✅
- [x] manager 可增刪改查**僅自己旗下**代理；範圍外 → 403。✅
- [x] admin 指派／重指派 `managing_manager_id` 只能使用存在且未停用的 manager，或 `NULL` admin 池。✅
- [x] `managing_manager_id` scope 只在 referral-service 執法；user-service 無此欄位、無重複範圍判斷。✅
- [x] affiliate 建立跨 user-service/referral-service 以 saga/outbox + `Idempotency-Key` 編排；任一服務失敗皆無無法修復的半提交。✅
- [x] admin app 建立 affiliate 帳號只能走 saga endpoint；無 UI/BFF path 可直接用 referral-service profile create 繞過 saga。✅
- [x] manager 不得管理 admin／其他 manager；不得提權；不得重指派歸屬。✅
- [x] manager 可調自己旗下代理 `commission_rate_bps`（單層佣金率）；改信用控制 `credit_*` → 403。✅
- [x] affiliate 不得管理任何帳號（403）。✅
- [x] 停用 manager 前強制處理孤兒代理。✅
- [x] 每筆 mutation 與 denied mutation 留稽核（actor／target／action／reason／authorization_basis 或拒絕原因）。✅
- [x] user-service 非 admin role/account denied operation 寫入 `member_account_denied_audit_logs`；不受 `member_account_audit_logs.actor_role CHECK admin` 阻擋。✅
- [x] denied create 不寫入 `affiliate_operation_audit_logs`；使用 nullable target 的 denied audit sink，可保存 attempted payload。✅
- [x] 軟刪除：無物理刪除帳號或稽核。✅（manager credential 停用以 `disabled_at` 表示；member row 仍走既有 `deleted_at` soft delete）

### Task 20: 全量驗證（交 CI／強力主機）
- [ ] push branch；CI 跑 `make test` + `make lint` 全量、`sqlx prepare --check`、`cargo geiger`。記錄驗證主機（CLAUDE.md Compute Policy）。

**2026-05-23 本機驗證記錄：**
- [x] `make test`：通過；Rust nextest 495 passed / 1 skipped，pnpm turbo test/build 通過。
- [x] `make lint`：通過；pnpm lint 8/8、workspace clippy `-D warnings`、`cargo fmt --all -- --check` 通過。
- [x] `cargo sqlx prepare --workspace --check`：通過；需本地 Postgres，已以 `docker compose -f infra/docker-compose.local.yaml up -d postgres` 啟動單一 postgres service。
- [x] `cargo geiger` 本機 fallback：`cargo-geiger 0.13.0` 不能直接對 virtual workspace manifest 跑 `--workspace`；完整 dependency JSON scan 在本機 package manifest 模式過慢且不穩定。已逐一對所有 workspace package manifest 跑 `cargo geiger --manifest-path <package>/Cargo.toml --forbid-only --output-format Json` 並用 `tools/scripts/check-geiger.sh` 檢查，全部通過。完整 CI 版仍需依 `.github/workflows/security-audit.yml` 用 stable toolchain 跑 `cargo geiger --workspace --output-format Json`。

---

## Open Questions（待落地時定案）

1. **Multi-sig**：批次停用 manager／大量重指派是否納入 SPEC-010 §9.3 multi-sig step-up？（§6.3.5.7 標為 OQ）
2. **跨服務孤兒檢查的一致性**：user-service 停用 manager 時查 referral-service 為跨服務讀；是否需在交易邊界外加補償（停用後背景掃描殘留歸屬）以防競態。
3. **affiliate 重指派的下游效應**：重指派 `managing_manager_id` 是否影響既有分潤歸集？依決策「管理與分潤分離」應為否——需在實作時以測試固化此不變式。
