# Implementation Plan: Gateway 透傳 member-summary 的 404（案B）

> 追溯：**SPEC-014 §6.1.1**「Gateway REST proxy 錯誤透傳」（v0.2.8）
> 缺陷：後台 KYC 審核會員查詢（`/members/kyc-review`）對不存在會員回 `500 ERROR_CODE_INTERNAL`，應回 `404 ERROR_CODE_MEMBER_NOT_FOUND`。

## Overview

api-gateway 的 `member_summary` handler 把 upstream（user-service）的所有錯誤
都壓成 `500`。根因有兩處：`user_service_json`（`services/api-gateway/src/admin_ops.rs:845`）
用 `.error_for_status()` 把 upstream 的 HTTP 狀態碼與回應主體整個丟棄；
`member_summary` handler（`admin_ops.rs:419`）再用 `.map_err(|_| internal_error())`
把僅剩的字串錯誤也丟棄。user-service 本身（`services/user-service/src/members.rs:1131`
`store_error`）已正確回 `404 ERROR_CODE_MEMBER_NOT_FOUND`——問題完全在 gateway。

案B：讓 `AdminOpsStoreError` 結構化地帶上 upstream 的 HTTP 狀態碼與 error code，
讓 `member_summary` handler 依此透傳，符合 SPEC-014 §6.1.1。

## Architecture Decisions

- **`AdminOpsStoreError` 改為附帶 upstream 狀態（additive）**：新增 `upstream_status`、
  `upstream_code` 兩個 `Option` 欄位與 `from_upstream` 建構子。既有 `new()` 與
  `message()` 不變，因此 `pending_member_cases_error` / `review_error` 等既有的字串比對
  消費者完全不受影響。
- **不擴大 `ErrorResponse`**：`ErrorResponse.error` 是 `&'static str`，全檔 27 處使用。
  改成 `Cow`/`String` 以「逐字回傳任意 upstream code」屬於與本缺陷正交的清理，超出範圍。
  user-service 的 error code 是封閉 enum（`ERROR_CODE_MEMBER_NOT_FOUND` /
  `_MEMBER_EXISTS` / `_INVALID_MEMBER` / `_INTERNAL`），mapper 以「已知 code → 同名靜態字串」
  對映即可，語意上等同透傳，完全符合 §6.1.1。
- **本次只動 `member_summary` 經路**。`pending_member_cases_error` 等其他 handler 收斂到
  新結構化狀態，列為後續工作（見 Open Questions），不混入本 PR。
- **`502` 的 unavailable code 用 `ERROR_CODE_USER_SERVICE_UNAVAILABLE`**，與既有
  `ERROR_CODE_KYC_SERVICE_UNAVAILABLE`（`pending_member_cases_error`）命名一致。
  這些是 gateway 自訂的純字串 code，與 errors.proto 無耦合。

## 受影響檔案

- `services/api-gateway/src/admin_ops.rs` — 錯誤型別、`user_service_json`、handler、mapper
- `services/api-gateway/tests/admin_ops.rs` — 新增測試

## Task List

### Phase 1：結構化錯誤型別

- [ ] **Task 1**：`AdminOpsStoreError` 附帶 upstream 狀態

### Phase 2：捕捉 upstream 狀態碼

- [ ] **Task 2**：`user_service_json` 捕捉非 2xx 的狀態碼與 error code

### Checkpoint：基礎（Task 1–2）

- [ ] `cargo check -p api-gateway --locked` 通過
- [ ] `cargo nextest run -p api-gateway --locked` 全綠（既有測試不回歸）
- [ ] 確認其他 `user_service_json` 消費者（`list_members` / `create_member` 等）行為未變

### Phase 3：handler 透傳

- [ ] **Task 3**：新增 `member_summary_error` mapper 並接進 `member_summary` handler

### Phase 4：測試

- [ ] **Task 4**：新增 404 與 502 的整合測試

### Checkpoint：完成（Task 3–4）

- [ ] `cargo clippy -p api-gateway --all-targets --locked -- -D warnings` 無告警
- [ ] `cargo nextest run -p api-gateway --locked` 全綠
- [ ] 手動：`/members/kyc-review` 查不存在會員 → 回 `404`，回應主體含 `ERROR_CODE_MEMBER_NOT_FOUND`
- [ ] 交付 reviewer

---

## Task 1：`AdminOpsStoreError` 附帶 upstream 狀態

**Description：** 為 `AdminOpsStoreError`（`admin_ops.rs:330`）新增 `upstream_status: Option<u16>`
與 `upstream_code: Option<String>` 兩欄位，並提供 `from_upstream(status, code, message)`
建構子與 `upstream_status()` / `upstream_code()` 取值方法。既有 `new()` 與 `message()` 維持原樣。

**Acceptance criteria：**

- [ ] `AdminOpsStoreError` 有 `from_upstream` 建構子與兩個取值方法
- [ ] `new()` 建出的錯誤其 `upstream_status()` / `upstream_code()` 皆為 `None`
- [ ] `Debug, Clone, PartialEq, Eq` derive 仍成立

**Verification：**

- [ ] `cargo check -p api-gateway --locked` 通過
- [ ] `cargo clippy -p api-gateway --lib --locked -- -D warnings` 無告警

**Dependencies：** None
**Files likely touched：** `services/api-gateway/src/admin_ops.rs`
**Estimated scope：** XS（1 檔）

---

## Task 2：`user_service_json` 捕捉非 2xx 的狀態碼與 error code

