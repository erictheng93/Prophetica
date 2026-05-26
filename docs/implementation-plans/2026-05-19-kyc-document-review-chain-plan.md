# KYC 證件取圖鏈路 (A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 讓 admin「會員管理 → KYC 審核」頁能即時檢視 KYC provider 收集的會員證件 / 自拍 / liveness 影像，全程經 `admin → api-gateway → kyc-service` 代理、不外露 vendor URL。

**Architecture:** kyc-service 新增 KYC provider document client 與內部 admin 端點（`user_id → provider_applicant_id` 解析、opaque `doc_id` 綁定驗證、稽核留痕）；api-gateway 新增「驗 signed call + streaming proxy」的 KYC document 路由；admin app 新增 streaming route handler 與證件 gallery UI。影像 bytes 全 3 hop streaming，不得 buffer。

**Tech Stack:** Rust 2024 + axum + reqwest + sqlx（offline）；Next.js 16 / React 19（admin app，ESM、import 用 `.js` 副檔名）；Node `node:test`。

**範圍：** 僅 (A) 即時取圖代理。設計文件 §5 的 (B) R2 歸檔為 spec-only，不在本計畫。對應設計：`docs/superpowers/specs/2026-05-19-kyc-document-review-chain-design.md`。

---

## 前置與排序

- **Phase 0（Task 1–2）是 spec PR，必須先合併**，後續實作 PR 才能引用 `SPEC-005 §5.x`（repo SDD 紀律）。
- Phase 1–6 為實作，依序進行；每個 Task 結束時 commit。
- 受限於開發機算力（CLAUDE.md Compute Policy）：Rust 一律用 `-p <crate>` 標的指令；不要跑 workspace 全量。SQLx 為 offline 模式，新增/改動 query 後需 `cargo sqlx prepare`（見 Task 7）。
- 全程對 `mock-provider` 開發；真 KYC provider 憑證接線受 SPEC-000 §5 Q-9 阻擋，不在本計畫。

## 檔案結構

| 檔案                                                                              | 動作 | 責任                                                  |
| --------------------------------------------------------------------------------- | ---- | ----------------------------------------------------- |
| `specs/SPEC-005-kyc-aml.html`                                                     | 改   | §5 展開為可實作規格（§5.3 取圖代理 / §5.4 R2 歸檔）   |
| `docs/implementation-plans/SPEC-005-kyc-aml-implementation-plan.html`             | 改   | 加 §5 段落，指向本計畫檔                              |
| `services/api-gateway/src/internal_call_auth.rs`                                  | 建   | 共用 signed-call 驗證模組（HMAC + timestamp skew）    |
| `tools/mocks/provider/src/lib.rs`                                                 | 改   | 新增 mock document list / image 端點                  |
| `services/kyc-service/src/provider_documents.rs`                                  | 建   | KYC provider document client（list / 下載影像 bytes） |
| `services/kyc-service/migrations/202605190001_create_kyc_document_access_log.sql` | 建   | 稽核表                                                |
| `services/kyc-service/src/kyc.rs`                                                 | 改   | `KycStore` 加文件端點所需方法                         |
| `services/kyc-service/src/api.rs`                                                 | 改   | 兩個內部 admin document 路由 + handler                |
| `services/api-gateway/src/admin_ops.rs`                                           | 改   | 兩個 KYC document gateway 路由 + streaming proxy      |
| `apps/admin/lib/kyc-documents-bff.ts`                                             | 建   | BFF：簽章 + streaming 代理 helper                     |
| `apps/admin/app/api/members/[userId]/kyc-documents/route.ts`                      | 建   | 文件清單 route handler                                |
| `apps/admin/app/api/members/[userId]/kyc-documents/[docId]/route.ts`              | 建   | 影像 streaming route handler                          |
| `apps/admin/lib/member-management-shared.ts`                                      | 改   | 新增 `KycDocument` / `KycDocumentList` 型別           |
| `apps/admin/components/kyc-document-gallery.tsx`                                  | 建   | 證件 gallery 元件                                     |
| `apps/admin/components/member-management.tsx`                                     | 改   | 以 gallery 取代 1311-1316 佔位卡片                    |

---

## Phase 0 — Spec PR（先合併）

### Task 1: 展開 SPEC-005 §5 HTML

**Files:**

- Modify: `specs/SPEC-005-kyc-aml.html`（§5「證件儲存與隱私」段落）

- [ ] **Step 1: 重構 §5**

把現有 §5.1 四條 bullet 擴成可實作規格。新增子節（沿用既有 HTML 結構與標題樣式）：

- **§5.3 即時取圖代理**：鏈路 `admin → api-gateway → kyc-service → KYC provider`；端點
  `GET /v1/internal/admin/kyc/members/{user_id}/documents`（清單）與
  `.../documents/{doc_id}`（影像）；綁定不變式（`user_id → applicant_id` 僅內部解析、
  `doc_id` 為 opaque、綁定失敗一律單一 404）；signed call 驗證在 api-gateway、
  影像全程 streaming；存取受 KYC review permission 限制（MVP = admin/manager）。
- **§5.4 R2 歸檔**：每日批次下載 KYC provider 影像至 R2（`kyc-documents` /
  `kyc-liveness`）、Object Lock 保存 7 年（標「待合規簽核」）、SSE-AES256；
  既有 `apps/{admin,web}/wrangler.toml` R2 binding 標為「歷史配置 / 待拆遷」。
- 在 §5 標注「短期 token」定義 = server-to-server signed internal call（不進瀏覽器）。

