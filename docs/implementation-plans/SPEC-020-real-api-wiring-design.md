# SPEC-020 Real API Wiring — Design

- Date: 2026-05-11
- Author: feat/background-sync-order-queue session (brainstormed with user)
- Related: [SPEC-020](../../../specs/SPEC-020-analytics-reporting.html), [SPEC-020 implementation plan](../../implementation-plans/SPEC-020-analytics-reporting-implementation-plan.html)
- Status: Design, awaiting user approval before implementation plan

## 1. Context

SPEC-020 §9 declares the analytics & reporting surface "landed" in `apps/admin/lib/analytics-reporting.ts` and `apps/admin/app/dashboard/page.tsx`. Read end-to-end, today's reality is:

- **Admin frontend (5 pages)** — `/dashboard`, `/analytics`, `/analytics/run/[code]`, `/analytics/jobs`, `/analytics/jobs/[id]`, `/analytics/schedules` — directly import an in-memory `AnalyticsReportingStore` from `lib/analytics-reporting.ts` with hardcoded sample rows. No HTTP is involved.
- **api-gateway** has a ~717-line `src/analytics_reporting.rs` plus 6 integration tests and a SQL migration (`migrations/202605110001_create_analytics_reporting.sql`) for 4 tables. The HTTP surface (`/api/internal/admin/report-templates`, `report-jobs[/:id/exports/:format | /export-deletions]`) exists and tests are green, but storage is `InMemoryAnalyticsReportingStore`. The migration is not wired to the Rust code. Admin frontend never calls this surface.
- **No backend service** for analytics-reporting exists.

The user has asked to wire all 5 admin pages to real APIs. This spec defines what "real" means for v1, where the boundary lives, and how the 4 PG tables get owned by a dedicated service.

## 2. Goals

- Admin pages no longer hold business data in-process; every page reads/writes through HTTP.
- A dedicated `analytics-reporting-service` owns the 4 PG tables and the report-job lifecycle.
- Job rows reflect *real* aggregated state pulled from source services (`kyc-service`, `wallet-ledger-service`, `risk-antifraud-service`, `user-service`) via internal HTTP endpoints, not hardcoded literals.
- `row_hash` plus `source_snapshot` make every report reproducible and auditable per SPEC-020 §5.
- api-gateway remains the single entry point for admin, performs coarse RBAC, and validates an internal-call HMAC token from the admin BFF.

## 3. Non-goals (Phase 3)

- Real cron / scheduler. `/analytics/schedules` reads template-derived placeholders; no execution.
- Plaintext PII reveal flow. `user-service /v1/internal/users/contacts:reveal` returns 501 + audit placeholder; UI surfaces the constraint.
- S3 / R2 export storage. Exports stream from the in-DB rows; `output_uri` stays null.
- Cross-service event streaming / CDC. Source services expose pull-style endpoints only.

## 4. Architecture

```
Browser
  │ session cookie (existing Ed25519/HMAC-signed)
  ▼
apps/admin (Next.js, 5 server components)
  │ awaits BFF route handlers in the same process
  ▼
apps/admin/app/api/analytics/*  (Next.js route handlers)
  │ - reads session → viewer.role + actor_id
  │ - signs request with HMAC(ADMIN_INTERNAL_CALL_SECRET) over
  │   (method, path, body_sha256, timestamp, actor_id, role)
  │ - attaches x-operator-role, x-actor-id, x-internal-call-signature,
  │   x-internal-call-timestamp, x-request-id headers
  ▼
services/api-gateway/src/analytics_reporting.rs  (proxy, ~120 lines)
  │ - validates x-internal-call-signature against INTERNAL_CALL_HMAC_SECRET
  │   (5-minute window via timestamp)
  │ - rejects timestamp drift or signature mismatch with 401
  │ - coarse RBAC: admin/manager pass through; affiliate → 403
  │ - reqwest forward to analytics-reporting-service, propagating
  │   x-actor-id, x-operator-role, x-request-id
  ▼
services/analytics-reporting-service  (NEW)
  │ - owns 4 PG tables (migration moved from api-gateway)
  │ - on create_report_job: load template, call source endpoint(s),
  │   project/sort/mask, compose source_snapshot, compute row_hash,
  │   insert report_jobs + report_audit
  │ - exports stream from in-DB rows
  ├─ HTTP → kyc-service           /v1/internal/reports/kyc-pass-rate
  ├─ HTTP → wallet-ledger-service /v1/internal/reports/reconciliation-snapshot
  ├─ HTTP → risk-antifraud-service /v1/internal/reports/aml-alerts
  └─ HTTP → user-service          /v1/internal/users/contacts:lookup-masked
                                  /v1/internal/users/contacts:reveal  (v1: 501 + audit)
```