**Description：** 把 `user_service_json`（`admin_ops.rs:845`）的 `.error_for_status()`
換成手動處理：`send()` 後先取 `response.status()`，非 2xx 時讀回應主體、以一個私有
`UpstreamError { error: String }` 結構嘗試解析出 error code，組成
`AdminOpsStoreError::from_upstream(...)`。`send()` 本身的傳輸層失敗路徑保持逐字不變
（維持既有 message 文字，避免影響 `pending_member_cases_error` 的子字串比對）。

**Acceptance criteria：**

- [ ] upstream 回非 2xx 時，回傳的 `AdminOpsStoreError` 帶有正確的 `upstream_status()`
- [ ] upstream 回應主體可解析時，`upstream_code()` 為該 code；不可解析時為 `None`
- [ ] 傳輸層失敗（連線拒絕等）路徑的 `AdminOpsStoreError` 與改動前一致（`upstream_status()` 為 `None`、message 文字不變）

**Verification：**

- [ ] `cargo check -p api-gateway --locked` 通過
- [ ] `cargo nextest run -p api-gateway --locked` 全綠（特別確認既有 `pending_member_cases` 與 `list_members` 相關測試）

**Dependencies：** Task 1
**Files likely touched：** `services/api-gateway/src/admin_ops.rs`
**Estimated scope：** S（1 檔）

---

## Task 3：`member_summary_error` mapper 並接進 handler

**Description：** 新增 `member_summary_error(&AdminOpsStoreError)` 函式，依 SPEC-014 §6.1.1：
有 `upstream_status()` → 透傳該狀態碼，code 取自 `upstream_code()`（已知者）或由狀態碼推導
（`404`→`ERROR_CODE_MEMBER_NOT_FOUND`、`400`→`ERROR_CODE_INVALID_MEMBER`、
`409`→`ERROR_CODE_MEMBER_EXISTS`，其餘 5xx→`ERROR_CODE_INTERNAL`）；
無狀態碼且為傳輸層失敗 → `502 ERROR_CODE_USER_SERVICE_UNAVAILABLE`；其餘 → `internal_error()`。
把 `member_summary` handler（`admin_ops.rs:419`）的 `.map_err(|_| internal_error())`
換成 `.map_err(|error| member_summary_error(&error))`。

**Acceptance criteria：**

- [ ] upstream 404 → handler 回 `404` + `ERROR_CODE_MEMBER_NOT_FOUND`
- [ ] 傳輸層失敗 → handler 回 `502` + `ERROR_CODE_USER_SERVICE_UNAVAILABLE`
- [ ] gateway 自身未預期失敗（如 JSON 解析失敗）→ 仍回 `500 ERROR_CODE_INTERNAL`

**Verification：**

- [ ] `cargo clippy -p api-gateway --all-targets --locked -- -D warnings` 無告警
- [ ] `cargo check -p api-gateway --locked` 通過

**Dependencies：** Task 1、Task 2
**Files likely touched：** `services/api-gateway/src/admin_ops.rs`
**Estimated scope：** S（1 檔）

---

## Task 4：404 與 502 整合測試

**Description：** 在 `tests/admin_ops.rs` 新增測試。404 路徑：起一個最小 axum mock router
（單一路由回 `404` + `{"error":"ERROR_CODE_MEMBER_NOT_FOUND"}`）於 `TcpListener`，
以 `HttpAdminOpsStore::new(mock_url, mock_url)` 指向它，呼叫 admin router 斷言回 `404`
且主體含該 code。502 路徑：沿用既有 `HttpAdminOpsStore::new("http://127.0.0.1:1", ...)`
模式（連線拒絕），斷言回 `502` + `ERROR_CODE_USER_SERVICE_UNAVAILABLE`。兩測試皆不需 Postgres。

**Acceptance criteria：**

- [ ] 新增 member-summary 404 透傳測試，斷言狀態碼與 error code
- [ ] 新增 member-summary 502 transport-failure 測試
- [ ] 兩測試不依賴 Docker / Postgres，可單獨以 `cargo nextest run -p api-gateway <name>` 跑

**Verification：**

- [ ] `cargo nextest run -p api-gateway --locked` 全綠
- [ ] 手動：`make dev` 後於 `/members/kyc-review` 查不存在會員 → 回 `404`，主體含 `ERROR_CODE_MEMBER_NOT_FOUND`（非 `500`）

**Dependencies：** Task 3
**Files likely touched：** `services/api-gateway/tests/admin_ops.rs`
**Estimated scope：** S（1 檔）

---

## Risks and Mitigations

| Risk                                                                         | Impact | Mitigation                                                                                            |
| ---------------------------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------- | --- | ------------------------------------------------------------------ |
| `user_service_json` 為 `list_members` / `create_member` 等共用，改動波及它們 | Med    | 改動為 additive；傳輸層失敗路徑逐字不變；那些 handler 本就 `.map_err(                                 | \_  | internal_error())`，不讀 message。Task 2 跑全 crate 測試確認無回歸 |
| `ErrorResponse.error` 是 `&'static str`，無法逐字回任意 upstream code        | Low    | user-service code 為封閉 enum，mapper 以已知 code 對映即可；不擴大 `ErrorResponse`（27 處、正交清理） |
| 編譯/測試在資源受限主機過重                                                  | Low    | 全程用 `-p api-gateway` 目標式指令；新測試不需 Postgres。動到的非共享 crate，毋須整 workspace 驗證    |

## Open Questions

- `pending_member_cases_error`（子字串比對）與其他 `user_service_json` 消費者要不要一併收斂到
  Task 1 的結構化狀態？建議列為**後續 PR**，不混入本次 案B（維持 PR 原子性）。
- 是否需要先送 spec PR（SPEC-014 v0.2.8 已改檔，尚未提交）再開本實作 PR？依 CLAUDE.md SDD
  紀律「spec PR 先於程式碼 PR 送審」——本計畫待 spec PR 通過後才進入實作。