來源內容以設計文件 §3–§9 為準，逐段轉為 HTML。

- [ ] **Step 2: 更新版本與變更紀錄**

§0 變更紀錄新增一筆（bump patch 版本，如 v0.1.5），說明「§5 展開為取圖代理 +
R2 歸檔可實作規格」。

- [ ] **Step 3: Commit**

```bash
git add specs/SPEC-005-kyc-aml.html
git commit -m "docs(spec): expand SPEC-005 §5 into KYC document proxy + R2 archival spec"
```

### Task 2: 在 implementation-plan HTML 加 §5 指標

**Files:**

- Modify: `docs/implementation-plans/SPEC-005-kyc-aml-implementation-plan.html`

- [ ] **Step 1: 新增 §5 段落**

加一段「§5 KYC 文件取圖鏈路」，說明 (A) 即時代理 / (B) R2 歸檔分批交付，並以連結
指向本檔 `docs/implementation-plans/2026-05-19-kyc-document-review-chain-plan.md`。
不在 HTML 內重抄任務細節。

- [ ] **Step 2: Commit**

```bash
git add docs/implementation-plans/SPEC-005-kyc-aml-implementation-plan.html
git commit -m "docs(spec): link SPEC-005 §5 KYC document chain plan"
```

> **Phase 0 完成後開 spec PR、待審查合併，再進 Phase 1。**

---

## Phase 1 — api-gateway：共用 signed-call 驗證模組

設計 §3.1 結論 2：admin ops path 目前不驗 signed call。本 Phase 抽出共用驗證模組，
**僅供新的 KYC document 路由使用**；既有 admin ops 端點不動（設計 §13，範圍外）。

### Task 3: 建立 `internal_call_auth` 模組

**Files:**

- Create: `services/api-gateway/src/internal_call_auth.rs`
- Modify: `services/api-gateway/src/lib.rs`（加 `mod internal_call_auth;`）
- Test: `services/api-gateway/tests/internal_call_auth.rs`

- [ ] **Step 1: 寫失敗測試**

`services/api-gateway/tests/internal_call_auth.rs`：

```rust
use api_gateway::internal_call_auth::{sign_internal_call, verify_internal_call, InternalCallRejection};
use axum::http::HeaderMap;

fn headers(method: &str, path: &str, body: &[u8], ts: i64, actor: &str, role: &str, secret: &str) -> HeaderMap {
    let sig = sign_internal_call(method, path, body, ts, actor, role, secret);
    let mut h = HeaderMap::new();
    h.insert("x-actor-id", actor.parse().unwrap());
    h.insert("x-operator-role", role.parse().unwrap());
    h.insert("x-internal-call-timestamp", ts.to_string().parse().unwrap());
    h.insert("x-internal-call-signature", sig.parse().unwrap());
    h
}

#[test]
fn accepts_valid_signed_call() {
    let secret = "shared-secret-32-bytes-1234567890";
    let h = headers("GET", "/p", b"", 1_000, "U-1", "admin", secret);
    let actor = verify_internal_call(&h, "GET", "/p", b"", secret, 1_000).expect("valid");
    assert_eq!(actor.actor_id, "U-1");
    assert_eq!(actor.role, "admin");
}

#[test]
fn rejects_timestamp_skew_over_limit() {
    let secret = "shared-secret-32-bytes-1234567890";
    let h = headers("GET", "/p", b"", 1_000, "U-1", "admin", secret);
    let err = verify_internal_call(&h, "GET", "/p", b"", secret, 1_000 + 301).unwrap_err();
    assert!(matches!(err, InternalCallRejection::Unauthorized));
}

#[test]
fn rejects_tampered_signature() {
    let secret = "shared-secret-32-bytes-1234567890";
    let mut h = headers("GET", "/p", b"", 1_000, "U-1", "admin", secret);
    h.insert("x-internal-call-signature", "deadbeef".parse().unwrap());
    let err = verify_internal_call(&h, "GET", "/p", b"", secret, 1_000).unwrap_err();
    assert!(matches!(err, InternalCallRejection::Unauthorized));
}
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo nextest run -p api-gateway internal_call_auth --locked`
Expected: FAIL（`internal_call_auth` 模組不存在 / 編譯錯誤）

- [ ] **Step 3: 實作模組**

`services/api-gateway/src/internal_call_auth.rs` —— 把 `analytics_reporting.rs:216-279`
的 `sign_internal_call` / `constant_time_eq` / `sha256_hex` 邏輯抽到此處（公開），
新增 `verify_internal_call`：