## 5. Invariants

These properties must hold across all changes. Each is a test target.

1. **Gateway never accepts an unsigned admin call.** Missing or invalid `x-internal-call-signature` returns 401 before any role check. Timestamp older than 5 minutes returns 401.
2. **Gateway enforces coarse RBAC.** `x-operator-role: affiliate` returns 403 from the proxy regardless of downstream policy. Unknown roles also return 403.
3. **Plaintext PII never crosses the reporting-service boundary by default.** `user-service /contacts:lookup-masked` returns only masked strings. The `:reveal` path is the only route that returns plaintext, and v1 returns 501.
4. **Source masking is unified.** `mask_email` lives in `sdk/src/masking.rs` and is imported by user-service (and any other masker). Source services do not redefine masking.
5. **`row_hash` includes `source_snapshot`.** Canonical JSON for hashing is `{ template_code, params, source_snapshot, rows }`. Two jobs with same template + params but different `source_version` produce different hashes.
6. **Every job records source provenance.** `report_jobs.source_snapshot` is non-null and contains at minimum `source_service`, `source_version`, `cutoff_at`, `source_generated_at`, `correlation_id`. The `latest-per-template` endpoint surfaces this on each returned `ReportJob`; the four dashboard-metrics cards (KPI / alerts / versioned-reports / audit-coverage) carry their own freshness timestamp but do not duplicate per-template source_snapshot.
7. **Dashboard never creates jobs.** `GET /v1/dashboard-metrics` and `GET /v1/report-jobs/latest-per-template` are pure reads. Job creation requires explicit user action on `/analytics/run/[code]`.
8. **Schedules are template-derived in v1.** No `report_schedules` table. `GET /v1/report-schedules` synthesizes from `report_templates.definition.schedule`. Cron strings parse at service startup (fail-fast).

## 6. Component breakdown

### 6.1 `services/analytics-reporting-service` (NEW)

- `Cargo.toml` — depends on `sdk`, `axum`, `sqlx`, `reqwest`, `serde`, `sha2`, `tokio`, `tracing`
- `src/main.rs` — service entry; ServiceConfig (`DATABASE_URL`, `KYC_SERVICE_URL`, `LEDGER_SERVICE_URL`, `RISK_SERVICE_URL`, `USER_SERVICE_URL`, `INTERNAL_CALL_HMAC_SECRET`); tracing init; sqlx pool; mount router
- `src/router.rs` — axum routes:
  - `GET /v1/report-templates`
  - `POST /v1/report-jobs`
  - `GET /v1/report-jobs` (list, with optional `template_code` filter)
  - `GET /v1/report-jobs/:id`
  - `GET /v1/report-jobs/:id/exports/:format`
  - `POST /v1/report-jobs/:id/export-deletions`
  - `GET /v1/report-schedules` (template-derived)
  - `GET /v1/dashboard-metrics` (pure read aggregator)
  - `GET /v1/report-jobs/latest-per-template`
- `src/job_lifecycle.rs` — pure domain (no DB, no HTTP):
  - `validate(template, request)`
  - `sort_rows`, `project_rows`, `mask_sensitive_value` (calls `sdk::masking`)
  - `compose_source_snapshot(source_responses) -> SourceSnapshot`
  - `row_hash({ template_code, params, source_snapshot, rows })`
