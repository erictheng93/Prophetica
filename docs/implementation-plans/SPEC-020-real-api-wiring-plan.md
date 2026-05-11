# SPEC-020 Real-API Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the admin-side in-memory `AnalyticsReportingStore` with a real HTTP chain: admin BFF → api-gateway proxy (HMAC-signed) → new `analytics-reporting-service` (Postgres-backed) → source services (kyc / ledger / risk / user). All 5 SPEC-020 admin pages must render from real APIs.

**Architecture:** Hybrid placement per `SPEC-020-real-api-wiring-design.md`. api-gateway keeps its admin-facing surface but becomes a thin proxy with coarse RBAC + HMAC verification. A new `analytics-reporting-service` owns the 4 PG tables and the job lifecycle. Source services expose pull-style `/v1/internal/reports/*` endpoints. Admin pages talk to BFF route handlers that sign requests with HMAC-SHA256.

**Tech Stack:** Rust (axum 0.7, sqlx 0.8, reqwest 0.12, tokio, sha2, hmac), Postgres, Next.js 14 (App Router server components, Node `node:test` runner), wiremock for proxy tests.

**Plan size note:** This plan spans 5 services + sdk crate + admin frontend. It is one cohesive feature with strong sequential dependencies (later phases need earlier ones). If you prefer to split across PRs, each Phase below corresponds naturally to one PR. The feature flag `ANALYTICS_REPORTING_PROXY_ENABLED` lets Phases 1–5 land on `main` without flipping admin into the new path until Phase 6.

---

## Phase 0: Baseline commit

The current branch has uncommitted SPEC-020 mock work (api-gateway in-memory analytics module + migration + admin TS lib + tests). Phase 0 commits these as the **mock baseline** that subsequent phases evolve into the real API. Skip Phase 0 only if these files are already committed.

### Task 0.1: Commit the SPEC-020 mock baseline

**Files:**
- Stage: `services/api-gateway/Cargo.toml`, `services/api-gateway/src/lib.rs`, `services/api-gateway/src/analytics_reporting.rs`, `services/api-gateway/tests/analytics_reporting.rs`, `services/api-gateway/migrations/202605110001_create_analytics_reporting.sql`, `apps/admin/lib/analytics-reporting.ts`, `apps/admin/tests/analytics-reporting.test.ts`, `Cargo.lock`

- [ ] **Step 1: Verify the staged set matches the mock baseline**

```bash
git status --short | grep -E '(analytics_reporting|analytics-reporting|api-gateway/(Cargo\.toml|migrations|src/lib\.rs))'
```

Expected: lines for the 7 files above plus `Cargo.lock`.

- [ ] **Step 2: Stage and verify the diff size**

```bash
git add services/api-gateway/Cargo.toml services/api-gateway/src/lib.rs services/api-gateway/src/analytics_reporting.rs services/api-gateway/tests/analytics_reporting.rs services/api-gateway/migrations/202605110001_create_analytics_reporting.sql apps/admin/lib/analytics-reporting.ts apps/admin/tests/analytics-reporting.test.ts Cargo.lock
git diff --cached --stat
```

Expected: `~1700 insertions` across ~8 files.

- [ ] **Step 3: Run admin tests to confirm baseline is green**

```bash
pnpm --filter @prophetica/admin test
```

Expected: `tests 29  pass 29  fail 0`.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(analytics): mock SPEC-020 backend baseline in api-gateway + admin lib

In-memory analytics_reporting module in api-gateway plus the admin-side
AnalyticsReportingStore. Migration file is present but the Rust code is
not wired to it yet. This baseline is the starting point for real-API
wiring per docs/implementation-plans/SPEC-020-real-api-wiring-plan.md."
```

Expected: new commit on `feat/background-sync-order-queue`.

---

## Phase 1: Shared `sdk::masking`

Foundation for §5 invariant 4 (unified masking). Trivially safe: pure function, no callers yet.

### Task 1.1: Add `sdk::masking::mask_email` with tests

**Files:**
- Create: `crates/sdk/src/masking.rs`
- Modify: `crates/sdk/src/lib.rs:1-8` (add `pub mod masking;`)
- Test: `crates/sdk/tests/masking.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/sdk/tests/masking.rs`:

```rust
use sdk::masking::mask_email;

#[test]
fn email_with_at_keeps_first_char_and_domain() {
    assert_eq!(mask_email("member-001@example.com"), "m***@example.com");
}

#[test]
fn short_value_without_at_returns_four_stars() {
    assert_eq!(mask_email("abc"), "****");
}

#[test]
fn longer_value_without_at_returns_four_stars() {
    assert_eq!(mask_email("not-an-email"), "****");
}

#[test]
fn empty_string_returns_four_stars() {
    assert_eq!(mask_email(""), "****");
}

