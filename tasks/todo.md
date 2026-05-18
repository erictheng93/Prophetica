# TODO：Gateway 透傳 member-summary 的 404（案B · SPEC-014 §6.1.1）

詳細計畫見 [`plan.md`](./plan.md)。前置：SPEC-014 v0.2.8 spec PR 須先送審通過。

## Phase 1：結構化錯誤型別

- [x] **T1** `AdminOpsStoreError` 新增 `upstream_status` / `upstream_code` 欄位 + `from_upstream` 建構子 + 取值方法（`new()` / `message()` 不變）
  - 驗證：`rtk cargo check -p api-gateway --locked`

## Phase 2：捕捉 upstream 狀態碼

- [x] **T2** `user_service_json` 改手動處理非 2xx：捕捉狀態碼、解析回應主體 error code → `from_upstream`；傳輸層失敗路徑逐字不變
  - 驗證：`rtk cargo nextest run -p api-gateway --locked` 全綠

### ◆ Checkpoint：基礎（T1–T2）

- [x] `rtk cargo check -p api-gateway --locked` 通過
- [x] `rtk cargo nextest run -p api-gateway --locked` 無回歸
- [x] 確認 `list_members` / `create_member` 等共用消費者行為未變

## Phase 3：handler 透傳

- [x] **T3** 新增 `member_summary_error` mapper（404/400/409 透傳、傳輸失敗→502、其餘→500），接進 `member_summary` handler 取代 `.map_err(|_| internal_error())`
  - 驗證：`rtk cargo clippy -p api-gateway --all-targets --locked -- -D warnings`

## Phase 4：測試

- [x] **T4** `tests/admin_ops.rs` 新增 404 透傳測試（mock upstream router）+ 502 transport-failure 測試（`127.0.0.1:1`）
  - 驗證：`rtk cargo nextest run -p api-gateway --locked` 全綠

### ◆ Checkpoint：完成（T3–T4）

- [x] `rtk cargo clippy -p api-gateway --all-targets --locked -- -D warnings` 無告警
- [x] `rtk cargo nextest run -p api-gateway --locked` 全綠
- [ ] 手動：`/members/kyc-review` 查不存在會員 → `404` + `ERROR_CODE_MEMBER_NOT_FOUND`
- [x] 交付 reviewer