```rust
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// timestamp skew 上限，沿用既有 analytics 驗證的 300 秒（設計 §4.3）。
const TIMESTAMP_SKEW_LIMIT_SECS: i64 = 300;

#[derive(Debug)]
pub enum InternalCallRejection {
    Unauthorized,
}

pub struct VerifiedActor {
    pub actor_id: String,
    pub role: String,
}

pub fn verify_internal_call(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
    secret: &str,
    now: i64,
) -> Result<VerifiedActor, InternalCallRejection> {
    let actor_id = header(headers, "x-actor-id").ok_or(InternalCallRejection::Unauthorized)?;
    let role = header(headers, "x-operator-role").ok_or(InternalCallRejection::Unauthorized)?;
    let timestamp = header(headers, "x-internal-call-timestamp")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or(InternalCallRejection::Unauthorized)?;
    if (now - timestamp).abs() > TIMESTAMP_SKEW_LIMIT_SECS {
        return Err(InternalCallRejection::Unauthorized);
    }
    let signature = header(headers, "x-internal-call-signature")
        .ok_or(InternalCallRejection::Unauthorized)?;
    let expected = sign_internal_call(method, path, body, timestamp, &actor_id, &role, secret);
    if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
        return Err(InternalCallRejection::Unauthorized);
    }
    Ok(VerifiedActor { actor_id, role })
}

pub fn sign_internal_call(
    method: &str, path: &str, body: &[u8], timestamp: i64,
    actor_id: &str, role: &str, secret: &str,
) -> String {
    let body_hash = sha256_hex(body);
    let canonical = format!("{method}\n{path}\n{body_hash}\n{timestamp}\n{actor_id}\n{role}");
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return String::new();
    };
    mac.update(canonical.as_bytes());
    hex_lower(&mac.finalize().into_bytes())
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
}
fn sha256_hex(bytes: &[u8]) -> String { hex_lower(&Sha256::digest(bytes)) }
fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() { return false; }
    left.iter().zip(right).fold(0_u8, |acc, (l, r)| acc | (l ^ r)) == 0
}
```

在 `services/api-gateway/src/lib.rs` 加 `pub mod internal_call_auth;`。

> **Secret 來源**：KYC document 路由用的 secret 須與 admin 端 `internal-call.ts`
> 的 `ADMIN_INTERNAL_CALL_SECRET` 一致 —— Task 8 從 `env::var("ADMIN_INTERNAL_CALL_SECRET")`
> 讀取。既有 `analytics_reporting.rs` 的 `INTERNAL_CALL_HMAC_SECRET` 不動（範圍外）；
> 部署時兩者須設為同一值，此事在 spec §5.3 已註明。

- [ ] **Step 4: 跑測試確認通過**

Run: `cargo nextest run -p api-gateway internal_call_auth --locked`
Expected: PASS（3 個測試）

- [ ] **Step 5: Commit**

```bash
git add services/api-gateway/src/internal_call_auth.rs services/api-gateway/src/lib.rs services/api-gateway/tests/internal_call_auth.rs
git commit -m "feat(api-gateway): add shared internal-call HMAC verification module"
```

---

## Phase 2 — mock-provider 影像 + kyc-service KYC provider document client

### Task 4: mock-provider 提供 mock 文件清單與影像

**Files:**

- Modify: `tools/mocks/provider/src/lib.rs`
- Test: `tools/mocks/provider/src/lib.rs`（`#[cfg(test)]` 區塊）

- [ ] **Step 1: 寫失敗測試**

在 `tools/mocks/provider/src/lib.rs` 的測試區塊新增（用既有測試掛 router 的方式）：

```rust
#[tokio::test]
async fn serves_document_list_and_image() {
    let app = router("test-secret".to_owned());
    // 清單
    let list = app.clone().oneshot(
        axum::http::Request::get("/mock/provider/applicants/applicant_1/documents")
            .body(axum::body::Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(list.status(), axum::http::StatusCode::OK);
    // 影像
    let img = app.oneshot(
        axum::http::Request::get("/mock/provider/applicants/applicant_1/documents/doc_1/image")
            .body(axum::body::Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(img.status(), axum::http::StatusCode::OK);
    assert_eq!(img.headers().get("content-type").unwrap(), "image/png");
}
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo nextest run -p provider-mock serves_document_list_and_image --locked`
（crate 名以 `tools/mocks/provider/Cargo.toml` 的 `name` 為準）
Expected: FAIL（404，路由不存在）

- [ ] **Step 3: 實作 mock 端點**

在 `router()` 加兩條路由（沿用既有 `Router::new().route(...)` 結構）：

```rust
.route(
    "/mock/provider/applicants/:applicant_id/documents",
    get(mock_document_list),
)
.route(
    "/mock/provider/applicants/:applicant_id/documents/:doc_id/image",
    get(mock_document_image),
)
```

handler：清單回固定 2 筆 mock 文件（id_doc / selfie），影像回一張寫死的 1x1 PNG
bytes（`Content-Type: image/png`）：

```rust
async fn mock_document_list(Path(applicant_id): Path<String>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "applicant_id": applicant_id,
        "documents": [
            { "doc_id": "doc_1", "kind": "id_doc", "country": "TWN", "side": "front",
              "content_type": "image/png", "captured_at": "2026-05-10T00:00:00Z" },
            { "doc_id": "doc_2", "kind": "selfie", "country": null, "side": null,
              "content_type": "image/png", "captured_at": "2026-05-10T00:01:00Z" }
        ]
    }))
}

const MOCK_PNG: &[u8] = &[
    0x89,0x50,0x4E,0x47,0x0D,0x0A,0x1A,0x0A,0x00,0x00,0x00,0x0D,0x49,0x48,0x44,0x52,
    0x00,0x00,0x00,0x01,0x00,0x00,0x00,0x01,0x08,0x06,0x00,0x00,0x00,0x1F,0x15,0xC4,
    0x89,0x00,0x00,0x00,0x0A,0x49,0x44,0x41,0x54,0x78,0x9C,0x63,0x00,0x01,0x00,0x00,
    0x05,0x00,0x01,0x0D,0x0A,0x2D,0xB4,0x00,0x00,0x00,0x00,0x49,0x45,0x4E,0x44,0xAE,
    0x42,0x60,0x82,
];

async fn mock_document_image(Path((_applicant, _doc)): Path<(String, String)>) -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "image/png")], MOCK_PNG)
}
```