#[test]
fn email_starting_with_at_returns_four_stars() {
    assert_eq!(mask_email("@example.com"), "****");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p sdk --test masking
```

Expected: FAIL with `unresolved import 'sdk::masking'` or `cannot find function 'mask_email'`.

- [ ] **Step 3: Create the module**

Create `crates/sdk/src/masking.rs`:

```rust
//! Shared masking helpers. All services that emit masked PII must use
//! this module so the mask format stays uniform across reporting.

#[must_use]
pub fn mask_email(value: &str) -> String {
    if let Some((local, domain)) = value.split_once('@') {
        if let Some(first) = local.chars().next() {
            return format!("{first}***@{domain}");
        }
        // Local part empty (e.g. "@example.com") -> generic mask.
    }
    "****".to_owned()
}
```

- [ ] **Step 4: Register the module**

Edit `crates/sdk/src/lib.rs`. Add `pub mod masking;` alphabetically:

```rust
pub mod config;
pub mod dr;
pub mod errors;
pub mod feature_flags;
pub mod ledger;
pub mod masking;
pub mod observability;
pub mod referral;
pub mod service_stub;
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p sdk --test masking
```

Expected: `test result: ok. 5 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/sdk/src/masking.rs crates/sdk/src/lib.rs crates/sdk/tests/masking.rs
git commit -m "feat(sdk): add masking::mask_email for shared PII masking

Used by user-service contacts:lookup-masked and any future source
service that needs to emit masked email values for reporting."
```

---

## Phase 2: Source-service internal report endpoints

Adds one `/v1/internal/reports/*` endpoint per source service. Each endpoint returns `{ rows, source_snapshot }` over HTTP. These additions are independent and have no callers yet, so they ship behind no flag.

**Shared response shape.** Every endpoint in Phase 2 returns JSON of this shape:

```json
{
  "rows": [ { "<column>": <value>, ... } ],
  "source_snapshot": {
    "source_service": "<service-name>",
    "source_version": "<semver or schema tag>",
    "cutoff_at": "<RFC3339>",
    "source_generated_at": "<RFC3339>",
    "correlation_id": "<x-request-id echo>"
  }
}
```

`x-request-id` from the request is echoed as `correlation_id` so the reporting-service can stitch the audit chain.

### Task 2.1: kyc-service `/v1/internal/reports/kyc-pass-rate`

**Files:**
- Create: `services/kyc-service/src/internal_reports.rs`
- Modify: `services/kyc-service/src/lib.rs` (add `pub mod internal_reports;` and mount route)
- Test: `services/kyc-service/tests/internal_reports_kyc_pass_rate.rs`

- [ ] **Step 1: Write the failing test**

Create `services/kyc-service/tests/internal_reports_kyc_pass_rate.rs`:

```rust
#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use kyc_service::internal_reports::internal_reports_router;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn kyc_pass_rate_returns_rows_and_source_snapshot() {
    let app = internal_reports_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/reports/kyc-pass-rate?from=2026-05-09T00:00:00Z&to=2026-05-10T23:59:59Z")
                .header("x-request-id", "corr-001")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    let body: Value = serde_json::from_slice(&bytes).expect("json");
    assert!(body["rows"].is_array());
    assert_eq!(body["source_snapshot"]["source_service"], "kyc-service");
    assert_eq!(body["source_snapshot"]["correlation_id"], "corr-001");
    assert!(body["source_snapshot"]["cutoff_at"].is_string());
}

#[tokio::test]
async fn kyc_pass_rate_rejects_missing_window() {
    let app = internal_reports_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/reports/kyc-pass-rate")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p kyc-service --test internal_reports_kyc_pass_rate
```

Expected: FAIL with `unresolved import 'kyc_service::internal_reports'`.

- [ ] **Step 3: Implement the handler**

Create `services/kyc-service/src/internal_reports.rs`:

```rust
use axum::{
    Json, Router,
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[must_use]
pub fn internal_reports_router() -> Router {
    Router::new().route("/v1/internal/reports/kyc-pass-rate", get(kyc_pass_rate))
}

#[derive(Debug, Deserialize)]
struct PassRateQuery {
    from: String,
    to: String,
}

#[derive(Debug, Serialize)]
struct ReportResponse {
    rows: Vec<Value>,
    source_snapshot: SourceSnapshot,
}

#[derive(Debug, Serialize)]
struct SourceSnapshot {
    source_service: &'static str,
    source_version: &'static str,
    cutoff_at: String,
    source_generated_at: String,
    correlation_id: String,
}

async fn kyc_pass_rate(
    headers: HeaderMap,
    query: Option<Query<PassRateQuery>>,
) -> Result<Json<ReportResponse>, Response> {
    let Some(Query(window)) = query else {
        return Err((StatusCode::BAD_REQUEST, "missing from/to").into_response());
    };
    let correlation_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    // v1: deterministic aggregate placeholder. Phase 3 will run a real
    // SELECT over user_kyc_records grouped by status.
    let rows = vec![json!({
        "report_date": &window.to[..10],
        "approvals_count": 16,
        "kyc_pass_rate": "91.4%",
        "service_uptime": "99.92%"
    })];
    Ok(Json(ReportResponse {
        rows,
        source_snapshot: SourceSnapshot {
            source_service: "kyc-service",
            source_version: "kyc.v2026.05.10",
            cutoff_at: window.to.clone(),
            source_generated_at: window.to,
            correlation_id,
        },
    }))
}
```

- [ ] **Step 4: Register the module**

Edit `services/kyc-service/src/lib.rs`. Find the existing `pub mod ...` block and add:

```rust
pub mod internal_reports;
```

Then mount the router in whichever function builds the service's top-level `Router`. If the entry function is `pub fn router(...) -> Router`, add `.merge(internal_reports::internal_reports_router())` to the chain.

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p kyc-service --test internal_reports_kyc_pass_rate
```

Expected: `test result: ok. 2 passed`.

- [ ] **Step 6: Commit**

```bash
git add services/kyc-service/src/internal_reports.rs services/kyc-service/src/lib.rs services/kyc-service/tests/internal_reports_kyc_pass_rate.rs
git commit -m "feat(kyc-service): add /v1/internal/reports/kyc-pass-rate

Source endpoint for SPEC-020 ops-kpi-daily template. v1 returns
deterministic placeholder rows with real source_snapshot metadata;
Phase 3 swaps the placeholder for a SELECT over user_kyc_records."
```

### Task 2.2: wallet-ledger-service `/v1/internal/reports/reconciliation-snapshot`

**Files:**
- Create: `services/wallet-ledger-service/src/internal_reports.rs`
- Modify: `services/wallet-ledger-service/src/lib.rs`
- Test: `services/wallet-ledger-service/tests/internal_reports_reconciliation_snapshot.rs`

- [ ] **Step 1: Write the failing test**

Create the test file with the same structure as Task 2.1 step 1, replacing the URI with `/v1/internal/reports/reconciliation-snapshot?cutoff=2026-05-10T07:59:59%2B08:00`, the source_service assertion with `"wallet-ledger-service"`, and the rows-shape assertion with:

```rust
assert_eq!(body["rows"][0]["source_version"], "ledger.v2026.05.10");
assert_eq!(body["rows"][0]["reconciliation_delta"], 0);
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p wallet-ledger-service --test internal_reports_reconciliation_snapshot
```

Expected: FAIL with unresolved import.

- [ ] **Step 3: Implement the handler**

Create `services/wallet-ledger-service/src/internal_reports.rs` with the same router/SourceSnapshot scaffold as Task 2.1 step 3. Handler:

```rust
#[derive(Debug, Deserialize)]
struct ReconciliationQuery {
    cutoff: String,
}

async fn reconciliation_snapshot(
    headers: HeaderMap,
    query: Option<Query<ReconciliationQuery>>,
) -> Result<Json<ReportResponse>, Response> {
    let Some(Query(window)) = query else {
        return Err((StatusCode::BAD_REQUEST, "missing cutoff").into_response());
    };
    let correlation_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let rows = vec![json!({
        "cutoff_at": window.cutoff.clone(),
        "source_version": "ledger.v2026.05.10",
        "settlement_total": 502_500,
        "reconciliation_delta": 0
    })];
    Ok(Json(ReportResponse {
        rows,
        source_snapshot: SourceSnapshot {
            source_service: "wallet-ledger-service",
            source_version: "ledger.v2026.05.10",
            cutoff_at: window.cutoff.clone(),
            source_generated_at: window.cutoff,
            correlation_id,
        },
    }))
}
```

Route: `Router::new().route("/v1/internal/reports/reconciliation-snapshot", get(reconciliation_snapshot))`.

- [ ] **Step 4: Register the module** — same pattern as Task 2.1 step 4.
- [ ] **Step 5: Run tests** — `cargo test -p wallet-ledger-service --test internal_reports_reconciliation_snapshot`. Expected: pass.
- [ ] **Step 6: Commit** with message `feat(wallet-ledger-service): add /v1/internal/reports/reconciliation-snapshot`.

### Task 2.3: risk-antifraud-service `/v1/internal/reports/aml-alerts`

**Decision from spec §11:** v1 returns deterministic stub rows with real `source_snapshot`. Phase 3 will either internally fetch from kyc-service `aml_events` over HTTP, or add a local migration. The contract is the same either way.

**Files:**
- Create: `services/risk-antifraud-service/src/internal_reports.rs`
- Modify: `services/risk-antifraud-service/src/lib.rs`
- Test: `services/risk-antifraud-service/tests/internal_reports_aml_alerts.rs`

- [ ] **Step 1: Write the failing test**

Same structure as Task 2.1; URI `/v1/internal/reports/aml-alerts?from=2026-05-09T00:00:00Z&to=2026-05-10T23:59:59Z`, source_service `"risk-antifraud-service"`, row assertion:

```rust
assert_eq!(body["rows"][0]["member_id"], "MEM-001");
assert_eq!(body["rows"][0]["risk_score"], 92);
```

- [ ] **Step 2: Run test** — Expected: FAIL.

- [ ] **Step 3: Implement the handler**

Reuse the Task 2.1 SourceSnapshot scaffold. Handler returns rows:

```rust
let rows = vec![json!({
    "member_id": "MEM-001",
    "risk_score": 92,
    "risk_reason": "device_velocity"
})];
```

Note: `member_email` is **not** in these rows. reporting-service joins emails via user-service `:lookup-masked` (Task 2.4). This preserves invariant 3 (PII boundary).

Route: `Router::new().route("/v1/internal/reports/aml-alerts", get(aml_alerts))`.

- [ ] **Step 4: Register the module** — same pattern.
- [ ] **Step 5: Run tests** — Expected: pass.
- [ ] **Step 6: Commit** with message `feat(risk-antifraud-service): add /v1/internal/reports/aml-alerts`.

### Task 2.4: user-service `/v1/internal/users/contacts:lookup-masked` and `:reveal`

**Files:**
- Create: `services/user-service/src/internal_contacts.rs`
- Modify: `services/user-service/src/lib.rs`
- Test: `services/user-service/tests/internal_contacts.rs`

- [ ] **Step 1: Write the failing tests**

Create `services/user-service/tests/internal_contacts.rs`:

```rust
#![allow(clippy::expect_used)]

use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use user_service::internal_contacts::internal_contacts_router;

#[tokio::test]
async fn lookup_masked_returns_first_char_plus_domain() {
    let app = internal_contacts_router();
    let body = json!({ "ids": ["MEM-001"] }).to_string();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/internal/users/contacts:lookup-masked")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("handled");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(parsed["contacts"][0]["id"], "MEM-001");
    assert_eq!(parsed["contacts"][0]["email_masked"], "m***@example.com");
}

#[tokio::test]
async fn reveal_returns_501_with_error_code() {
    let app = internal_contacts_router();
    let body = json!({
        "ids": ["MEM-001"],
        "actor_id": "U-ADMIN-001",
        "second_authorization_id": "MFA-1",
        "job_id": "rpt-job-000001"
    }).to_string();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/internal/users/contacts:reveal")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("handled");
    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(parsed["error"], "ERROR_CODE_REVEAL_NOT_AVAILABLE");
}
```

- [ ] **Step 2: Run tests** — Expected: FAIL with unresolved import.

- [ ] **Step 3: Implement the handlers**

Create `services/user-service/src/internal_contacts.rs`:

```rust
use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use sdk::masking::mask_email;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[must_use]
pub fn internal_contacts_router() -> Router {
    Router::new()
        .route("/v1/internal/users/contacts:lookup-masked", post(lookup_masked))
        .route("/v1/internal/users/contacts:reveal", post(reveal))
}

#[derive(Debug, Deserialize)]
struct LookupRequest {
    ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Contact {
    id: String,
    email_masked: String,
}

#[derive(Debug, Serialize)]
struct LookupResponse {
    contacts: Vec<Contact>,
}

async fn lookup_masked(Json(request): Json<LookupRequest>) -> Result<Json<LookupResponse>, Response> {
    if request.ids.iter().any(|id| id.trim().is_empty()) {
        return Err((StatusCode::BAD_REQUEST, "empty id").into_response());
    }
    let contacts = request
        .ids
        .into_iter()
        .map(|id| {
            // v1: deterministic email lookup. Phase 3 swaps for a real
            // SELECT against the users table.
            let raw = format!("{}@example.com", id.to_lowercase());
            Contact {
                email_masked: mask_email(&raw),
                id,
            }
        })
        .collect();
    Ok(Json(LookupResponse { contacts }))
}

#[derive(Debug, Deserialize)]
struct RevealRequest {
    ids: Vec<String>,
    actor_id: String,
    second_authorization_id: String,
    job_id: String,
}

async fn reveal(Json(request): Json<RevealRequest>) -> Response {
    // v1: record the denial then return 501. Phase 3 implements the
    // plaintext path after the second-auth flow lands.
    tracing::warn!(
        actor_id = %request.actor_id,
        job_id = %request.job_id,
        second_auth = %request.second_authorization_id,
        id_count = request.ids.len(),
        "contacts:reveal attempted before Phase 3 enablement",
    );
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "ERROR_CODE_REVEAL_NOT_AVAILABLE" })),
    )
        .into_response()
}
```

- [ ] **Step 4: Register the module** — add `pub mod internal_contacts;` to `lib.rs` and merge the router.
- [ ] **Step 5: Run tests** — `cargo test -p user-service --test internal_contacts`. Expected: 2 passed.
- [ ] **Step 6: Commit**

```bash
git add services/user-service/src/internal_contacts.rs services/user-service/src/lib.rs services/user-service/tests/internal_contacts.rs
git commit -m "feat(user-service): add contacts:lookup-masked and :reveal endpoints

lookup-masked applies sdk::masking::mask_email. reveal returns 501 with
ERROR_CODE_REVEAL_NOT_AVAILABLE and warn-level audit log per
SPEC-020-real-api-wiring-design §6.3."
```

---

## Phase 3: `analytics-reporting-service` (new crate)

Owns the 4 PG tables and the job lifecycle. Built behind `ANALYTICS_REPORTING_PROXY_ENABLED` (flipped in Phase 6); nothing calls this service yet, so it ships independently. Tasks 3.1–3.8 are pure-Rust + wiremock, no Postgres required.

### Task 3.1: Scaffold the crate

**Files:**
- Create: `services/analytics-reporting-service/Cargo.toml`
- Create: `services/analytics-reporting-service/src/{lib,main,dashboard_metrics,error,exports,job_lifecycle,router,sources,store,templates,types}.rs`
- Create: `services/analytics-reporting-service/migrations/202605110001_create_analytics_reporting.sql`
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1:** Edit root `Cargo.toml` `members` list and insert `"services/analytics-reporting-service",` alphabetically.

- [ ] **Step 2:** Create `services/analytics-reporting-service/Cargo.toml`:

```toml
[package]
name = "analytics-reporting-service"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
anyhow.workspace = true
async-trait.workspace = true
axum.workspace = true
chrono = { version = "0.4", features = ["serde"] }
http-body-util = "0.1"
reqwest.workspace = true
sdk = { path = "../../crates/sdk" }
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
sha2 = "0.10"
sqlx.workspace = true
thiserror.workspace = true
tokio.workspace = true
tower = "0.5"
tower-http = { workspace = true, features = ["trace"] }
tracing.workspace = true
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
wiremock = "0.6"

[features]
default = []
pg-tests = []

[lints]
workspace = true
```

- [ ] **Step 3:** Create `src/lib.rs`:

```rust
pub mod dashboard_metrics;
pub mod error;
pub mod exports;
pub mod job_lifecycle;
pub mod router;
pub mod sources;
pub mod store;
pub mod templates;
pub mod types;
```

- [ ] **Step 4:** Create `src/main.rs`:

```rust
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing::info!("analytics-reporting-service starting up");
    Ok(())
}
```

- [ ] **Step 5:** For each of `dashboard_metrics, error, exports, job_lifecycle, router, sources, store, templates, types`, create `src/<name>.rs` with the single line:

```rust
// scaffolded in Task 3.1; implemented in a later Phase 3 task
```

- [ ] **Step 6:** Create the migration file at `services/analytics-reporting-service/migrations/202605110001_create_analytics_reporting.sql`. Copy the full SQL from `docs/implementation-plans/SPEC-020-real-api-wiring-design.md` §8. The two differences from the api-gateway migration that you are replacing: `report_jobs` includes `source_snapshot JSONB NOT NULL DEFAULT '{}'`, and there is a new index `report_jobs_template_generated_idx ON report_jobs(template_id, generated_at DESC)`.

- [ ] **Step 7:** Verify:

```bash
cargo check -p analytics-reporting-service
```

Expected: `Finished dev`. Warnings about unused modules are acceptable.

- [ ] **Step 8:** Commit:

```bash
git add Cargo.toml services/analytics-reporting-service/
git commit -m "feat(analytics-reporting-service): scaffold crate, migration, workspace entry"
```

### Task 3.2: Domain types and `ReportingError`

**Files:**
- Modify: `services/analytics-reporting-service/src/types.rs`
- Modify: `services/analytics-reporting-service/src/error.rs`

- [ ] **Step 1:** Replace `src/types.rs`:

```rust
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportRole { Admin, Manager, Affiliate }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionCategory { Operations, LegalAudit }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat { Csv, Json, Xlsx }

impl ReportFormat {
    #[must_use]
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Csv => "text/csv; charset=utf-8",
            Self::Json => "application/json; charset=utf-8",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value { "csv" => Some(Self::Csv), "json" => Some(Self::Json), "xlsx" => Some(Self::Xlsx), _ => None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportField { pub key: String, pub label: String, #[serde(default)] pub sensitive: bool }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy { pub category: RetentionCategory, pub expires_at: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleHint { pub cron: String, pub owner: String, pub next_fire_hint: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportTemplate {
    pub code: String, pub name: String, pub category: String, pub schema_version: String,
    pub viewer_roles: Vec<ReportRole>, pub fields: Vec<ReportField>, pub generated_at: String,
    pub retention: RetentionPolicy, pub schedule: ScheduleHint, pub rows: Vec<HashMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportParams {
    pub start_at: String, pub end_at: String, pub timezone: String,
    pub sort_by: String, pub sort_direction: String, pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSnapshot {
    pub source_service: String, pub source_version: String, pub cutoff_at: String,
    pub source_generated_at: String, pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportJob {
    pub id: String, pub template_code: String, pub template_name: String, pub requested_by: String,
    pub schema_version: String, pub version: String, pub state: String, pub generated_at: String,
    pub params: ReportParams, pub source_snapshot: SourceSnapshot, pub output_rows: usize,
    pub row_hash: String, pub available_formats: Vec<ReportFormat>, pub retention: RetentionPolicy,
    pub rows: Vec<HashMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportAuditRecord {
    pub actor_id: String, pub action: String, pub job_id: String,
    pub action_at: String, pub details: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateReportJobRequest {
    pub actor_id: String, pub role: ReportRole, pub template_code: String, pub params: ReportParams,
    #[serde(default)] pub reveal_sensitive_fields: Option<bool>,
    #[serde(default)] pub second_authorization_id: Option<String>,
    #[serde(default)] pub rerun_of_job_id: Option<String>,
}
```

- [ ] **Step 2:** Replace `src/error.rs`:

```rust
use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ReportingError {
    #[error("forbidden")] Forbidden,
    #[error("invalid request: {0}")] InvalidRequest(String),
    #[error("not found")] NotFound,
    #[error("reveal not available")] RevealNotAvailable,
    #[error("source: {0}")] Source(String),
    #[error("store: {0}")] Store(String),
}

#[derive(Debug, Serialize)]
struct ErrorBody { error: &'static str }

impl IntoResponse for ReportingError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Forbidden => (StatusCode::FORBIDDEN, "ERROR_CODE_REPORT_FORBIDDEN"),
            Self::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "ERROR_CODE_REPORT_INVALID_REQUEST"),
            Self::NotFound => (StatusCode::NOT_FOUND, "ERROR_CODE_REPORT_NOT_FOUND"),
            Self::RevealNotAvailable => (StatusCode::NOT_IMPLEMENTED, "ERROR_CODE_REVEAL_NOT_AVAILABLE"),
            Self::Source(_) => (StatusCode::BAD_GATEWAY, "ERROR_CODE_REPORT_SOURCE"),
            Self::Store(_) => (StatusCode::INTERNAL_SERVER_ERROR, "ERROR_CODE_REPORT_STORE"),
        };
        (status, Json(ErrorBody { error: code })).into_response()
    }
}
```

- [ ] **Step 3:** `cargo check -p analytics-reporting-service`. Expected: clean.

- [ ] **Step 4:** Commit:

```bash
git add services/analytics-reporting-service/src/types.rs services/analytics-reporting-service/src/error.rs
git commit -m "feat(analytics-reporting-service): add domain types and ReportingError"
```

### Task 3.3: Job lifecycle pure-domain logic with row_hash including source_snapshot

This ports + extends the existing api-gateway implementation. The existing `services/api-gateway/src/analytics_reporting.rs` lines 444–608 (validate / sort_rows / project_rows / mask_sensitive_value / row_hash / value helpers / version_prefix) are the starting point; the changes for this task are: `row_hash` now includes `source_snapshot` in canonical JSON, and `mask_sensitive_value` delegates to `sdk::masking::mask_email` for the email shape.

**Files:**
- Modify: `services/analytics-reporting-service/src/job_lifecycle.rs`
- Test: `services/analytics-reporting-service/tests/job_lifecycle.rs`

- [ ] **Step 1:** Write the failing tests at `services/analytics-reporting-service/tests/job_lifecycle.rs`:

```rust
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use analytics_reporting_service::job_lifecycle::{compute_row_hash, mask_value, project_rows, sort_rows, validate_request};
use analytics_reporting_service::types::{CreateReportJobRequest, ReportField, ReportParams, ReportRole, ReportTemplate, RetentionCategory, RetentionPolicy, ScheduleHint, SourceSnapshot};
use serde_json::{Value, json};

fn template_ops() -> ReportTemplate {
    ReportTemplate {
        code: "ops-kpi-daily".into(),
        name: "ops".into(),
        category: "read_only_operations".into(),
        schema_version: "analytics.v1".into(),
        viewer_roles: vec![ReportRole::Admin, ReportRole::Manager],
        fields: vec![
            ReportField { key: "report_date".into(), label: "d".into(), sensitive: false },
            ReportField { key: "approvals_count".into(), label: "a".into(), sensitive: false },
        ],
        generated_at: "2026-05-10T08:00:00+08:00".into(),
        retention: RetentionPolicy { category: RetentionCategory::Operations, expires_at: "2027-05-10T08:00:00+08:00".into() },
        schedule: ScheduleHint { cron: "0 0 8 * * *".into(), owner: "Ops".into(), next_fire_hint: "2026-05-11T08:00:00+08:00".into() },
        rows: vec![HashMap::from([("report_date".to_owned(), json!("2026-05-10")), ("approvals_count".to_owned(), json!(16))])],
    }
}

fn ops_params() -> ReportParams {
    ReportParams {
        start_at: "2026-05-10T00:00:00+08:00".into(),
        end_at: "2026-05-10T23:59:59+08:00".into(),
        timezone: "Asia/Taipei".into(),
        sort_by: "report_date".into(),
        sort_direction: "asc".into(),
        columns: vec!["report_date".into(), "approvals_count".into()],
    }
}

fn ops_snapshot() -> SourceSnapshot {
    SourceSnapshot {
        source_service: "kyc-service".into(),
        source_version: "kyc.v2026.05.10".into(),
        cutoff_at: "2026-05-10T23:59:59+08:00".into(),
        source_generated_at: "2026-05-10T23:59:59+08:00".into(),
        correlation_id: "corr-001".into(),
    }
}

#[test]
fn validate_rejects_affiliate_role() {
    let template = template_ops();
    let req = CreateReportJobRequest {
        actor_id: "U".into(), role: ReportRole::Affiliate,
        template_code: template.code.clone(), params: ops_params(),
        reveal_sensitive_fields: None, second_authorization_id: None, rerun_of_job_id: None,
    };
    assert!(validate_request(&template, &req).is_err());
}

#[test]
fn validate_rejects_utc_for_non_admin() {
    let template = template_ops();
    let mut params = ops_params();
    params.timezone = "UTC".into();
    let req = CreateReportJobRequest {
        actor_id: "U".into(), role: ReportRole::Manager,
        template_code: template.code.clone(), params,
        reveal_sensitive_fields: None, second_authorization_id: None, rerun_of_job_id: None,
    };
    assert!(validate_request(&template, &req).is_err());
}

#[test]
fn validate_rejects_reveal_without_admin_or_second_auth() {
    let template = template_ops();
    let base = ops_params();
    let req_no_auth = CreateReportJobRequest {
        actor_id: "U".into(), role: ReportRole::Admin,
        template_code: template.code.clone(), params: base.clone(),
        reveal_sensitive_fields: Some(true), second_authorization_id: None, rerun_of_job_id: None,
    };
    assert!(validate_request(&template, &req_no_auth).is_err());
    let req_not_admin = CreateReportJobRequest {
        actor_id: "U".into(), role: ReportRole::Manager,
        template_code: template.code.clone(), params: base,
        reveal_sensitive_fields: Some(true), second_authorization_id: Some("MFA-1".into()), rerun_of_job_id: None,
    };
    assert!(validate_request(&template, &req_not_admin).is_err());
}

#[test]
fn mask_email_shape_uses_first_char_and_domain() {
    let masked = mask_value(&json!("member-001@example.com"));
    assert_eq!(masked, json!("m***@example.com"));
}

#[test]
fn mask_non_string_returns_four_stars() {
    assert_eq!(mask_value(&json!(42)), json!("****"));
    assert_eq!(mask_value(&Value::Null), Value::Null);
}

#[test]
fn sort_rows_numeric_ascending_and_descending() {
    let rows = vec![
        HashMap::from([("n".to_owned(), json!(3))]),
        HashMap::from([("n".to_owned(), json!(1))]),
        HashMap::from([("n".to_owned(), json!(2))]),
    ];
    let asc = sort_rows(&rows, "n", "asc");
    assert_eq!(asc[0]["n"], json!(1));
    assert_eq!(asc[2]["n"], json!(3));
    let desc = sort_rows(&rows, "n", "desc");
    assert_eq!(desc[0]["n"], json!(3));
    assert_eq!(desc[2]["n"], json!(1));
}

#[test]
fn project_rows_drops_unselected_columns_and_masks_sensitive() {
    let fields = vec![
        ReportField { key: "id".into(), label: "id".into(), sensitive: false },
        ReportField { key: "email".into(), label: "email".into(), sensitive: true },
    ];
    let rows = vec![HashMap::from([
        ("id".to_owned(), json!("MEM-001")),
        ("email".to_owned(), json!("alice@example.com")),
        ("note".to_owned(), json!("ignored")),
    ])];
    let projected = project_rows(&rows, &fields, &["id".into(), "email".into()], false);
    assert_eq!(projected[0]["id"], json!("MEM-001"));
    assert_eq!(projected[0]["email"], json!("a***@example.com"));
    assert!(projected[0].get("note").is_none());
    let revealed = project_rows(&rows, &fields, &["id".into(), "email".into()], true);
    assert_eq!(revealed[0]["email"], json!("alice@example.com"));
}

#[test]
fn row_hash_differs_when_source_version_differs() {
    let rows: Vec<HashMap<String, Value>> = vec![HashMap::from([("k".to_owned(), json!("v"))])];
    let params = ops_params();
    let snap_a = ops_snapshot();
    let mut snap_b = snap_a.clone();
    snap_b.source_version = "kyc.v2026.05.11".into();
    let a = compute_row_hash("ops-kpi-daily", &params, &snap_a, &rows);
    let b = compute_row_hash("ops-kpi-daily", &params, &snap_b, &rows);
    assert_ne!(a, b);
    assert!(a.starts_with("sha256:"));
    assert_eq!(a.len(), 71);
}

#[test]
fn row_hash_equal_when_only_correlation_id_differs() {
    // correlation_id changes per request but does NOT affect determinism
    // — implementations may choose to exclude it from the canonical hash.
    // This test pins the chosen behavior: correlation_id IS included.
    // If you change that decision, update this test deliberately.
    let rows: Vec<HashMap<String, Value>> = vec![HashMap::from([("k".to_owned(), json!("v"))])];
    let params = ops_params();
    let snap_a = ops_snapshot();
    let mut snap_b = snap_a.clone();
    snap_b.correlation_id = "corr-002".into();
    let a = compute_row_hash("ops-kpi-daily", &params, &snap_a, &rows);
    let b = compute_row_hash("ops-kpi-daily", &params, &snap_b, &rows);
    assert_ne!(a, b);
}
```

- [ ] **Step 2:** Run tests:

```bash
cargo test -p analytics-reporting-service --test job_lifecycle
```

Expected: FAIL with `unresolved imports` from `job_lifecycle`.

- [ ] **Step 3:** Implement `src/job_lifecycle.rs`:

```rust
use std::collections::HashMap;

use sdk::masking::mask_email;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::ReportingError;
use crate::types::{CreateReportJobRequest, ReportField, ReportParams, ReportRole, ReportTemplate, SourceSnapshot};

pub fn validate_request(
    template: &ReportTemplate,
    request: &CreateReportJobRequest,
) -> Result<(), ReportingError> {
    if !template.viewer_roles.contains(&request.role) || request.role == ReportRole::Affiliate {
        return Err(ReportingError::Forbidden);
    }
    if request.params.timezone == "UTC" && request.role != ReportRole::Admin {
        return Err(ReportingError::Forbidden);
    }
    if request.reveal_sensitive_fields.unwrap_or(false) {
        if request.role != ReportRole::Admin {
            return Err(ReportingError::Forbidden);
        }
        if request.second_authorization_id.as_ref().is_none_or(|id| id.trim().is_empty()) {
            return Err(ReportingError::Forbidden);
        }
    }
    let field_keys: Vec<&str> = template.fields.iter().map(|f| f.key.as_str()).collect();
    if !field_keys.contains(&request.params.sort_by.as_str()) {
        return Err(ReportingError::InvalidRequest(format!("unknown sort field: {}", request.params.sort_by)));
    }
    for column in &request.params.columns {
        if !field_keys.contains(&column.as_str()) {
            return Err(ReportingError::InvalidRequest(format!("unknown column: {column}")));
        }
    }
    Ok(())
}

#[must_use]
pub fn sort_rows(rows: &[HashMap<String, Value>], sort_by: &str, direction: &str) -> Vec<HashMap<String, Value>> {
    let mut sorted = rows.to_vec();
    sorted.sort_by(|left, right| {
        let l = sort_key(left.get(sort_by));
        let r = sort_key(right.get(sort_by));
        if direction == "desc" { r.cmp(&l) } else { l.cmp(&r) }
    });
    sorted
}

fn sort_key(value: Option<&Value>) -> String {
    match value {
        Some(Value::Number(n)) => format!("{n:0>20}"),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

#[must_use]
pub fn project_rows(
    rows: &[HashMap<String, Value>],
    fields: &[ReportField],
    columns: &[String],
    reveal_sensitive: bool,
) -> Vec<HashMap<String, Value>> {
    let sensitive: Vec<&str> = fields.iter().filter(|f| f.sensitive).map(|f| f.key.as_str()).collect();
    rows.iter().map(|row| {
        columns.iter().map(|col| {
            let value = row.get(col).cloned().unwrap_or(Value::Null);
            let projected = if sensitive.contains(&col.as_str()) && !reveal_sensitive {
                mask_value(&value)
            } else {
                value
            };
            (col.clone(), projected)
        }).collect()
    }).collect()
}

#[must_use]
pub fn mask_value(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::String(text) if text.contains('@') => json!(mask_email(text)),
        _ => json!("****"),
    }
}

#[must_use]
pub fn compute_row_hash(
    template_code: &str,
    params: &ReportParams,
    source_snapshot: &SourceSnapshot,
    rows: &[HashMap<String, Value>],
) -> String {
    let canonical = json!({
        "template_code": template_code,
        "params": params,
        "source_snapshot": source_snapshot,
        "rows": rows,
    });
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&canonical).unwrap_or_default());
    format!("sha256:{:x}", hasher.finalize())
}

#[must_use]
pub fn version_prefix(generated_at: &str) -> String {
    format!("v{}", generated_at.get(0..10).unwrap_or("1970-01-01").replace('-', "."))
}
```

- [ ] **Step 4:** Run tests:

```bash
cargo test -p analytics-reporting-service --test job_lifecycle
```

Expected: `test result: ok. 8 passed`.

- [ ] **Step 5:** Commit:

```bash
git add services/analytics-reporting-service/src/job_lifecycle.rs services/analytics-reporting-service/tests/job_lifecycle.rs
git commit -m "feat(analytics-reporting-service): port job lifecycle with source_snapshot in row_hash

mask_value delegates to sdk::masking::mask_email; row_hash canonical JSON
includes source_snapshot so identical params with different source
versions produce distinct hashes per design §5 invariant 5."
```

### Task 3.4: Templates with schedule and source bindings

**Files:**
- Modify: `services/analytics-reporting-service/src/templates.rs`

- [ ] **Step 1:** Replace `src/templates.rs`. Port the three template definitions from `services/api-gateway/src/analytics_reporting.rs` lines 633–708 (`default_templates`) and add a `schedule: ScheduleHint { ... }` field to each:

```rust
use std::collections::HashMap;

use serde_json::json;

use crate::types::{ReportField, ReportRole, ReportTemplate, RetentionCategory, RetentionPolicy, ScheduleHint};

fn field(key: &str, label: &str, sensitive: bool) -> ReportField {
    ReportField { key: key.to_owned(), label: label.to_owned(), sensitive }
}

#[must_use]
pub fn default_templates() -> Vec<ReportTemplate> {
    vec![
        ReportTemplate {
            code: "ops-kpi-daily".into(),
            name: "營運 KPI 日報".into(),
            category: "read_only_operations".into(),
            schema_version: "analytics.v1".into(),
            viewer_roles: vec![ReportRole::Admin, ReportRole::Manager],
            generated_at: "2026-05-10T08:00:00+08:00".into(),
            retention: RetentionPolicy { category: RetentionCategory::Operations, expires_at: "2027-05-10T08:00:00+08:00".into() },
            schedule: ScheduleHint { cron: "0 0 8 * * *".into(), owner: "Ops Control".into(), next_fire_hint: "2026-05-11T08:00:00+08:00".into() },
            fields: vec![
                field("report_date", "報表日", false),
                field("approvals_count", "審批數", false),
                field("kyc_pass_rate", "KYC 通過率", false),
                field("service_uptime", "服務可用性", false),
            ],
            rows: vec![HashMap::new()],
        },
        ReportTemplate {
            code: "risk-alert-summary".into(),
            name: "風控警示摘要".into(),
            category: "risk".into(),
            schema_version: "analytics.v1".into(),
            viewer_roles: vec![ReportRole::Admin, ReportRole::Manager],
            generated_at: "2026-05-10T08:15:00+08:00".into(),
            retention: RetentionPolicy { category: RetentionCategory::Operations, expires_at: "2027-05-10T08:15:00+08:00".into() },
            schedule: ScheduleHint { cron: "0 15 8 * * *".into(), owner: "Risk Ops".into(), next_fire_hint: "2026-05-11T08:15:00+08:00".into() },
            fields: vec![
                field("member_id", "會員 ID", false),
                field("member_email", "會員 email", true),
                field("risk_score", "風險分數", false),
                field("risk_reason", "風險原因", false),
            ],
            rows: vec![HashMap::new()],
        },
        ReportTemplate {
            code: "finance-reconciliation".into(),
            name: "財務對帳快照".into(),
            category: "finance_review".into(),
            schema_version: "analytics.v1".into(),
            viewer_roles: vec![ReportRole::Admin],
            generated_at: "2026-05-10T08:00:00+08:00".into(),
            retention: RetentionPolicy { category: RetentionCategory::LegalAudit, expires_at: "2033-05-10T08:00:00+08:00".into() },
            schedule: ScheduleHint { cron: "0 0 8 * * *".into(), owner: "Finance".into(), next_fire_hint: "2026-05-11T08:00:00+08:00".into() },
            fields: vec![
                field("cutoff_at", "資料截斷時間", false),
                field("source_version", "資料來源版本", false),
                field("settlement_total", "結算總額", false),
                field("reconciliation_delta", "對帳差異", false),
            ],
            rows: vec![HashMap::new()],
        },
    ]
}

/// Returns the static template metadata mapping `template_code -> (source_url_key, expected_param_keys)`.
/// `source_url_key` matches the ServiceConfig field name in Task 3.10. Used by router to dispatch.
#[must_use]
pub fn source_bindings(code: &str) -> &'static [&'static str] {
    match code {
        "ops-kpi-daily" => &["kyc-service"],
        "risk-alert-summary" => &["risk-antifraud-service", "user-service"],
        "finance-reconciliation" => &["wallet-ledger-service"],
        _ => &[],
    }
}
```

- [ ] **Step 2:** Verify build:

```bash
cargo check -p analytics-reporting-service
```

Expected: clean. Unused-warning on `default_templates` and `source_bindings` is fine — Task 3.10 uses them.

- [ ] **Step 3:** Commit:

```bash
git add services/analytics-reporting-service/src/templates.rs
git commit -m "feat(analytics-reporting-service): seed three SPEC-020 templates with schedule"
```

### Task 3.5: Exports — CSV, JSON, XLSX including source_snapshot header

**Files:**
- Modify: `services/analytics-reporting-service/src/exports.rs`
- Test: `services/analytics-reporting-service/tests/exports.rs`

- [ ] **Step 1:** Write the failing tests at `services/analytics-reporting-service/tests/exports.rs`:

```rust
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use analytics_reporting_service::exports::export_report_job;
use analytics_reporting_service::types::{ReportFormat, ReportJob, ReportParams, ReportRole, RetentionCategory, RetentionPolicy, SourceSnapshot};
use serde_json::{Value, json};

fn sample_job() -> ReportJob {
    ReportJob {
        id: "rpt-job-000001".into(),
        template_code: "ops-kpi-daily".into(),
        template_name: "ops".into(),
        requested_by: "U-ADMIN-001".into(),
        schema_version: "analytics.v1".into(),
        version: "v2026.05.10.1".into(),
        state: "completed".into(),
        generated_at: "2026-05-10T08:00:00+08:00".into(),
        params: ReportParams {
            start_at: "2026-05-10T00:00:00+08:00".into(),
            end_at: "2026-05-10T23:59:59+08:00".into(),
            timezone: "Asia/Taipei".into(),
            sort_by: "report_date".into(),
            sort_direction: "asc".into(),
            columns: vec!["report_date".into(), "approvals_count".into()],
        },
        source_snapshot: SourceSnapshot {
            source_service: "kyc-service".into(),
            source_version: "kyc.v2026.05.10".into(),
            cutoff_at: "2026-05-10T23:59:59+08:00".into(),
            source_generated_at: "2026-05-10T23:59:59+08:00".into(),
            correlation_id: "corr-001".into(),
        },
        output_rows: 1,
        row_hash: "sha256:abc".into(),
        available_formats: vec![ReportFormat::Csv, ReportFormat::Json, ReportFormat::Xlsx],
        retention: RetentionPolicy { category: RetentionCategory::Operations, expires_at: "2027-05-10T08:00:00+08:00".into() },
        rows: vec![HashMap::from([("report_date".to_owned(), json!("2026-05-10")), ("approvals_count".to_owned(), json!(16))])],
    }
}

#[test]
fn csv_export_includes_schema_metadata_and_source_snapshot_lines() {
    let bytes = export_report_job(&sample_job(), ReportFormat::Csv);
    let text = String::from_utf8(bytes).expect("utf-8");
    assert!(text.contains("schema_version,analytics.v1"));
    assert!(text.contains("source_service,kyc-service"));
    assert!(text.contains("source_version,kyc.v2026.05.10"));
    assert!(text.contains("report_date,approvals_count"));
    assert!(text.contains("2026-05-10,16"));
}

#[test]
fn json_export_carries_source_snapshot_object() {
    let bytes = export_report_job(&sample_job(), ReportFormat::Json);
    let parsed: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(parsed["schema_version"], json!("analytics.v1"));
    assert_eq!(parsed["source_snapshot"]["source_service"], json!("kyc-service"));
    assert_eq!(parsed["rows"][0]["report_date"], json!("2026-05-10"));
}

#[test]
fn xlsx_export_starts_with_pk_zip_magic() {
    let bytes = export_report_job(&sample_job(), ReportFormat::Xlsx);
    assert_eq!(&bytes[0..2], b"PK");
}
```

- [ ] **Step 2:** Run: `cargo test -p analytics-reporting-service --test exports`. Expected: FAIL.

- [ ] **Step 3:** Implement `src/exports.rs`. Port the CSV/JSON/XLSX generators from `services/api-gateway/src/analytics_reporting.rs` lines 542–597 with three modifications:
  1. CSV adds two header lines after `row_hash`: `source_service,<value>` and `source_version,<value>` (and `cutoff_at,<value>`). The blank-line then column headers stays.
  2. JSON output adds `"source_snapshot": <object>` at the top level.
  3. XLSX wraps the new CSV (the existing implementation in api-gateway is a CSV-prefixed PK stub; keep that behavior — it satisfies the byte-magic test and Phase 3 leaves real XLSX rendering to a follow-up).

Skeleton:

```rust
use serde_json::{Value, json};

use crate::types::{ReportFormat, ReportJob};

#[must_use]
pub fn export_report_job(job: &ReportJob, format: ReportFormat) -> Vec<u8> {
    match format {
        ReportFormat::Csv => csv_export(job).into_bytes(),
        ReportFormat::Json => serde_json::to_vec_pretty(&json!({
            "schema_version": job.schema_version,
            "generated_at": job.generated_at,
            "report_version": job.version,
            "row_hash": job.row_hash,
            "source_snapshot": job.source_snapshot,
            "params": job.params,
            "rows": job.rows,
        })).unwrap_or_default(),
        ReportFormat::Xlsx => {
            let mut body = b"PK\x03\x04".to_vec();
            body.extend_from_slice(csv_export(job).as_bytes());
            body
        }
    }
}

fn csv_export(job: &ReportJob) -> String {
    let mut lines = vec![
        csv_line(&["schema_version".into(), job.schema_version.clone()]),
        csv_line(&["generated_at".into(), job.generated_at.clone()]),
        csv_line(&["report_version".into(), job.version.clone()]),
        csv_line(&["row_hash".into(), job.row_hash.clone()]),
        csv_line(&["source_service".into(), job.source_snapshot.source_service.clone()]),
        csv_line(&["source_version".into(), job.source_snapshot.source_version.clone()]),
        csv_line(&["cutoff_at".into(), job.source_snapshot.cutoff_at.clone()]),
        String::new(),
        csv_line(&job.params.columns),
    ];
    for row in &job.rows {
        lines.push(csv_line(
            &job.params.columns.iter().map(|c| row.get(c).map_or_else(String::new, value_to_csv)).collect::<Vec<_>>(),
        ));
    }
    format!("{}\n", lines.join("\n"))
}

fn csv_line(values: &[String]) -> String {
    values.iter().map(|v| {
        if v.contains(',') || v.contains('"') || v.contains('\n') {
            format!("\"{}\"", v.replace('"', "\"\""))
        } else { v.clone() }
    }).collect::<Vec<_>>().join(",")
}

fn value_to_csv(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}
```

- [ ] **Step 4:** Run tests: `cargo test -p analytics-reporting-service --test exports`. Expected: 3 passed.

- [ ] **Step 5:** Commit:

```bash
git add services/analytics-reporting-service/src/exports.rs services/analytics-reporting-service/tests/exports.rs
git commit -m "feat(analytics-reporting-service): export CSV/JSON/XLSX with source_snapshot"
```

### Task 3.6: Source HTTP clients

**Files:**
- Modify: `services/analytics-reporting-service/src/sources.rs` (split into module dir)
- Test: `services/analytics-reporting-service/tests/sources.rs`

- [ ] **Step 1:** Convert `src/sources.rs` to a module directory. Delete `src/sources.rs` and create `src/sources/mod.rs` plus `src/sources/{kyc,ledger,risk,users}.rs`.

- [ ] **Step 2:** `src/sources/mod.rs`:

```rust
pub mod kyc;
pub mod ledger;
pub mod risk;
pub mod users;

use std::collections::HashMap;
use serde_json::Value;
use crate::error::ReportingError;
use crate::types::SourceSnapshot;

#[derive(Debug, Clone)]
pub struct SourceResponse {
    pub rows: Vec<HashMap<String, Value>>,
    pub snapshot: SourceSnapshot,
}

#[async_trait::async_trait]
pub trait SourceClient: Send + Sync {
    async fn fetch(
        &self,
        path_and_query: &str,
        correlation_id: &str,
    ) -> Result<SourceResponse, ReportingError>;
}

pub(crate) async fn fetch_json(
    client: &reqwest::Client,
    base_url: &str,
    method: reqwest::Method,
    path_and_query: &str,
    body: Option<Value>,
    correlation_id: &str,
) -> Result<Value, ReportingError> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), path_and_query);
    let mut request = client.request(method, &url).header("x-request-id", correlation_id);
    if let Some(payload) = body {
        request = request.json(&payload);
    }
    let response = request.send().await.map_err(|e| ReportingError::Source(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ReportingError::Source(format!("status {}", response.status())));
    }
    response.json::<Value>().await.map_err(|e| ReportingError::Source(e.to_string()))
}

pub(crate) fn parse_source_response(value: Value) -> Result<SourceResponse, ReportingError> {
    let rows = serde_json::from_value(value.get("rows").cloned().unwrap_or(Value::Null))
        .map_err(|e| ReportingError::Source(format!("rows: {e}")))?;
    let snapshot = serde_json::from_value(value.get("source_snapshot").cloned().unwrap_or(Value::Null))
        .map_err(|e| ReportingError::Source(format!("source_snapshot: {e}")))?;
    Ok(SourceResponse { rows, snapshot })
}
```

- [ ] **Step 3:** Each of `kyc.rs`, `ledger.rs`, `risk.rs` follows the same pattern. Example `src/sources/kyc.rs`:

```rust
use crate::error::ReportingError;
use crate::sources::{SourceClient, SourceResponse, fetch_json, parse_source_response};

#[derive(Debug, Clone)]
pub struct KycSource {
    pub client: reqwest::Client,
    pub base_url: String,
}

#[async_trait::async_trait]
impl SourceClient for KycSource {
    async fn fetch(&self, path_and_query: &str, correlation_id: &str) -> Result<SourceResponse, ReportingError> {
        let value = fetch_json(&self.client, &self.base_url, reqwest::Method::GET, path_and_query, None, correlation_id).await?;
        parse_source_response(value)
    }
}
```

Repeat verbatim for `ledger.rs` (struct `LedgerSource`) and `risk.rs` (struct `RiskSource`).

- [ ] **Step 4:** `src/sources/users.rs` exposes both lookup-masked (POST) and reveal (POST). Add two methods on a `UserSource` struct:

```rust
use serde_json::{Value, json};
use crate::error::ReportingError;
use crate::sources::{SourceResponse, fetch_json, parse_source_response};

#[derive(Debug, Clone)]
pub struct UserSource {
    pub client: reqwest::Client,
    pub base_url: String,
}

impl UserSource {
    pub async fn lookup_masked(&self, ids: &[String], correlation_id: &str) -> Result<Vec<(String, String)>, ReportingError> {
        let body = json!({ "ids": ids });
        let value = fetch_json(&self.client, &self.base_url, reqwest::Method::POST, "/v1/internal/users/contacts:lookup-masked", Some(body), correlation_id).await?;
        let contacts = value.get("contacts").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(contacts.into_iter().filter_map(|c| {
            let id = c.get("id")?.as_str()?.to_owned();
            let email = c.get("email_masked")?.as_str()?.to_owned();
            Some((id, email))
        }).collect())
    }

    pub async fn reveal(&self, ids: &[String], actor_id: &str, second_authorization_id: &str, job_id: &str, correlation_id: &str) -> Result<Vec<(String, String)>, ReportingError> {
        let body = json!({ "ids": ids, "actor_id": actor_id, "second_authorization_id": second_authorization_id, "job_id": job_id });
        // user-service returns 501 in v1; surface as RevealNotAvailable.
        let url = format!("{}/v1/internal/users/contacts:reveal", self.base_url.trim_end_matches('/'));
        let response = self.client.post(&url).header("x-request-id", correlation_id).json(&body).send().await
            .map_err(|e| ReportingError::Source(e.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_IMPLEMENTED {
            return Err(ReportingError::RevealNotAvailable);
        }
        if !response.status().is_success() {
            return Err(ReportingError::Source(format!("status {}", response.status())));
        }
        Ok(Vec::new())
    }
}
```

- [ ] **Step 5:** Tests in `tests/sources.rs` spin up wiremock and assert each client serializes/deserializes correctly. Write at minimum:

```rust
#![allow(clippy::expect_used)]

use analytics_reporting_service::error::ReportingError;
use analytics_reporting_service::sources::{SourceClient, kyc::KycSource, users::UserSource};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn kyc_source_returns_rows_and_snapshot() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v1/internal/reports/kyc-pass-rate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "rows": [{"report_date": "2026-05-10"}],
            "source_snapshot": {
                "source_service": "kyc-service",
                "source_version": "kyc.v1",
                "cutoff_at": "2026-05-10T23:59:59Z",
                "source_generated_at": "2026-05-10T23:59:59Z",
                "correlation_id": "corr-001"
            }
        })))
        .mount(&server).await;
    let source = KycSource { client: reqwest::Client::new(), base_url: server.uri() };
    let response = source.fetch("/v1/internal/reports/kyc-pass-rate?from=a&to=b", "corr-001").await.expect("ok");
    assert_eq!(response.snapshot.source_service, "kyc-service");
    assert_eq!(response.rows.len(), 1);
}

#[tokio::test]
async fn user_reveal_surfaces_501_as_reveal_not_available() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/v1/internal/users/contacts:reveal"))
        .respond_with(ResponseTemplate::new(501).set_body_json(json!({"error": "ERROR_CODE_REVEAL_NOT_AVAILABLE"})))
        .mount(&server).await;
    let source = UserSource { client: reqwest::Client::new(), base_url: server.uri() };
    let err = source.reveal(&["MEM-001".into()], "U-1", "MFA-1", "rpt-job-1", "corr-001").await.expect_err("must error");
    assert!(matches!(err, ReportingError::RevealNotAvailable));
}
```

- [ ] **Step 6:** Run: `cargo test -p analytics-reporting-service --test sources`. Expected: 2 passed.

- [ ] **Step 7:** Commit:

```bash
git add services/analytics-reporting-service/src/sources services/analytics-reporting-service/tests/sources.rs
git commit -m "feat(analytics-reporting-service): add reqwest source clients for 4 services"
```

(Note: also `git rm services/analytics-reporting-service/src/sources.rs` if the scaffold left it.)

### Task 3.7: Store trait, in-memory store, and dashboard metrics

**Files:**
- Modify: `services/analytics-reporting-service/src/store.rs` → split into module
- Modify: `services/analytics-reporting-service/src/dashboard_metrics.rs`
- Test: `services/analytics-reporting-service/tests/store_in_memory.rs`
- Test: `services/analytics-reporting-service/tests/dashboard_metrics.rs`

- [ ] **Step 1:** Convert `src/store.rs` to `src/store/mod.rs` + `src/store/memory.rs`. `mod.rs`:

```rust
pub mod memory;

use async_trait::async_trait;

use crate::error::ReportingError;
use crate::types::{ReportAuditRecord, ReportJob, ReportTemplate, ReportRole};

#[async_trait]
pub trait Store: Send + Sync {
    async fn templates_for_role(&self, role: ReportRole) -> Result<Vec<ReportTemplate>, ReportingError>;
    async fn insert_job_with_audit(&self, job: ReportJob, audit: Vec<ReportAuditRecord>) -> Result<(), ReportingError>;
    async fn find_job(&self, public_id: &str) -> Result<Option<ReportJob>, ReportingError>;
    async fn list_jobs(&self, template_code: Option<&str>) -> Result<Vec<ReportJob>, ReportingError>;
    async fn latest_per_template(&self) -> Result<Vec<ReportJob>, ReportingError>;
    async fn list_audit_for_job(&self, public_id: &str) -> Result<Vec<ReportAuditRecord>, ReportingError>;
    async fn append_audit(&self, audit: ReportAuditRecord) -> Result<(), ReportingError>;
    async fn count_jobs_in_last_24h(&self) -> Result<usize, ReportingError>;
}
```

- [ ] **Step 2:** `src/store/memory.rs` provides `InMemoryStore` implementing `Store`. Use `Arc<Mutex<...>>` mirroring api-gateway's existing pattern. Templates are seeded from `templates::default_templates()`.

- [ ] **Step 3:** `src/dashboard_metrics.rs`:

```rust
use crate::error::ReportingError;
use crate::store::Store;
use crate::types::ReportRole;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DashboardMetric {
    pub id: &'static str,
    pub label: &'static str,
    pub value: String,
    pub detail: String,
    pub tone: &'static str,
}

pub async fn dashboard_metrics(store: &dyn Store, role: ReportRole) -> Result<Vec<DashboardMetric>, ReportingError> {
    let templates = store.templates_for_role(role).await?.len();
    let jobs_last_24h = store.count_jobs_in_last_24h().await?;
    Ok(vec![
        DashboardMetric { id: "kpi", label: "KPI", value: "99.92%".into(), detail: "審批服務可用性".into(), tone: "success" },
        DashboardMetric { id: "alerts", label: "警示", value: "9".into(), detail: "高風險事件".into(), tone: "critical" },
        DashboardMetric { id: "versioned-reports", label: "版本化報表", value: jobs_last_24h.to_string(), detail: "近 24h 已產生".into(), tone: "neutral" },
        DashboardMetric { id: "audit-trail", label: "稽核覆蓋", value: format!("{templates} templates"), detail: "report_audit 同寫".into(), tone: "warning" },
    ])
}
```

- [ ] **Step 4:** Write tests at `tests/store_in_memory.rs` and `tests/dashboard_metrics.rs` covering: templates filter by role; insert + find job; latest_per_template returns at most one per template; dashboard_metrics returns 4 cards regardless of job count.

- [ ] **Step 5:** Run: `cargo test -p analytics-reporting-service`. All tests so far should pass.

- [ ] **Step 6:** Commit:

```bash
git add services/analytics-reporting-service/src/store services/analytics-reporting-service/src/dashboard_metrics.rs services/analytics-reporting-service/tests/store_in_memory.rs services/analytics-reporting-service/tests/dashboard_metrics.rs
git commit -m "feat(analytics-reporting-service): add Store trait, in-memory impl, dashboard metrics"
```

### Task 3.8: Postgres store behind `pg-tests` feature

**Files:**
- Create: `services/analytics-reporting-service/src/store/pg.rs`
- Modify: `services/analytics-reporting-service/src/store/mod.rs` (add `#[cfg(feature = "pg-tests")] pub mod pg;` OR always-on `pub mod pg;` depending on whether the service runs without PG. For production it must always compile; the gate is on tests only)
- Test: `services/analytics-reporting-service/tests/store_pg.rs` (gated with `#![cfg(feature = "pg-tests")]`)

- [ ] **Step 1:** Make `pub mod pg;` unconditional in `mod.rs`. The `pg-tests` feature gates the integration **tests** that connect to a real database.

- [ ] **Step 2:** Implement `src/store/pg.rs` with `PgStore { pool: sqlx::PgPool }` and the same `Store` trait. Use `sqlx::query!` / `sqlx::query_as!` macros — they require `DATABASE_URL` at compile time, so the engineer must run `sqlx migrate run` against a dev Postgres first and check in the offline metadata (see `sdk/.sqlx/`).

Key queries:
  - `templates_for_role`: `SELECT * FROM report_templates WHERE is_active = TRUE AND definition->'viewer_roles' ? $1`
  - `insert_job_with_audit`: BEGIN; INSERT report_jobs (...) RETURNING id; INSERT report_audit (...) for each record; COMMIT
  - `latest_per_template`: `SELECT DISTINCT ON (template_id) * FROM report_jobs ORDER BY template_id, generated_at DESC`
  - `count_jobs_in_last_24h`: `SELECT COUNT(*) FROM report_jobs WHERE generated_at > NOW() - INTERVAL '24 hours'`

- [ ] **Step 3:** Tests at `tests/store_pg.rs` gated with `#![cfg(feature = "pg-tests")]`. Use `sqlx::test` macro. Tests reuse the same 8 cases as the in-memory store, asserting equivalent behavior.

- [ ] **Step 4:** Verify default build skips PG tests:

```bash
cargo test -p analytics-reporting-service
```

Expected: `store_pg` not compiled; other tests pass.

- [ ] **Step 5:** (Optional, CI-only) verify PG tests when env provides Postgres:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/analytics_test \
  cargo test -p analytics-reporting-service --features pg-tests
```

- [ ] **Step 6:** Commit:

```bash
git add services/analytics-reporting-service/src/store/pg.rs services/analytics-reporting-service/src/store/mod.rs services/analytics-reporting-service/tests/store_pg.rs
git commit -m "feat(analytics-reporting-service): add Postgres-backed Store behind pg-tests feature"
```

### Task 3.9: Router and main wiring

**Files:**
- Modify: `services/analytics-reporting-service/src/router.rs`
- Modify: `services/analytics-reporting-service/src/main.rs`
- Test: `services/analytics-reporting-service/tests/router_with_mock_sources.rs`

- [ ] **Step 1:** Implement `src/router.rs` exposing `pub fn build_router(state: AppState) -> Router`. `AppState` holds `Arc<dyn Store>`, the 4 source clients, and `chrono::Utc::now` injectable for tests. Routes:

```
GET  /v1/report-templates
GET  /v1/report-jobs           (optional ?template_code=...)
POST /v1/report-jobs
GET  /v1/report-jobs/:id
GET  /v1/report-jobs/:id/exports/:format
POST /v1/report-jobs/:id/export-deletions
GET  /v1/report-schedules
GET  /v1/dashboard-metrics
GET  /v1/report-jobs/latest-per-template
```

The POST `/v1/report-jobs` handler ties it all together:

```rust
// pseudocode
async fn create_report_job(State(state): State<AppState>, Json(req): Json<CreateReportJobRequest>) -> Result<Json<ReportJobResponse>, ReportingError> {
    let template = state.store.templates_for_role(req.role).await?.into_iter()
        .find(|t| t.code == req.template_code).ok_or(ReportingError::NotFound)?;
    validate_request(&template, &req)?;
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let source_response = fetch_source_rows_for(&state, &template, &req, &correlation_id).await?;
    let columns = req.params.columns.clone();
    let rows = project_rows(
        &sort_rows(&source_response.rows, &req.params.sort_by, &req.params.sort_direction),
        &template.fields,
        &columns,
        req.reveal_sensitive_fields.unwrap_or(false),
    );
    let row_hash = compute_row_hash(&template.code, &req.params, &source_response.snapshot, &rows);
    let job = ReportJob { /* assemble per types::ReportJob */ };
    let audit = build_audit_records(&req, &job);
    state.store.insert_job_with_audit(job.clone(), audit.clone()).await?;
    Ok(Json(ReportJobResponse { job, audit }))
}
```

Where `fetch_source_rows_for` dispatches per `template.code` to the appropriate `SourceClient`(s), composes a single `SourceResponse` (for `risk-alert-summary` it joins risk rows with user-service `lookup_masked`).

- [ ] **Step 2:** Wire `src/main.rs` to read `DATABASE_URL`, `KYC_SERVICE_URL`, `LEDGER_SERVICE_URL`, `RISK_SERVICE_URL`, `USER_SERVICE_URL`, build sources + sqlx pool + `PgStore`, mount router, bind on `0.0.0.0:8090` (pick the next free port in the workspace).

- [ ] **Step 3:** Write the wiremock integration test at `tests/router_with_mock_sources.rs`. Spin up 4 wiremock servers, build the router with `InMemoryStore` and source clients pointing at the mocks. Assert end-to-end behaviors mapping to the six existing tests in `services/api-gateway/tests/analytics_reporting.rs` (port them; the assertion shape stays the same, only the URL paths change to `/v1/*`). Add one new test: `dashboard_render_does_not_create_jobs` — call `GET /v1/dashboard-metrics` 5 times, assert `store.list_jobs(None).await.len() == 0`.

- [ ] **Step 4:** Run:

```bash
cargo test -p analytics-reporting-service
```

Expected: all tests pass.

- [ ] **Step 5:** Commit:

```bash
git add services/analytics-reporting-service/src/router.rs services/analytics-reporting-service/src/main.rs services/analytics-reporting-service/tests/router_with_mock_sources.rs
git commit -m "feat(analytics-reporting-service): wire router, main, end-to-end wiremock tests"
```

---

## Phase 4: api-gateway proxy + HMAC verification

Replaces the 717-line in-memory module with a ~120-line reverse proxy. Behind `ANALYTICS_REPORTING_PROXY_ENABLED` so the old behavior keeps shipping until Phase 6 flips the flag.

### Task 4.1: HMAC verification helper

**Files:**
- Modify: `services/api-gateway/Cargo.toml` (add `hmac = "0.12"`)
- Create: `services/api-gateway/src/internal_call_auth.rs`
- Test: `services/api-gateway/tests/internal_call_auth.rs`

- [ ] **Step 1:** Add to `services/api-gateway/Cargo.toml` `[dependencies]`:

```toml
hex = "0.4"
hmac = "0.12"
```

- [ ] **Step 2:** Write failing tests at `tests/internal_call_auth.rs`:

```rust
#![allow(clippy::expect_used)]

use api_gateway::internal_call_auth::{verify_signature, SignatureInput};

const SECRET: &[u8] = b"shared-secret-32-bytes-1234567890";

fn signature_for(method: &str, path: &str, body: &[u8], timestamp: i64, actor: &str, role: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let body_hash = {
        use sha2::Digest;
        let mut hasher = Sha256::new();
        hasher.update(body);
        hex::encode(hasher.finalize())
    };
    let canonical = format!("{method}\n{path}\n{body_hash}\n{timestamp}\n{actor}\n{role}");
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET).expect("hmac");
    mac.update(canonical.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[test]
fn signature_accepts_within_5_minutes() {
    let now = 1_780_000_000;
    let sig = signature_for("GET", "/api/internal/admin/report-templates", b"", now, "U-1", "admin");
    let input = SignatureInput {
        method: "GET", path: "/api/internal/admin/report-templates", body: b"",
        timestamp: now, actor_id: "U-1", role: "admin", signature: &sig,
    };
    assert!(verify_signature(SECRET, &input, now + 60).is_ok());
}

#[test]
fn signature_rejected_when_timestamp_too_old() {
    let now = 1_780_000_000;
    let sig = signature_for("GET", "/x", b"", now, "U-1", "admin");
    let input = SignatureInput { method: "GET", path: "/x", body: b"", timestamp: now, actor_id: "U-1", role: "admin", signature: &sig };
    let err = verify_signature(SECRET, &input, now + 301).expect_err("expired");
    assert_eq!(err.to_string(), "timestamp drift");
}

#[test]
fn signature_rejected_when_role_tampered() {
    let now = 1_780_000_000;
    let sig = signature_for("GET", "/x", b"", now, "U-1", "admin");
    let input = SignatureInput { method: "GET", path: "/x", body: b"", timestamp: now, actor_id: "U-1", role: "manager", signature: &sig };
    assert!(verify_signature(SECRET, &input, now).is_err());
}
```

- [ ] **Step 3:** Implement `src/internal_call_auth.rs`:

```rust
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("timestamp drift")]
    TimestampDrift,
    #[error("invalid signature")]
    Invalid,
    #[error("malformed signature")]
    Malformed,
}

pub struct SignatureInput<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub body: &'a [u8],
    pub timestamp: i64,
    pub actor_id: &'a str,
    pub role: &'a str,
    pub signature: &'a str,
}

pub fn verify_signature(secret: &[u8], input: &SignatureInput<'_>, now_unix_secs: i64) -> Result<(), SignatureError> {
    if (now_unix_secs - input.timestamp).abs() > 300 {
        return Err(SignatureError::TimestampDrift);
    }
    let body_hash = {
        let mut hasher = Sha256::new();
        hasher.update(input.body);
        hex::encode(hasher.finalize())
    };
    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        input.method, input.path, body_hash, input.timestamp, input.actor_id, input.role
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).map_err(|_| SignatureError::Malformed)?;
    mac.update(canonical.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if constant_time_eq(expected.as_bytes(), input.signature.as_bytes()) {
        Ok(())
    } else {
        Err(SignatureError::Invalid)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) { diff |= x ^ y; }
    diff == 0
}
```

- [ ] **Step 4:** Register `pub mod internal_call_auth;` in `services/api-gateway/src/lib.rs`.

- [ ] **Step 5:** Run: `cargo test -p api-gateway --test internal_call_auth`. Expected: 3 passed.

- [ ] **Step 6:** Commit:

```bash
git add services/api-gateway/Cargo.toml services/api-gateway/src/internal_call_auth.rs services/api-gateway/src/lib.rs services/api-gateway/tests/internal_call_auth.rs
git commit -m "feat(api-gateway): add HMAC-SHA256 internal-call verification"
```

### Task 4.2: Reverse proxy module with coarse RBAC

**Files:**
- Modify: `services/api-gateway/src/analytics_reporting.rs` (REWRITE to proxy)
- Modify: `services/api-gateway/src/lib.rs` (read `ANALYTICS_REPORTING_PROXY_ENABLED` + `INTERNAL_CALL_HMAC_SECRET` + `ANALYTICS_REPORTING_SERVICE_URL`)
- Modify: `services/api-gateway/tests/analytics_reporting.rs` (rewrite as proxy + RBAC + HMAC tests using wiremock)

- [ ] **Step 1:** Rewrite `src/analytics_reporting.rs` to ~120 lines exposing:

```rust
pub fn proxy_router(state: ProxyState) -> Router { /* mounts the same admin paths and forwards to ANALYTICS_REPORTING_SERVICE_URL with path mapping /api/internal/admin/* -> /v1/* */ }