- `src/store/mod.rs` — `Store` trait: `templates_for_role`, `insert_job`, `find_job`, `list_jobs`, `latest_per_template`, `insert_audit`, `list_audit_for_job`
- `src/store/pg.rs` — sqlx implementation
- `src/templates.rs` — 3 seeded `ReportTemplate` definitions; `definition` JSONB now carries `schedule: { cron, owner, next_fire_hint }` sub-field
- `src/exports.rs` — CSV/JSON/XLSX generators (ported from api-gateway, header now also writes `source_snapshot` summary)
- `src/dashboard_metrics.rs` — on-the-fly aggregation: KPI from latest ledger snapshot, alerts count from latest risk pull, versioned-reports count from `report_jobs` over 24h, audit coverage from `report_audit`
- `src/sources/mod.rs` — `Source` trait
- `src/sources/kyc.rs`, `ledger.rs`, `risk.rs`, `users.rs` — reqwest clients; each injects `x-request-id` and an internal service token
- `src/error.rs` — `ReportingError` with `IntoResponse`
- `migrations/202605110001_create_analytics_reporting.sql` — copied from api-gateway, **with one schema change**: add `source_snapshot JSONB NOT NULL DEFAULT '{}'` to `report_jobs`

### 6.2 `services/api-gateway` (modified)

- `src/analytics_reporting.rs` — replace 717-line in-memory implementation with proxy module (~120 lines):
  - validates `x-internal-call-signature`: HMAC-SHA256, lowercase hex output, computed over the canonical string `"{method}\n{path}\n{body_sha256_hex}\n{timestamp}\n{actor_id}\n{role}"` using `INTERNAL_CALL_HMAC_SECRET`. `timestamp` is Unix epoch seconds as decimal text; reject if `|now - timestamp| > 300`
  - performs coarse RBAC against `x-operator-role`
  - reqwest forwards to `ANALYTICS_REPORTING_SERVICE_URL` preserving method, path under `/api/internal/admin/*` mapped to `/v1/*`, body, and propagated headers
  - streams response back unchanged