- [ ] **Step 4: 跑測試確認通過**

Run: `cargo nextest run -p provider-mock serves_document_list_and_image --locked`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tools/mocks/provider/src/lib.rs
git commit -m "feat(mock-provider): serve mock KYC document list and image endpoints"
```

### Task 5: kyc-service KYC provider document client

**Files:**

- Create: `services/kyc-service/src/provider_documents.rs`
- Modify: `services/kyc-service/src/lib.rs`（加 `mod provider_documents;`）
- Test: `services/kyc-service/tests/provider_documents.rs`

- [ ] **Step 1: 寫失敗測試**

`services/kyc-service/tests/provider_documents.rs` —— 用 `wiremock` 或既有 mock 模式
起一個假伺服器，驗證 client 能取清單並下載影像 bytes：

```rust
use kyc_service::provider_documents::{ProviderDocumentClient, ProviderDocument};

#[tokio::test]
async fn lists_documents_for_applicant() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/mock/provider/applicants/applicant_1/documents"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "applicant_id": "applicant_1",
            "documents": [{ "doc_id": "doc_1", "kind": "id_doc", "country": "TWN",
                "side": "front", "content_type": "image/png", "captured_at": "2026-05-10T00:00:00Z" }]
        })))
        .mount(&server).await;

    let client = ProviderDocumentClient::new(server.uri(), "app-token".into(), "secret-key".into());
    let docs: Vec<ProviderDocument> = client.list_documents("applicant_1").await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].doc_id, "doc_1");
    assert_eq!(docs[0].content_type, "image/png");
}
```

（若 workspace 尚無 `wiremock` dev-dependency，於 `services/kyc-service/Cargo.toml`
的 `[dev-dependencies]` 加入；版本對齊 workspace 其他用法，無則用最新穩定。）

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo nextest run -p kyc-service provider_documents --locked`
Expected: FAIL（模組不存在）

- [ ] **Step 3: 實作 client**

`services/kyc-service/src/provider_documents.rs`。`ProviderDocument` struct 對應 mock
清單欄位（`doc_id`、`kind`、`country: Option<String>`、`side: Option<String>`、
`content_type`、`captured_at`）。`ProviderDocumentClient`：

- `new(base_url, app_token, secret_key)`
- `list_documents(&self, applicant_id) -> Result<Vec<ProviderDocument>, ProviderDocumentError>`
  → `GET {base_url}/mock/provider/applicants/{applicant_id}/documents`
- `fetch_image(&self, applicant_id, doc_id) -> Result<ProviderImage, ProviderDocumentError>`
  → `GET .../documents/{doc_id}/image`；`ProviderImage { content_type: String, body: reqwest::Response }`
  **保留 `reqwest::Response` 以便上層 streaming**，不在此處 `bytes()` 整包讀取。
- `ProviderDocumentError` 區分 `NotFound` / `ProviderAuth`（HTTP 401/403）/ `Upstream`
  （其他）。**錯誤訊息不得含 KYC provider URL**（設計 §7、§8 不變式 1）。
- app-token 簽章 header 真實接 KYC provider 時才需要；mock 階段帶上 `X-App-Token` 即可，
  簽章函式留介面、以憑證在則加 header 實作。

- [ ] **Step 4: 跑測試確認通過**