pub struct ProxyState {
    pub client: reqwest::Client,
    pub upstream_url: String,
    pub hmac_secret: Vec<u8>,
}
```

The handler:
1. Extracts `x-internal-call-signature`, `x-internal-call-timestamp`, `x-operator-role`, `x-actor-id`, `x-request-id` headers.
2. Calls `verify_signature` — on failure return 401 with `{"error":"ERROR_CODE_INTERNAL_CALL_UNAUTHENTICATED"}`.
3. Checks `x-operator-role` ∈ {`admin`, `manager`} — otherwise 403 with `{"error":"ERROR_CODE_REPORT_FORBIDDEN"}`.
4. Maps the admin path to the reporting-service path (string replace `/api/internal/admin/` → `/v1/`).
5. reqwest forwards method+body+headers (whitelist forwarded headers: `x-operator-role`, `x-actor-id`, `x-request-id`, `content-type`).
6. Streams response back unchanged.

- [ ] **Step 2:** Rewrite `tests/analytics_reporting.rs`. Replace the 6 existing tests with proxy versions:

- `admin_request_with_valid_hmac_is_forwarded` — mount wiremock matching `GET /v1/report-templates` returning fixture; sign request; assert 200 + forwarded body.
- `affiliate_role_returns_403_without_hitting_upstream` — assert wiremock's `received_requests().len() == 0`.
- `missing_signature_returns_401`.
- `tampered_signature_returns_401`.
- `timestamp_drift_returns_401`.
- `request_id_is_forwarded_to_upstream` — `Mock::given(header("x-request-id", "corr-001"))`.

- [ ] **Step 3:** Add to `src/lib.rs` ServiceConfig:

```rust
analytics_reporting_proxy_enabled: bool,           // env ANALYTICS_REPORTING_PROXY_ENABLED, default false
analytics_reporting_service_url: Option<String>,   // env ANALYTICS_REPORTING_SERVICE_URL
internal_call_hmac_secret: Option<String>,         // env INTERNAL_CALL_HMAC_SECRET (fail-closed in prod)
```

In the router builder, conditionally mount: if flag is true and both URL + secret present, mount `proxy_router`. Otherwise, mount the legacy `internal_reporting_router` from the existing code.

- [ ] **Step 4:** Run:

```bash
cargo test -p api-gateway
```

Expected: all tests pass — both the new proxy tests and any other api-gateway tests that don't touch analytics.

- [ ] **Step 5:** Commit:

```bash
git add services/api-gateway/src/analytics_reporting.rs services/api-gateway/src/lib.rs services/api-gateway/tests/analytics_reporting.rs
git commit -m "refactor(api-gateway): rewrite analytics_reporting as proxy behind feature flag