- `src/lib.rs` — adds `ANALYTICS_REPORTING_SERVICE_URL` and `INTERNAL_CALL_HMAC_SECRET` to ServiceConfig; mounts proxy module unchanged at same surface
- `migrations/202605110001_create_analytics_reporting.sql` — **delete**, accompanied by `migrations/202605120001_drop_analytics_reporting.sql` (down-migration to drop 4 tables in api-gateway's PG so dev/staging environments that ran the original migration end up clean)
- `tests/analytics_reporting.rs` — rewrite all 6 tests as proxy tests using `wiremock` to simulate analytics-reporting-service. Assertions move from "store side effects" to "forwarded URL, headers, body" + RBAC + HMAC rejection

### 6.3 Source services (additions only)

- `services/kyc-service/src/...` — add handler `GET /v1/internal/reports/kyc-pass-rate?from=&to=`. Returns aggregate over `user_kyc_records` (counts by status), plus `source_snapshot { source_service: "kyc-service", source_version, cutoff_at, source_generated_at, correlation_id }`
- `services/wallet-ledger-service/src/...` — add handler `GET /v1/internal/reports/reconciliation-snapshot?cutoff=`. Returns rows derived from `reconciliation_runs` + `reconciliation_findings`, plus source_snapshot
- `services/risk-antifraud-service/src/...` — add handler `GET /v1/internal/reports/aml-alerts?from=&to=`. ⚠ This service has no migrations directory; **before implementation begins, decide whether the data source is (a) kyc-service `aml_events` via internal HTTP, (b) a new migration in risk-antifraud-service, or (c) stub aggregate returned from in-memory state**. This decision must be made in the implementation plan, not in this spec.
- `services/user-service/src/...`:
  - `POST /v1/internal/users/contacts:lookup-masked` — body `{ ids: string[] }` → `{ contacts: [{ id, email_masked }] }`. Implements `sdk::masking::mask_email`. Returns 400 if any `id` is empty.
  - `POST /v1/internal/users/contacts:reveal` — body `{ ids, actor_id, second_authorization_id, job_id }` → v1 returns 501 `{ "error": "ERROR_CODE_REVEAL_NOT_AVAILABLE" }` and writes an audit row recording the attempt (actor_id, job_id, ids[].len, second_authorization_id). Reporting-service catches the 501 and propagates it upstream unchanged so the admin UI surfaces the same status code; reporting-service also writes its own `sensitive_fields_revealed_denied` audit row.

### 6.4 Shared `sdk` crate

- `sdk/src/masking.rs` — `pub fn mask_email(value: &str) -> String` matching the existing logic (first char + `***` + `@domain` for email-shaped values, `****` otherwise). Re-exported via `sdk::masking`. The `sdk` crate is confirmed present: `services/api-gateway/src/lib.rs` already imports `use sdk::{config::ServiceConfig, observability, service_stub};`.

### 6.5 Admin frontend

- `apps/admin/lib/analytics-reporting.ts` — heavy rewrite:
  - **Drop**: `AnalyticsReportingStore`, hardcoded `reportTemplates`, `getAnalyticsReportingSnapshot`, `exportReportJob`, `createDefaultAnalyticsReportingStore`, all helpers below them (sort/project/mask/CSV/XLSX/zip/crc32/canonical-json/sha256)
  - **Keep**: all exported types (`ReportFormat`, `ReportTimezone`, `ReportSortDirection`, `RetentionCategory`, `ReportCell`, `ReportRow`, `ReportFieldDefinition`, `ReportRetentionPolicy`, `ReportTemplate`, `ReportParams`, `CreateReportJobRequest`, `ReportJob`, `ReportAuditRecord`, `RecordExportDeletionRequest`, `ReportExportPayload`, `AnalyticsReportingMetric`, `AnalyticsReportingSnapshot`)
  - **Add**: API client functions calling admin BFF routes:
    - `fetchTemplates(): Promise<ReportTemplate[]>`
    - `createJob(req: CreateReportJobRequest): Promise<ReportJob>`
    - `fetchJobs(filter?: { templateCode?: string }): Promise<ReportJob[]>`
    - `fetchJobDetail(id: string): Promise<{ job: ReportJob; audit: ReportAuditRecord[] }>`
    - `exportJob(id: string, format: ReportFormat): Promise<Response>` (raw Response for streaming download)
    - `recordExportDeletion(id: string, req: RecordExportDeletionRequest): Promise<void>`
    - `fetchSchedules(): Promise<{ templateCode: string; cron: string; nextFireHint: string; owner: string }[]>`
    - `fetchDashboardMetrics(): Promise<AnalyticsReportingMetric[]>`
    - `fetchLatestJobsPerTemplate(): Promise<ReportJob[]>`
- `apps/admin/lib/internal-call.ts` (NEW) — HMAC signer for outbound BFF→gateway calls. Same algorithm as §6.2 (HMAC-SHA256, lowercase hex, canonical string `"{method}\n{path}\n{body_sha256_hex}\n{timestamp}\n{actor_id}\n{role}"`, timestamp = `Math.floor(Date.now() / 1000)`). Reads `ADMIN_INTERNAL_CALL_SECRET` env (fail-closed: throws in production if unset, no dev fallback). Exports `signInternalCall(method, path, body, actor) -> { headers }`
- `apps/admin/app/api/analytics/templates/route.ts` (NEW) — GET; forwards to gateway
- `apps/admin/app/api/analytics/jobs/route.ts` (NEW) — GET (list) + POST (create)
- `apps/admin/app/api/analytics/jobs/[id]/route.ts` (NEW) — GET (detail)
- `apps/admin/app/api/analytics/jobs/[id]/export/route.ts` (MODIFY existing) — drop local snapshot regeneration; proxy to gateway `/exports/:format`; stream response back to browser preserving `content-type` and `content-disposition`
- `apps/admin/app/api/analytics/jobs/[id]/deletions/route.ts` (NEW) — POST
- `apps/admin/app/api/analytics/schedules/route.ts` (NEW) — GET
- `apps/admin/app/api/analytics/dashboard-metrics/route.ts` (NEW) — GET
- `apps/admin/app/api/analytics/jobs/latest-per-template/route.ts` (NEW) — GET
- All 5 admin pages (`dashboard/page.tsx`, `analytics/page.tsx`, `analytics/run/[code]/page.tsx`, `analytics/jobs/page.tsx`, `analytics/jobs/[id]/page.tsx`, `analytics/schedules/page.tsx`) — server components `await` BFF via the API client. `dashboard/page.tsx` does `Promise.all([fetchDashboardMetrics(), fetchLatestJobsPerTemplate()])`
- `apps/admin/components/report-builder-form.tsx` — submit handler POSTs to `/api/analytics/jobs` instead of in-process call

## 7. Data flow

### 7.1 Job creation (user clicks Run on `/analytics/run/[code]`)

1. `ReportBuilderForm` submits `POST /api/analytics/jobs` with `templateCode`, `params`, optional `revealSensitiveFields`/`secondAuthorizationId`
2. Admin BFF reads session → `viewer.role`, `viewer.id`; computes HMAC; forwards `POST /api/internal/admin/report-jobs` to gateway with headers
3. Gateway validates HMAC + role; reqwest → `POST /v1/report-jobs` on reporting-service
4. Reporting-service:
   - loads template from `report_templates`
   - validates role/template visibility/sort field/columns/sensitive reveal authorization
   - resolves data: calls source(s) by `template_code`:
     - `ops-kpi-daily` → kyc-service kyc-pass-rate + ledger reconciliation-snapshot (for uptime/approvals proxy)
     - `risk-alert-summary` → risk-antifraud-service aml-alerts + user-service contacts:lookup-masked
     - `finance-reconciliation` → wallet-ledger-service reconciliation-snapshot
   - composes `source_snapshot` from response headers (`source_version`, `source_generated_at`, `cutoff_at`) + own correlation_id
   - sorts/projects/masks rows; if reveal active and role==admin and second_auth_id present, calls user-service `:reveal` (v1 → 501 → reporting-service writes `sensitive_fields_revealed_denied` audit and propagates 501 upstream)
   - `row_hash = sha256(canonical_json({ template_code, params, source_snapshot, rows }))`
   - INSERT `report_jobs` (with `source_snapshot`); INSERT `report_audit` (action=`report_job_created` or `report_job_rerun`); if reveal succeeded, INSERT additional audit row `sensitive_fields_revealed`
5. Response flows back; admin BFF returns job; page redirects to `/analytics/jobs/[id]`

### 7.2 Dashboard render

1. `dashboard/page.tsx` (server component) `await Promise.all([ fetchDashboardMetrics(), fetchLatestJobsPerTemplate() ])`
2. Both BFF routes forward to gateway → reporting-service
3. `dashboard-metrics` returns 4 cards computed live (no DB write)
4. `latest-per-template` returns at most one `ReportJob` per active template, ordered by `generated_at desc`
5. Page renders both. If `latest-per-template` is empty, show "尚未產生報表" empty state with link to `/analytics`

### 7.3 Export download

1. User clicks CSV/JSON/XLSX button on `/analytics/jobs/[id]`
2. Browser hits `GET /api/analytics/jobs/[id]/export?format=csv`
3. BFF signs HMAC, forwards to gateway `/api/internal/admin/report-jobs/:id/exports/:format`
4. Gateway proxies to reporting-service `/v1/report-jobs/:id/exports/:format`
5. Reporting-service loads job rows + source_snapshot from PG; generates bytes; streams response
6. Gateway and BFF stream pass-through; `content-disposition: attachment; filename="rpt-job-XXXXXX.csv"`

## 8. PG schema (analytics-reporting-service)

```sql
CREATE TABLE report_templates (
  id BIGSERIAL PRIMARY KEY,
  code TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  category TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  definition JSONB NOT NULL,  -- includes fields, viewer_roles, retention, schedule
  created_by TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  is_active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE report_jobs (
  id BIGSERIAL PRIMARY KEY,
  public_id TEXT NOT NULL UNIQUE,                -- e.g. rpt-job-000001
  template_id BIGINT NOT NULL REFERENCES report_templates(id),
  requested_by TEXT NOT NULL,
  params JSONB NOT NULL DEFAULT '{}',
  source_snapshot JSONB NOT NULL DEFAULT '{}',   -- NEW vs api-gateway migration
  state TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  report_version TEXT NOT NULL,                  -- e.g. v2026.05.10.1
  started_at TIMESTAMPTZ,
  finished_at TIMESTAMPTZ,
  generated_at TIMESTAMPTZ NOT NULL,
  output_uri TEXT,                               -- NULL in v1 (no S3)
  output_rows BIGINT NOT NULL DEFAULT 0,
  row_hash CHAR(71) NOT NULL,                    -- sha256:<64 hex>
  retention_category TEXT NOT NULL,
  retention_expires_at TIMESTAMPTZ NOT NULL,
  error_message TEXT
);

CREATE TABLE report_shares (
  id BIGSERIAL PRIMARY KEY,
  report_job_id BIGINT NOT NULL REFERENCES report_jobs(id),
  shared_to TEXT NOT NULL,
  shared_scope TEXT NOT NULL,
  share_type TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  expires_at TIMESTAMPTZ
);

CREATE TABLE report_audit (
  id BIGSERIAL PRIMARY KEY,
  report_job_id BIGINT NOT NULL REFERENCES report_jobs(id),
  actor_id TEXT NOT NULL,
  action TEXT NOT NULL,                          -- report_job_created | report_job_rerun | sensitive_fields_revealed | report_export_deleted
  action_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  ip INET,
  user_agent TEXT,
  details JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX report_jobs_template_id_idx ON report_jobs(template_id);
CREATE INDEX report_jobs_requested_by_idx ON report_jobs(requested_by);
CREATE INDEX report_jobs_template_generated_idx ON report_jobs(template_id, generated_at DESC);  -- supports latest-per-template
CREATE INDEX report_audit_report_job_id_idx ON report_audit(report_job_id);
CREATE INDEX report_audit_actor_id_idx ON report_audit(actor_id);
```

## 9. Test strategy

| Layer | Location | DB? | Other services? |
|---|---|---|---|
| Domain | `analytics-reporting-service/tests/job_lifecycle.rs` | No | No |
| Repository | `analytics-reporting-service/tests/repository.rs` | sqlx + PG via `#[cfg(feature = "pg-tests")]` | No |
| Service router + sources | `analytics-reporting-service/tests/router_with_mock_sources.rs` | In-memory store | wiremock for 4 source services |
| Dashboard metrics | `analytics-reporting-service/tests/dashboard_metrics.rs` | No (or in-memory) | wiremock |
| Gateway proxy | `services/api-gateway/tests/analytics_reporting.rs` (rewritten) | No | wiremock for reporting-service |
| Source endpoints | each source `tests/internal_reports.rs` | each service's existing pattern | No |
| Admin API client | `apps/admin/tests/analytics-reporting-client.test.ts` | No | mock `fetch` |
| Admin BFF route | `apps/admin/tests/analytics-bff.test.ts` | No | mock fetch to gateway; assert session→header translation + HMAC signing |
| Admin page render | `apps/admin/tests/analytics-page.test.ts` | No | mock BFF |

The 6 existing domain assertions in api-gateway port directly to `analytics-reporting-service/tests/job_lifecycle.rs`:
1. admin/manager visibility; affiliate forbidden
2. job carries fixed version + timestamp + hash + retention + audit
3. sensitive fields masked by default; reveal requires admin + second_auth (v1: reveal returns 501 from user-service; reporting-service surfaces 422)
4. CSV/JSON/XLSX export deterministic, schema metadata header
5. rerun preserves original job and reproduces row_hash (with same source_snapshot)
6. legal retention 7y + deletion audit chain

The existing 6 admin TS tests in `apps/admin/tests/analytics-reporting.test.ts` retire — their domain logic now lives in Rust. New TS tests cover only API client shape and BFF translation.

**CI gates**: domain + proxy + BFF + page tests run on every PR. Repository tests run only when `PROPHETICA_PG_TESTS=1` is set (CI provides Postgres). `router_with_mock_sources` runs everywhere (no external dep).

**Windows local workflow**: repository tests skip by default (the sqlx_macros linker issue observed on Windows 1.85 toolchain blocks the full cargo test suite). Domain + proxy + router + dashboard tests all build without `sqlx_macros`.

## 10. Migration / rollout sequence

Proxy rollout mechanism: api-gateway always mounts the analytics admin surface. When both `ANALYTICS_REPORTING_SERVICE_URL` and `INTERNAL_CALL_HMAC_SECRET` are configured, it forwards through the proxy to analytics-reporting-service; when either env var is missing, the same surface fails closed with 503. Admin uses the BFF/API-client chain, so rollout is controlled by configuring the service URL and shared HMAC secret, not by a separate build-time switch.

1. Add `sdk::masking` (one PR; trivially safe).
2. Add source endpoints in kyc / ledger / risk / user services (one PR per service). These are additive and have no flag.
3. Build `analytics-reporting-service` with PG migration; do not configure api-gateway to forward yet.
4. Add api-gateway proxy module with env-gated config. With URL + secret present, the proxy module is active; otherwise the unconfigured router returns 503. Both active and unconfigured paths have tests.
5. Add admin BFF routes + API client for the real api-gateway path.
6. Configure proxy/admin env in staging; verify all 5 pages render with real data.
7. Run api-gateway down-migration (`drop_analytics_reporting.sql`) in dev/staging.
8. Delete any remaining api-gateway in-memory implementation, old api-gateway migration file, and admin hardcoded data path.
9. Update `specs/SPEC-020-analytics-reporting.html` §9.

## 11. Open items for the implementation plan

These need decisions before or during planning, not in this spec:

- **risk-antifraud-service data source.** Service has no migrations directory. Options: (a) call kyc-service `aml_events` via internal HTTP; (b) add a migration to risk-antifraud-service; (c) v1 returns a stub aggregate from in-memory hardcoded rows but with real `source_snapshot` metadata. Recommended: (a), since `aml_events` already lives in kyc-service per `202605060001_create_kyc.sql:41-51`.
- **user-service `:reveal` audit destination.** If user-service does not have an audit table today, decide whether to add one or write to a service-shared audit channel.
- **`ADMIN_INTERNAL_CALL_SECRET` rotation policy.** Single value vs key-id-tagged; rotation runbook out of scope but the implementation should not preclude rotation.
- **Per-source timeout / retry budget.** Each source call needs a deadline (default 2s? configurable?). reporting-service total deadline should not exceed admin page render budget.
- **schedule cron validation library.** Pick one Rust cron parser (`cron` crate is common). Fail-fast at startup if any active template has an unparseable cron.

## 12. Acceptance for this spec

- All 5 admin pages render with data from PG (via the full chain).
- Creating a report on `/analytics/run/[code]` results in a new row in `report_jobs` and at least one row in `report_audit`.
- Two consecutive runs of the same template+params produce identical `row_hash` **iff** the underlying source_snapshot is identical.
- Affiliate role hitting any of the 5 pages or any BFF route gets 403; same role hitting api-gateway directly with the right HMAC also gets 403 from the proxy.
- Tampered HMAC or 6-minute-old timestamp gets 401 from the gateway.
- Dashboard page never writes to `report_jobs` (verified by integration test asserting row count is stable across N renders).