Run: `cargo nextest run -p kyc-service provider_documents --locked`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add services/kyc-service/src/provider_documents.rs services/kyc-service/src/lib.rs services/kyc-service/Cargo.toml services/kyc-service/tests/provider_documents.rs
git commit -m "feat(kyc-service): add KYC provider document client (list + streaming image fetch)"
```

---

## Phase 3 — kyc-service：稽核表 + 內部 admin document 端點

### Task 6: `kyc_document_access_log` 遷移

**Files:**

- Create: `services/kyc-service/migrations/202605190001_create_kyc_document_access_log.sql`

- [ ] **Step 1: 寫遷移 SQL**

```sql
-- SPEC-005 §5.3: KYC document access audit log.
CREATE TABLE IF NOT EXISTS kyc_document_access_log (
    id             BIGSERIAL PRIMARY KEY,
    reviewer_id    TEXT NOT NULL,
    reviewer_role  TEXT NOT NULL,
    user_id        TEXT NOT NULL,
    doc_id         TEXT,
    action         TEXT NOT NULL CHECK (action IN ('list', 'view')),
    result         TEXT NOT NULL CHECK (result IN ('success', 'denied')),
    denial_reason  TEXT,
    request_id     TEXT,
    source_ip_hash TEXT,
    accessed_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_kyc_doc_access_user
    ON kyc_document_access_log (user_id, accessed_at DESC);
```

- [ ] **Step 2: 套用遷移並驗證**

Run: `make migrate`（或 kyc-service 啟動時自動跑；遷移記入 `_sqlx_migrations_kyc_service`）
Expected: 無錯誤；`kyc_document_access_log` 表建立。

- [ ] **Step 3: Commit**

```bash
git add services/kyc-service/migrations/202605190001_create_kyc_document_access_log.sql
git commit -m "feat(kyc-service): add kyc_document_access_log migration"
```

### Task 7: kyc-service 文件清單 + 影像端點

**Files:**

- Modify: `services/kyc-service/src/kyc.rs`（`KycStore` 加方法）
- Modify: `services/kyc-service/src/api.rs`（2 路由 + 2 handler）
- Test: `services/kyc-service/tests/kyc_documents_api.rs`

- [ ] **Step 1: 寫失敗測試**

`services/kyc-service/tests/kyc_documents_api.rs`，用 in-memory store + `mock-provider`
（或 wiremock）掛 `api::router`，覆蓋設計 §10 的 kyc-service 項目：

```rust
// 1. 已有 Tier2 record（provider_applicant_id = Some）→ 清單回 200 + 文件陣列
// 2. 影像端點以合法 (user_id, doc_id) → 200 + image/png bytes
// 3. doc_id 不屬於該 user_id → 404；body 與「doc 不存在」完全相同
// 4. user_id 無 KYC record / 無 applicant → 清單回空、影像回 404
// 5. 每次成功 list/view 在 kyc_document_access_log 寫一列 result='success'
// 6. doc 綁定失敗寫一列 result='denied', denial_reason 設定
```

每個情境一個 `#[tokio::test]`，斷言 HTTP status、body、以及 store 中 access log 列數。

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo nextest run -p kyc-service kyc_documents_api --locked`
Expected: FAIL（路由不存在）

- [ ] **Step 3: `KycStore` 加方法**

`services/kyc-service/src/kyc.rs` 的 `KycStore` trait 新增：

```rust
/// 解析會員當前 KYC record 的 KYC provider applicant id。無 record / 無 applicant 回 None。
async fn applicant_id_for_user(&self, user_id: &str) -> Result<Option<String>, KycStoreError>;

/// 寫入一列文件存取稽核。
async fn record_document_access(
    &self,
    entry: DocumentAccessEntry,
) -> Result<(), KycStoreError>;
```

`DocumentAccessEntry` struct 欄位對應 Task 6 表（`reviewer_id`、`reviewer_role`、
`user_id`、`doc_id: Option<String>`、`action`、`result`、`denial_reason: Option<String>`、
`request_id: Option<String>`、`source_ip_hash: Option<String>`）。

`applicant_id_for_user` 實作：呼叫既有 `current_record(user_id)`，取
`record.provider_applicant_id`。in-memory 與 Postgres 兩個 store impl 都要實作。
Postgres 端的 `record_document_access` 為 `INSERT INTO kyc_document_access_log ...`。

- [ ] **Step 4: api.rs 加路由 + handler**

`services/kyc-service/src/api.rs` 的 `router()` 加：

```rust
.route(
    "/v1/internal/admin/kyc/members/:user_id/documents",
    get(list_member_documents::<S>),
)
.route(
    "/v1/internal/admin/kyc/members/:user_id/documents/:doc_id",
    get(get_member_document::<S>),
)
```

`ApiState` 加一個 `provider_documents: ProviderDocumentClient` 欄位（由 `router()`
的 `secrets` / 設定建構）。

`list_member_documents`：

1. `applicant_id_for_user(user_id)` → 無則回 `Json([])`、寫 `success`/`list` 稽核。
2. 有則 `provider_documents.list_documents(applicant_id)`。
3. 回 `Json<Vec<ProviderDocument>>`（Task 5 已定義且 `Serialize`，其欄位即對外
   DTO；`doc_id` 直接用 KYC provider doc id —— 本 MVP doc id 即 opaque 對外 id，
   因其本就不是可猜的 vendor URL，且下載端會重驗綁定）。
4. 寫 `kyc_document_access_log`（`action='list'`, `result='success'`）。

`get_member_document`：

1. `applicant_id_for_user(user_id)`；無 applicant → 寫 `denied`/`view`
   (`denial_reason='no_applicant'`)、回 **404**。
2. `provider_documents.list_documents` 確認 `doc_id` ∈ 該 applicant 的文件集；
   不在 → 寫 `denied`/`view`（`denial_reason='doc_not_bound'`）、回 **404**
   （與步驟 1 的 404 body 完全相同 —— 設計不變式 3）。
3. `provider_documents.fetch_image(applicant_id, doc_id)` → 用
   `axum::body::Body::from_stream(response.bytes_stream())` **streaming** 回傳，
   `Content-Type` 取自影像、限白名單 `image/jpeg|png|webp`，加
   `Cache-Control: private, no-store`。**不得 `bytes().await` 整包讀取。**
4. KYC provider 401/403 → 記 provider auth error、回通用 502/「文件暫時無法載入」，
   不外露 vendor code。
5. 成功寫 `kyc_document_access_log`（`action='view'`, `result='success'`）。

`request_id` / `reviewer_id` / `reviewer_role` 從既有 `x-request-id` /
`x-actor-id` / `x-operator-role` header 取（api-gateway 已轉發，見 Task 8）。

- [ ] **Step 5: 跑測試 + 重建 SQLx 中繼資料**

Run: `cargo sqlx prepare -p kyc-service --workspace -- --tests`（新增 query 後）
Run: `cargo nextest run -p kyc-service kyc_documents_api --locked`
Expected: PASS（6 情境全過）

- [ ] **Step 6: Commit**

```bash
git add services/kyc-service/src/kyc.rs services/kyc-service/src/api.rs services/kyc-service/tests/kyc_documents_api.rs .sqlx/
git commit -m "feat(kyc-service): add internal admin KYC document list + streaming image endpoints"
```

---

## Phase 4 — api-gateway：KYC document streaming proxy

### Task 8: gateway KYC document 路由（驗章 + streaming）

**Files:**

- Modify: `services/api-gateway/src/admin_ops.rs`
- Test: `services/api-gateway/tests/kyc_documents_proxy.rs`

- [ ] **Step 1: 寫失敗測試**

`services/api-gateway/tests/kyc_documents_proxy.rs`：

```rust
// 1. 無 / 錯誤 signed call header → 401（用 Task 3 的 verify_internal_call）
// 2. 合法 signed call + 假 kyc-service（wiremock）回影像 → gateway 回 200 + image bytes
// 3. gateway 不 buffer：對大 body 確認以 streaming 轉發（可斷言 Transfer-Encoding
//    或以 chunk 計數驗證；最低限度斷言 body 內容正確且 handler 未呼叫整包讀取）
// 4. kyc-service 回 404 → gateway 透傳 404
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo nextest run -p api-gateway kyc_documents_proxy --locked`
Expected: FAIL（路由不存在）

- [ ] **Step 3: 實作 gateway 路由**

`admin_ops.rs` 的 admin Router 新增：

```rust
.route(
    "/api/internal/admin/members/:user_id/kyc-documents",
    get(proxy_kyc_document_list::<S>),
)
.route(
    "/api/internal/admin/members/:user_id/kyc-documents/:doc_id",
    get(proxy_kyc_document_image::<S>),
)
```

兩個 handler 共同流程：

1. 取 `secret = env::var("ADMIN_INTERNAL_CALL_SECRET")`；缺失 → 500
   （production fail-fast）。
2. `internal_call_auth::verify_internal_call(&headers, method, path, b"", &secret, now)`
   —— 失敗回 **401**。`path` 用不含 query 的請求路徑（與 admin 簽章端一致）。
   若 api-gateway 已有安全稽核 channel，此 401 denial 應留痕（設計 §6.3）；
   若無則列範圍外，於 PR 描述標明。
3. 驗章通過後，用 `reqwest` 轉發至
   `{kyc_service_url}/v1/internal/admin/kyc/members/{user_id}/documents[/{doc_id}]`，
   透傳 `x-actor-id` / `x-operator-role` / `x-request-id`（沿用 `admin_proxy_headers`
   模式，讓 kyc-service 稽核能取得 reviewer 身分）。
4. **streaming 回傳**：用 `reqwest::Response::bytes_stream()` →
   `axum::body::Body::from_stream(...)`，透傳 status 與 `Content-Type`。
   **禁止 `response.bytes().await` 整包讀取。**
5. 影像 handler 透傳 `Cache-Control: private, no-store`；清單 handler 回 JSON。

`now` 用既有時間來源（與 `analytics_reporting.rs` 的 `config.now()` 一致；
無則 `chrono::Utc::now().timestamp()`）。

- [ ] **Step 4: 跑測試確認通過**

Run: `cargo nextest run -p api-gateway kyc_documents_proxy --locked`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add services/api-gateway/src/admin_ops.rs services/api-gateway/tests/kyc_documents_proxy.rs
git commit -m "feat(api-gateway): add KYC document proxy routes with signed-call verification and streaming"
```

---

## Phase 5 — admin app：BFF + route handlers

### Task 9: BFF streaming 代理 helper + 型別

**Files:**

- Create: `apps/admin/lib/kyc-documents-bff.ts`
- Modify: `apps/admin/lib/member-management-shared.ts`
- Test: `apps/admin/tests/kyc-documents-bff.test.ts`

- [ ] **Step 1: 加共享型別**

`apps/admin/lib/member-management-shared.ts` 新增：

```typescript
export interface KycDocument {
  doc_id: string;
  kind: "id_doc" | "selfie" | "liveness";
  country: string | null;
  side: string | null;
  content_type: string;
  captured_at: string;
}

export interface KycDocumentList {
  documents: KycDocument[];
}
```

- [ ] **Step 2: 寫失敗測試**

`apps/admin/tests/kyc-documents-bff.test.ts`（`node:test`，import 用 `.js` 副檔名）：

```typescript
import assert from "node:assert/strict";
import { test } from "node:test";
import { proxyKycDocumentRequest } from "../lib/kyc-documents-bff.js";

test("proxyKycDocumentRequest streams body without buffering", async () => {
  const stream = new ReadableStream({
    start(c) {
      c.enqueue(new Uint8Array([1, 2, 3]));
      c.close();
    },
  });
  const fetchImpl = async () =>
    new Response(stream, {
      status: 200,
      headers: { "content-type": "image/png" },
    });
  const res = await proxyKycDocumentRequest(
    { sub: "U-ADMIN", role: "admin" } as never,
    {
      userId: "U-1",
      adminPath: "/api/internal/admin/members/U-1/kyc-documents/doc_1",
    },
    {
      gatewayUrl: "https://gw.test",
      secret: "shared-secret-32-bytes-1234567890",
      fetchImpl,
    },
  );
  assert.equal(res.status, 200);
  assert.equal(res.headers.get("content-type"), "image/png");
  // body 為 stream 透傳：res.body 應為 ReadableStream（非預先 buffer 的結果）
  assert.ok(res.body instanceof ReadableStream);
});
```

- [ ] **Step 3: 跑測試確認失敗**

Run: `pnpm --filter @prophetica/admin test`
Expected: FAIL（`kyc-documents-bff` 模組不存在）

- [ ] **Step 4: 實作 BFF helper**

`apps/admin/lib/kyc-documents-bff.ts` —— 參考 `member-management-bff.ts` 的
`proxyMemberGatewayRequest`，但**關鍵差異：不要 `await upstream.arrayBuffer()`**，
直接把 `upstream.body`（`ReadableStream`）透傳給新的 `Response`：

```typescript
import "server-only";
import { signInternalCall } from "./internal-call.js";
import type { SessionPayload } from "./session-crypto.js";

interface ProxyInput {
  userId: string;
  adminPath: string;
}
interface ProxyDeps {
  gatewayUrl?: string;
  secret?: string;
  fetchImpl?: typeof fetch;
}

const IMAGE_CONTENT_TYPES = new Set(["image/jpeg", "image/png", "image/webp"]);

export async function proxyKycDocumentRequest(
  viewer: SessionPayload,
  input: ProxyInput,
  deps: ProxyDeps = {},
): Promise<Response> {
  const gatewayUrl = deps.gatewayUrl ?? process.env.API_GATEWAY_URL;
  if (!gatewayUrl) {
    return Response.json({ error: "gateway_unconfigured" }, { status: 500 });
  }
  const signaturePath = input.adminPath.split("?")[0] ?? input.adminPath;
  const headers = signInternalCall({
    method: "GET",
    path: signaturePath,
    body: "",
    actorId: viewer.sub,
    role: viewer.role,
    secret: deps.secret,
  });
  const doFetch = deps.fetchImpl ?? fetch;
  let upstream: Response;
  try {
    upstream = await doFetch(new URL(input.adminPath, gatewayUrl), {
      method: "GET",
      headers: { ...headers, "x-request-id": crypto.randomUUID() },
      cache: "no-store",
    });
  } catch {
    return Response.json({ error: "gateway_unavailable" }, { status: 502 });
  }

  const contentType =
    upstream.headers.get("content-type") ?? "application/octet-stream";
  const responseHeaders = new Headers();
  responseHeaders.set("content-type", contentType);
  responseHeaders.set("cache-control", "private, no-store");
  responseHeaders.set("pragma", "no-cache");
  responseHeaders.set("x-content-type-options", "nosniff");

  // 影像回應強制白名單；非影像（JSON 清單 / 錯誤）原樣透傳。
  if (
    contentType.startsWith("image/") &&
    !IMAGE_CONTENT_TYPES.has(contentType)
  ) {
    return Response.json({ error: "unsupported_media_type" }, { status: 502 });
  }
  // 關鍵：streaming 透傳，不 buffer。
  return new Response(upstream.body, {
    status: upstream.status,
    headers: responseHeaders,
  });
}
```

- [ ] **Step 5: 跑測試確認通過**

Run: `pnpm --filter @prophetica/admin test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add apps/admin/lib/kyc-documents-bff.ts apps/admin/lib/member-management-shared.ts apps/admin/tests/kyc-documents-bff.test.ts
git commit -m "feat(admin): add streaming KYC document BFF proxy helper"
```

### Task 10: admin route handlers

**Files:**

- Create: `apps/admin/app/api/members/[userId]/kyc-documents/route.ts`
- Create: `apps/admin/app/api/members/[userId]/kyc-documents/[docId]/route.ts`

- [ ] **Step 1: 文件清單 route**

`apps/admin/app/api/members/[userId]/kyc-documents/route.ts`：

```typescript
import { getViewer } from "@/lib/session";
import { proxyKycDocumentRequest } from "@/lib/kyc-documents-bff";

interface RouteContext {
  params: Promise<{ userId: string }>;
}

export async function GET(_request: Request, context: RouteContext) {
  const viewer = await getViewer();
  if (!viewer || (viewer.role !== "admin" && viewer.role !== "manager")) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
  const { userId } = await context.params;
  const enc = encodeURIComponent(userId);
  return proxyKycDocumentRequest(viewer, {
    userId,
    adminPath: `/api/internal/admin/members/${enc}/kyc-documents`,
  });
}
```

- [ ] **Step 2: 影像 route**

`apps/admin/app/api/members/[userId]/kyc-documents/[docId]/route.ts`：

```typescript
import { getViewer } from "@/lib/session";
import { proxyKycDocumentRequest } from "@/lib/kyc-documents-bff";

interface RouteContext {
  params: Promise<{ userId: string; docId: string }>;
}

export async function GET(_request: Request, context: RouteContext) {
  const viewer = await getViewer();
  if (!viewer || (viewer.role !== "admin" && viewer.role !== "manager")) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
  const { userId, docId } = await context.params;
  const enc = `${encodeURIComponent(userId)}/kyc-documents/${encodeURIComponent(docId)}`;
  return proxyKycDocumentRequest(viewer, {
    userId,
    adminPath: `/api/internal/admin/members/${enc}`,
  });
}
```

> session 失效 / 角色不符的拒絕在此層回 403。設計 §6.3 要求此類拒絕在 admin
> 層留痕 —— 若 `apps/admin` 已有安全稽核 channel 則接上；若無，於 spec §5.3
> 待辦清單補一筆並在 PR 描述標明（不在此私自新建稽核基建）。

- [ ] **Step 3: 跑 lint（typecheck）**

Run: `pnpm --filter @prophetica/admin lint`
Expected: PASS（`tsc` 無錯誤）

- [ ] **Step 4: Commit**

```bash
git add apps/admin/app/api/members/
git commit -m "feat(admin): add KYC document list and image route handlers"
```

---

## Phase 6 — admin UI：證件 gallery

### Task 11: gallery 元件取代佔位卡片

**Files:**

- Create: `apps/admin/components/kyc-document-gallery.tsx`
- Modify: `apps/admin/components/member-management.tsx`（取代 §1311-1316 佔位卡片）

- [ ] **Step 1: 實作 gallery 元件**

`apps/admin/components/kyc-document-gallery.tsx` —— client component，props `{ userId: string }`：

1. mount 時 `fetch('/api/members/{userId}/kyc-documents')` 取清單（`cache: 'no-store'`）。
2. **先有清單才 render `<img>`**（設計 §4.5）。清單失敗顯示「文件暫時無法載入」，
   空清單顯示「尚無文件」。
3. 每筆文件 render `<img src="/api/members/{userId}/kyc-documents/{docId}">`，
   附 `onError` handler 顯示「文件暫時無法載入」佔位，不留破圖。
4. 顯示 `kind` 中文標籤（證件 / 自拍 / Liveness）與 `captured_at`。
5. 沿用既有 admin 元件樣式（`InfoCard` / Tailwind class 慣例）。

```tsx
"use client";
import { useEffect, useState } from "react";
import type { KycDocument } from "@/lib/member-management-shared";

const KIND_LABEL: Record<KycDocument["kind"], string> = {
  id_doc: "證件",
  selfie: "自拍",
  liveness: "Liveness",
};

export function KycDocumentGallery({ userId }: { userId: string }) {
  const [docs, setDocs] = useState<KycDocument[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    fetch(`/api/members/${encodeURIComponent(userId)}/kyc-documents`, {
      cache: "no-store",
    })
      .then(async (r) => {
        if (!r.ok) throw new Error("load_failed");
        return (await r.json()) as KycDocument[];
      })
      .then((d) => active && setDocs(d))
      .catch(() => active && setError("文件暫時無法載入。"));
    return () => {
      active = false;
    };
  }, [userId]);

  if (error)
    return <p className="text-[14px] text-proph-ink-muted-80">{error}</p>;
  if (!docs)
    return <p className="text-[14px] text-proph-ink-muted-80">載入中…</p>;
  if (docs.length === 0)
    return <p className="text-[14px] text-proph-ink-muted-80">尚無文件。</p>;

  return (
    <div className="grid grid-cols-2 gap-3">
      {docs.map((doc) => (
        <figure key={doc.doc_id} className="space-y-1">
          <img
            src={`/api/members/${encodeURIComponent(userId)}/kyc-documents/${encodeURIComponent(doc.doc_id)}`}
            alt={KIND_LABEL[doc.kind]}
            className="w-full rounded-proph-md border border-proph-hairline"
            onError={(e) => {
              e.currentTarget.replaceWith(
                Object.assign(document.createElement("p"), {
                  className: "text-[13px] text-proph-ink-muted-80",
                  textContent: "文件暫時無法載入。",
                }),
              );
            }}
          />
          <figcaption className="text-[12px] text-proph-ink-muted-80">
            {KIND_LABEL[doc.kind]} · {doc.captured_at}
          </figcaption>
        </figure>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: 取代佔位卡片**

`apps/admin/components/member-management.tsx` 的 `MemberOverview` 區（§1311-1316
附近，即 `<InfoCard title="KYC 證件材料">` 區塊），改為：

```tsx
<InfoCard title="KYC 證件材料">
  <KycDocumentGallery userId={summary.member.user_id} />
</InfoCard>
```

（`summary.member` 的 user id 欄位名以 `MemberIdentitySummary` 實際定義為準；
若非 `user_id` 則對應之。）在檔案頂部 import `KycDocumentGallery`。

- [ ] **Step 3: 跑 lint**

Run: `pnpm --filter @prophetica/admin lint`
Expected: PASS

- [ ] **Step 4: 手動驗證（dev）**

Run: `make dev-infra && make dev`，登入 admin（`admin@prophetica.local` / `admin`），
進「會員管理 → KYC 審核」，選一個有 Tier 2 record 的會員，確認證件卡片顯示
mock-provider 的 mock 影像；開 DevTools Network 確認 `/api/members/.../kyc-documents/...`
回 `image/png`、`cache-control: private, no-store`。

- [ ] **Step 5: Commit**

```bash
git add apps/admin/components/kyc-document-gallery.tsx apps/admin/components/member-management.tsx
git commit -m "feat(admin): replace KYC placeholder card with live document gallery"
```

---

## 驗收條件（(A) 完成定義）

- admin「KYC 審核」頁可顯示 KYC provider（mock-provider）收集的證件 / 自拍影像。
- 影像鏈路 `admin → api-gateway → kyc-service → KYC provider` 全程 streaming，無 buffer。
- api-gateway 對 KYC document 路由驗 signed call + timestamp skew。
- `doc_id` 不屬於 `user_id` 一律單一 404，不洩存在性。
- 每次 list/view（成功與綁定拒絕）在 `kyc_document_access_log` 留痕。
- 影像回應帶 `private, no-store` + `nosniff`，content-type 限白名單。
- KYC provider URL / vendor path 不外露至 api-gateway / admin / 瀏覽器。
- `cargo nextest run -p kyc-service`、`-p api-gateway`、`pnpm --filter @prophetica/admin lint test` 全綠。

## 範圍外（後續任務）

- (B) 每日 R2 歸檔批次（spec §5.4 已定義，實作另開計畫）。
- 既有 admin ops 端點全面補 signed-call 驗證（設計 §13，建議另開 SPEC-010 finding）。
- 真實 KYC provider 憑證接線（SPEC-000 §5 Q-9）。
- `kyc_document:read` 獨立 permission（設計 §6.1 長期項）。
- admin / api-gateway 安全稽核 channel 若不存在的建置（設計 §6.3）。