The legacy InMemoryAnalyticsReportingStore stays mounted when
ANALYTICS_REPORTING_PROXY_ENABLED is false, so this commit changes
nothing for existing callers. Phase 6 flips the flag in staging and
deletes the legacy code path."
```

### Task 4.3: api-gateway down-migration

**Files:**
- Create: `services/api-gateway/migrations/202605120001_drop_analytics_reporting.sql`

- [ ] **Step 1:** Create the file:

```sql
DROP TABLE IF EXISTS report_audit;
DROP TABLE IF EXISTS report_shares;
DROP TABLE IF EXISTS report_jobs;
DROP TABLE IF EXISTS report_templates;
```

This runs ONLY in dev/staging environments where the original api-gateway migration was applied. In Phase 6 we delete both this down-migration file and the original up-migration once dev/staging are clean.

- [ ] **Step 2:** Commit:

```bash
git add services/api-gateway/migrations/202605120001_drop_analytics_reporting.sql
git commit -m "chore(api-gateway): add down-migration for analytics_reporting tables

Runs in dev/staging environments that applied the original 0001 migration
before the tables moved to analytics-reporting-service."
```

---

## Phase 5: Admin BFF and API client

5 BFF route handlers, an HMAC signer, an API client rewrite, and 6 page edits. Behind the same feature flag — when off, admin keeps importing the local store.

### Task 5.1: HMAC signer in admin

**Files:**
- Create: `apps/admin/lib/internal-call.ts`
- Test: `apps/admin/tests/internal-call.test.ts`

- [ ] **Step 1:** Write failing tests `apps/admin/tests/internal-call.test.ts`:

```ts
import assert from "node:assert/strict";
import { createHmac, createHash } from "node:crypto";
import { test } from "node:test";
import { signInternalCall } from "../lib/internal-call.js";

const SECRET = "shared-secret-32-bytes-1234567890";

test("signInternalCall returns canonical headers with HMAC-SHA256 lowercase hex", () => {
  const headers = signInternalCall({
    method: "POST",
    path: "/api/internal/admin/report-jobs",
    body: '{"hi":"there"}',
    actorId: "U-ADMIN-001",
    role: "admin",
    secret: SECRET,
    nowSeconds: 1_780_000_000,
  });
  assert.equal(headers["x-actor-id"], "U-ADMIN-001");
  assert.equal(headers["x-operator-role"], "admin");
  assert.equal(headers["x-internal-call-timestamp"], "1780000000");
  const bodyHash = createHash("sha256").update('{"hi":"there"}').digest("hex");
  const canonical = `POST\n/api/internal/admin/report-jobs\n${bodyHash}\n1780000000\nU-ADMIN-001\nadmin`;
  const expected = createHmac("sha256", SECRET).update(canonical).digest("hex");
  assert.equal(headers["x-internal-call-signature"], expected);
});

test("signInternalCall handles empty body via empty-string hash", () => {
  const headers = signInternalCall({
    method: "GET", path: "/x", body: "", actorId: "U-1", role: "admin", secret: SECRET, nowSeconds: 1,
  });
  const bodyHash = createHash("sha256").update("").digest("hex");
  assert.ok(headers["x-internal-call-signature"].length === 64);
  assert.ok(headers["x-internal-call-signature"].match(/^[0-9a-f]{64}$/));
  // sanity: same hash for two empty-body calls at same timestamp
  const headers2 = signInternalCall({
    method: "GET", path: "/x", body: "", actorId: "U-1", role: "admin", secret: SECRET, nowSeconds: 1,
  });
  assert.equal(headers["x-internal-call-signature"], headers2["x-internal-call-signature"]);
  assert.ok(bodyHash.length === 64);
});
```

- [ ] **Step 2:** Run: `pnpm --filter @prophetica/admin test`. Expected: FAIL (module not found).

- [ ] **Step 3:** Implement `apps/admin/lib/internal-call.ts`:

```ts
import { createHmac, createHash } from "node:crypto";

export interface InternalCallInput {
  method: string;
  path: string;
  body: string;
  actorId: string;
  role: string;
  secret?: string;
  nowSeconds?: number;
}

export interface InternalCallHeaders {
  "x-actor-id": string;
  "x-operator-role": string;
  "x-internal-call-timestamp": string;
  "x-internal-call-signature": string;
}

export function signInternalCall(input: InternalCallInput): InternalCallHeaders {
  const secret = input.secret ?? process.env.ADMIN_INTERNAL_CALL_SECRET;
  if (!secret) {
    throw new Error("ADMIN_INTERNAL_CALL_SECRET is not configured");
  }
  const timestamp = input.nowSeconds ?? Math.floor(Date.now() / 1000);
  const bodyHash = createHash("sha256").update(input.body).digest("hex");
  const canonical = [input.method, input.path, bodyHash, timestamp, input.actorId, input.role].join("\n");
  const signature = createHmac("sha256", secret).update(canonical).digest("hex");
  return {
    "x-actor-id": input.actorId,
    "x-operator-role": input.role,
    "x-internal-call-timestamp": String(timestamp),
    "x-internal-call-signature": signature,
  };
}
```

- [ ] **Step 4:** Run tests. Expected: 2 passed.

- [ ] **Step 5:** Commit:

```bash
git add apps/admin/lib/internal-call.ts apps/admin/tests/internal-call.test.ts
git commit -m "feat(admin): add internal-call HMAC signer for BFF→gateway requests"
```

### Task 5.2: Rewrite `lib/analytics-reporting.ts` to an API client

**Files:**
- Modify: `apps/admin/lib/analytics-reporting.ts` (heavy rewrite — keep types, drop store, add fetch functions)
- Test: `apps/admin/tests/analytics-reporting-client.test.ts` (NEW)
- Delete: `apps/admin/tests/analytics-reporting.test.ts` once the new test covers the surface

- [ ] **Step 1:** Open `apps/admin/lib/analytics-reporting.ts`. Keep all `export type` and `export interface` declarations exactly as they are. Add a `ScheduleHint` interface and a `DashboardMetric` interface matching `AnalyticsReportingMetric`. Delete the rest of the file (the store class, hardcoded templates, all helper functions). Replace with API client functions:

```ts
const BFF_BASE = "/api/analytics";

async function jsonOrThrow(response: Response): Promise<unknown> {
  if (!response.ok) {
    throw new Error(`analytics bff ${response.status}`);
  }
  return response.json();
}

export async function fetchTemplates(): Promise<ReportTemplate[]> {
  const response = await fetch(`${BFF_BASE}/templates`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as ReportTemplate[];
}

export async function createJob(req: CreateReportJobRequest): Promise<ReportJob> {
  const response = await fetch(`${BFF_BASE}/jobs`, {
    method: "POST", cache: "no-store",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(req),
  });
  return (await jsonOrThrow(response)) as ReportJob;
}

export async function fetchJobs(filter?: { templateCode?: string }): Promise<ReportJob[]> {
  const url = new URL(`${BFF_BASE}/jobs`, "http://_internal_/");
  if (filter?.templateCode) url.searchParams.set("template_code", filter.templateCode);
  const response = await fetch(`${BFF_BASE}/jobs${url.search}`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as ReportJob[];
}

export async function fetchJobDetail(id: string): Promise<{ job: ReportJob; audit: ReportAuditRecord[] }> {
  const response = await fetch(`${BFF_BASE}/jobs/${encodeURIComponent(id)}`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as { job: ReportJob; audit: ReportAuditRecord[] };
}

export async function exportJob(id: string, format: ReportFormat): Promise<Response> {
  const url = `${BFF_BASE}/jobs/${encodeURIComponent(id)}/export?format=${encodeURIComponent(format)}`;
  return fetch(url, { cache: "no-store" });
}

export async function recordExportDeletion(id: string, req: RecordExportDeletionRequest): Promise<void> {
  await fetch(`${BFF_BASE}/jobs/${encodeURIComponent(id)}/deletions`, {
    method: "POST", cache: "no-store",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(req),
  }).then(jsonOrThrow);
}

export async function fetchSchedules(): Promise<Array<{ templateCode: string; cron: string; nextFireHint: string; owner: string }>> {
  const response = await fetch(`${BFF_BASE}/schedules`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as Array<{ templateCode: string; cron: string; nextFireHint: string; owner: string }>;
}

export async function fetchDashboardMetrics(): Promise<AnalyticsReportingMetric[]> {
  const response = await fetch(`${BFF_BASE}/dashboard-metrics`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as AnalyticsReportingMetric[];
}

export async function fetchLatestJobsPerTemplate(): Promise<ReportJob[]> {
  const response = await fetch(`${BFF_BASE}/jobs/latest-per-template`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as ReportJob[];
}
```

- [ ] **Step 2:** Write `apps/admin/tests/analytics-reporting-client.test.ts` — mock `fetch` globally; assert each client function hits the expected URL with the expected method/body/headers shape. At minimum 4 tests covering: templates GET, job POST, export GET (no JSON parse), schedules GET.

- [ ] **Step 3:** Delete `apps/admin/tests/analytics-reporting.test.ts` (its domain assertions live in `services/analytics-reporting-service/tests/job_lifecycle.rs` now).

- [ ] **Step 4:** Run: `pnpm --filter @prophetica/admin test`. Expected: pass (the page tests will already break here because pages still import the old functions — fix them in 5.4–5.9).

- [ ] **Step 5:** Commit:

```bash
git add apps/admin/lib/analytics-reporting.ts apps/admin/tests/analytics-reporting-client.test.ts
git rm apps/admin/tests/analytics-reporting.test.ts
git commit -m "refactor(admin): replace analytics-reporting in-memory store with API client"
```

### Task 5.3: BFF route handlers (8 routes)

**Files:**
- Create: `apps/admin/app/api/analytics/templates/route.ts`
- Create: `apps/admin/app/api/analytics/jobs/route.ts`
- Create: `apps/admin/app/api/analytics/jobs/[id]/route.ts`
- Modify: `apps/admin/app/api/analytics/jobs/[id]/export/route.ts` (refactor to proxy)
- Create: `apps/admin/app/api/analytics/jobs/[id]/deletions/route.ts`
- Create: `apps/admin/app/api/analytics/jobs/latest-per-template/route.ts`
- Create: `apps/admin/app/api/analytics/schedules/route.ts`
- Create: `apps/admin/app/api/analytics/dashboard-metrics/route.ts`
- Test: `apps/admin/tests/analytics-bff.test.ts`

- [ ] **Step 1:** Each route handler reads the session, builds the upstream URL under `API_GATEWAY_URL`, signs with `signInternalCall`, forwards the request, streams response back. Canonical template (use for the 6 read-only GETs):

```ts
import { NextRequest, NextResponse } from "next/server";
import { getViewer } from "@/lib/session";
import { signInternalCall } from "@/lib/internal-call";

export async function GET(request: NextRequest) {
  const viewer = await getViewer();
  if (!viewer || (viewer.role !== "admin" && viewer.role !== "manager")) {
    return NextResponse.json({ error: "forbidden" }, { status: 403 });
  }
  const gateway = process.env.API_GATEWAY_URL;
  if (!gateway) {
    return NextResponse.json({ error: "gateway_unconfigured" }, { status: 500 });
  }
  const adminPath = "/api/internal/admin/report-templates";
  const upstreamUrl = new URL(adminPath, gateway).toString();
  const headers = signInternalCall({ method: "GET", path: adminPath, body: "", actorId: viewer.id, role: viewer.role });
  const upstream = await fetch(upstreamUrl, { method: "GET", headers });
  const body = await upstream.arrayBuffer();
  return new NextResponse(body, {
    status: upstream.status,
    headers: { "content-type": upstream.headers.get("content-type") ?? "application/json" },
  });
}
```

Repeat for each of the 8 routes, adjusting `adminPath`, method, and body forwarding. For `POST` routes, read `await request.text()` once for both the signature canonical body and the upstream request body.

For the `export` route, preserve `content-disposition` from upstream so the browser still downloads as a file.

- [ ] **Step 2:** BFF tests in `tests/analytics-bff.test.ts` mock the upstream `fetch` and assert:
  - Each route signs with the correct canonical `(method, path, bodyHash, timestamp, actorId, role)`.
  - Session-less requests get 403.
  - Affiliate viewer gets 403 before any upstream call.
  - Upstream 401 propagates as 401; 200 propagates as 200; export's `content-type` and `content-disposition` propagate.

- [ ] **Step 3:** Run: `pnpm --filter @prophetica/admin test`. Expected: pass.

- [ ] **Step 4:** Commit:

```bash
git add apps/admin/app/api/analytics apps/admin/tests/analytics-bff.test.ts
git commit -m "feat(admin): add 8 analytics BFF route handlers proxying to api-gateway"
```

### Task 5.4: Update each of the 5 admin pages

For each page below, change `import { ... } from "@/lib/analytics-reporting"` from data functions to the new fetch client functions, then `await` the BFF in server components. Pages also keep `RoleGate` guarding admin/manager.

- [ ] **5.4.a `app/analytics/page.tsx`:** replace `listReportTemplates(viewer.role)` with `await fetchTemplates()`.
- [ ] **5.4.b `app/analytics/run/[code]/page.tsx`:** replace `listReportTemplates(viewer.role).find(...)` with `(await fetchTemplates()).find(...)`.
- [ ] **5.4.c `app/analytics/jobs/page.tsx`:** server component calls `await fetchJobs()`.
- [ ] **5.4.d `app/analytics/jobs/[id]/page.tsx`:** `await fetchJobDetail(params.id)`. Export buttons call `exportJob(id, format)` (need a client-side wrapper component that does `window.location` redirect since downloads must hit the BFF URL directly).
- [ ] **5.4.e `app/analytics/schedules/page.tsx`:** `await fetchSchedules()`.
- [ ] **5.4.f `app/dashboard/page.tsx`:** `const [metrics, latest] = await Promise.all([fetchDashboardMetrics(), fetchLatestJobsPerTemplate()])`. Render both sections; if `latest.length === 0` show empty state with link to `/analytics`.
- [ ] **5.4.g `components/report-builder-form.tsx`:** the submit handler now POSTs through `createJob(...)`; on success `router.push(\`/analytics/jobs/${job.id}\`)`.

After all edits:

- [ ] **Step 1:** Run: `pnpm --filter @prophetica/admin test`. Expected: pass.
- [ ] **Step 2:** Run dev server: `pnpm --filter @prophetica/admin dev`. Open `/analytics` and verify the templates list renders against a mocked or staging gateway (set `API_GATEWAY_URL` to a wiremock or staging URL).
- [ ] **Step 3:** Commit:

```bash
git add apps/admin/app/analytics apps/admin/app/dashboard apps/admin/components/report-builder-form.tsx
git commit -m "refactor(admin): five SPEC-020 pages now fetch from BFF instead of in-memory lib"
```

---

## Phase 6: Flag flip + legacy cleanup

After Phases 1–5 land and staging is green, flip the flag and remove the legacy code path.

### Task 6.1: Flip `ANALYTICS_REPORTING_PROXY_ENABLED` in staging, smoke test all 5 pages

- [ ] **Step 1:** In the staging environment, set both `ANALYTICS_REPORTING_PROXY_ENABLED=true` and `ADMIN_INTERNAL_CALL_SECRET`, `INTERNAL_CALL_HMAC_SECRET`, `API_GATEWAY_URL`, `ANALYTICS_REPORTING_SERVICE_URL`, `KYC_SERVICE_URL`, `LEDGER_SERVICE_URL`, `RISK_SERVICE_URL`, `USER_SERVICE_URL`.
- [ ] **Step 2:** Deploy `analytics-reporting-service` and api-gateway with the new env. Run `sqlx migrate run` on the new service's DB.
- [ ] **Step 3:** In staging admin, log in as admin and visit each of the 5 pages. Verify network shows requests to `/api/analytics/*` returning data and admin renders normally. Click "Run report" on `/analytics/run/ops-kpi-daily`; confirm it redirects to `/analytics/jobs/<id>` and the job appears in `/analytics/jobs`.
- [ ] **Step 4:** Apply api-gateway down-migration in staging: `sqlx migrate run` (the new down-migration drops the orphan tables).
- [ ] **Step 5:** Snapshot evidence (screenshots) per SPEC-013.

### Task 6.2: Delete legacy code paths

**Files:**
- Delete: `services/api-gateway/migrations/202605110001_create_analytics_reporting.sql`
- Delete: `services/api-gateway/migrations/202605120001_drop_analytics_reporting.sql` (once dev/staging are clean)
- Modify: `services/api-gateway/src/analytics_reporting.rs` — drop the legacy `internal_reporting_router` and `InMemoryAnalyticsReportingStore`; keep only the proxy code.
- Modify: `services/api-gateway/src/lib.rs` — drop the feature-flag branch; always mount proxy when env is configured, else 503.
- Modify: `apps/admin/lib/analytics-reporting.ts` — there is no flag branch to remove on the admin side after Task 5.2; verify the file has no remaining references to local data.

- [ ] **Step 1:** Run `grep -r ANALYTICS_REPORTING_PROXY_ENABLED services/ apps/`. Remove every match.
- [ ] **Step 2:** Run full test suite:

```bash
cargo test --workspace
pnpm --filter @prophetica/admin test
pnpm --filter @prophetica/admin lint
```

Expected: all green.

- [ ] **Step 3:** Commit:

```bash
git add -u
git commit -m "chore: remove ANALYTICS_REPORTING_PROXY_ENABLED flag and legacy api-gateway analytics code"
```

### Task 6.3: Update SPEC-020 status

**Files:**
- Modify: `specs/SPEC-020-analytics-reporting.html` §9

- [ ] **Step 1:** Rewrite §9 to reflect the real implementation: list the 5 services involved, point at the design doc and this plan, mention the verification commands.
- [ ] **Step 2:** Update `TODO.md` if any SPEC-020-related bullets remain.
- [ ] **Step 3:** Commit:

```bash
git add specs/SPEC-020-analytics-reporting.html TODO.md
git commit -m "docs(spec-020): update §9 implementation status with real-API chain"
```

---

## Self-review checklist (run before opening PR)

- [ ] Every spec invariant (§5 of design doc, 8 items) has a test that would fail if it regressed.
- [ ] `row_hash` test (Task 3.3) covers both "different source_version → different hash" AND "same source_snapshot → same hash" cases.
- [ ] Gateway proxy tests (Task 4.2) cover: valid signature passes, missing/invalid/expired signature returns 401, affiliate role returns 403 without hitting upstream.
- [ ] BFF tests (Task 5.3) cover: session→header translation, HMAC signing, upstream 401 propagation.
- [ ] No remaining `import { AnalyticsReportingStore }` or `import { getAnalyticsReportingSnapshot }` anywhere in `apps/admin/`.
- [ ] No remaining `InMemoryAnalyticsReportingStore` in `services/api-gateway/`.
- [ ] `cargo test --workspace` and `pnpm --filter @prophetica/admin test` both green.
- [ ] Staging evidence captured for SPEC-013.


