# SPEC-022 Completion Audit

Last updated: 2026-05-18

Objective: implement `specs/SPEC-022-login-event-intelligence.html` according
to `docs/implementation-plans/SPEC-022-login-event-intelligence-implementation-plan.html`
until login event intelligence can be formally declared complete.

This audit is intentionally strict. An item is complete only when there is a
concrete artifact plus validation or operational evidence. Passing tests are
accepted only when the tested surface covers the requirement.

Machine-readable evidence summary: `docs/spec-022-evidence-manifest.json`.
Formal completion gates: `tools/scripts/finalize-spec-022-complete.mjs` and
`tools/scripts/check-spec-022-complete.mjs`.
External evidence runbook: `docs/evidence/spec-022/README.md`.

## Checklist

| SPEC-022 requirement | Implementation evidence | Validation / evidence | Status |
| --- | --- | --- | --- |
| `login_events` data model preserves principal, surface, IP, geo, device, auth method, failure reason, risk flags, and retention/audit metadata | `services/user-service/migrations/202605180001_create_login_events.sql`, `services/user-service/src/login_events.rs` | `USER_SERVICE_TEST_DATABASE_URL=postgres://proph:dev@localhost:5432/prophetica cargo test -p user-service --test login_event_migrations --locked`: 1 passed | Complete |
| Outbox / fail-open path exists when primary event write fails | `services/user-service/migrations/202605180002_create_login_event_outbox.sql`, `services/user-service/src/login_events.rs`, `services/user-service/src/auth.rs` | `cargo test -p user-service --locked`: includes outbox fail-open metrics coverage | Complete |
| User/admin/manager/affiliate password login success and failure write events through one contract | `services/user-service/src/auth.rs`, `services/user-service/tests/auth.rs`, admin/web BFF forwarding code | `cargo test -p user-service --locked`: 48 passed, 1 ignored; admin/web tests recorded in manifest | Complete |
| Unknown account failure keeps `login_surface`, attempted principal type, and identifier HMAC/key version | `services/user-service/src/auth.rs`, `services/user-service/src/login_events.rs` | `login_events_capture_success_failure_and_surface_identity` and migration CHECK coverage | Complete |
| Affiliate account id is stored in `principal_external_id`, not `risk_flags` | `services/user-service/src/auth.rs`, `services/user-service/tests/auth.rs` | `login_events_capture_success_failure_and_surface_identity`, `admin_login_events_query_filters_by_principal_ip_event_and_time` | Complete |
| LIFF exchanges LINE identity for a `user-service` session and records `login_surface = liff` | `apps/liff/components/liff-bootstrap.tsx`, `apps/liff/app/api/auth/liff/session/route.ts`, `services/user-service/src/auth.rs`, `services/_binaries/core-api/src/main.rs` | `cargo test -p user-service --test auth liff_session_exchange --locked`: 2 passed; `pnpm --filter @prophetica/liff lint`: passed | Code complete; live evidence blocked |
| LIFF linked-account lookup uses migrated Postgres `user_members.line_id` and ignores soft-deleted members | `services/user-service/src/auth.rs`, `services/user-service/tests/liff_linked_account_store.rs` | `USER_SERVICE_TEST_DATABASE_URL=postgres://proph:dev@localhost:5432/prophetica cargo test -p user-service --test liff_linked_account_store --locked`: 1 passed | Complete |
| Client IP resolution trusts only configured proxy / LB headers and records `ip_source` | `services/user-service/src/login_events.rs` | Login event unit tests cover Cloudflare, XFF, X-Real-IP, remote fallback, and spoofed header ignore behavior | Complete |
| Geolocation provider failure / timeout does not block login | `services/user-service/src/auth.rs`, `services/user-service/src/login_events.rs` | `geolocation_provider_timeout_does_not_block_login_or_event_query` | Complete |
| User-Agent parsing failure does not block event write and falls back to unknown/null values | `services/user-service/src/login_events.rs`, self/admin response helpers | `self_login_events_query_masks_ip_and_uses_unknown_fallbacks` and parser fallback tests | Complete |
| Privacy and access control: self query masks IP/raw UA; admin full detail is RBAC protected and audited | `services/user-service/src/auth.rs`, `apps/web/app/account/security/page.tsx`, `apps/admin/app/security/login-events/page.tsx`, `apps/admin/lib/rbac.ts` | `admin_login_event_detail_is_rbac_protected_and_audited`, `self_login_events_query_masks_ip_and_uses_unknown_fallbacks`, frontend tests listed in manifest | Complete |
| Admin query supports principal type/id, user, IP, time range, and event type filters | `services/user-service/src/auth.rs`, `apps/admin/lib/login-events-bff.ts` | `admin_login_events_query_filters_by_principal_ip_event_and_time` | Complete |
| Retention dry-run and delete path exists | `services/user-service/src/auth.rs`, `services/user-service/src/login_events.rs` | `admin_login_event_retention_supports_dry_run_and_delete` | Complete |
| Metrics and alerts exist for write failures, outbox backlog, spoofed headers, geo failures, and admin detail reads | `services/user-service/src/auth.rs`, `infra/prometheus/rules/login-events.yml` | `login_event_metrics_count_writes_and_spoofed_headers`, `admin_login_event_detail_is_rbac_protected_and_audited`, outbox fail-open test | Complete |
| Login event write plus geolocation does not add more than 100ms P95 in local evidence run | `services/user-service/tests/auth.rs` ignored explicit NFR test | `cargo test -p user-service --test auth spec_022_login_event_latency_p95_local_budget --locked -- --ignored`: 1 passed | Complete for local evidence; staging/live evidence optional unless required by release gate |
| UI uses approximate location wording and `未知` fallback | `apps/web/lib/login-events.ts`, `apps/web/app/account/security/page.tsx`, `apps/admin/app/security/login-events/page.tsx` | Unit/source tests recorded in manifest; live/manual QA evidence still required by runbook | Code complete; manual QA blocked |
| Evidence gate prevents premature completion | `docs/spec-022-evidence-manifest.json`, `docs/evidence/spec-022/liff-log.example`, `docs/evidence/spec-022/manual-qa-log.example`, `tools/scripts/preflight-spec-022-evidence.mjs`, `tools/scripts/finalize-spec-022-complete.mjs`, `tools/scripts/check-spec-022-complete.mjs`, evidence recorders/tests | `node --test tools/scripts/*.test.mjs`: 17 passed; recorder scripts reject the `false` marker templates and missing input logs; preflight reports missing env/log/evidence prerequisites and evidence fields that are not true; `node tools/scripts/finalize-spec-022-complete.mjs`: restores the blocked manifest and exits 1 while external evidence is missing; `node tools/scripts/check-spec-022-complete.mjs`: fails closed while blockers remain | Complete |

## Formal Declaration Blockers

SPEC-022 cannot be formally declared complete until these external evidence
items exist and the manifest is updated to `status = complete` with no blocking
items:

### 待辦：Current Blocking Gap

The local implementation, tests, docs, and evidence gates are complete. The
remaining gap is external proof from a configured LINE/LIFF environment and a
manual QA run:

- Missing target environment configuration:
  - `NEXT_PUBLIC_LIFF_ID`
  - `USER_SERVICE_LINE_LOGIN_CHANNEL_ID`
  - `USER_SERVICE_BASE_URL`
- Missing input marker logs:
  - `.tmp/spec-022-liff.log`
  - `.tmp/spec-022-manual-qa.log`
- Missing generated evidence artifacts:
  - `docs/evidence/spec-022-liff/latest.json`
  - `docs/evidence/spec-022-manual-qa/latest.json`
- Missing live LIFF proof:
  - A target player linked through `user_members.line_id`.
  - iOS and Android LINE LIFF login runs.
  - `login_success` events recorded with `login_surface = liff`.
- Missing manual QA proof:
  - Desktop and mobile self-service recent-login pages show approximate
    location wording.
  - Admin login-events RBAC, audit, query, and LIFF/WebView visibility are
    checked.
  - Unknown location fallback renders `未知` instead of blank fields.

1. `docs/evidence/spec-022-liff/latest.json`
   - Real LINE Login / LIFF channel configured.
   - `USER_SERVICE_LINE_LOGIN_CHANNEL_ID` set in the target environment.
   - A player linked through `user_members.line_id`.
   - iOS and Android LIFF login runs produce `login_success` events.
   - Stored events confirm `login_surface = liff`.

2. `docs/evidence/spec-022-manual-qa/latest.json`
   - Desktop web recent-login page checked.
   - Mobile web recent-login page checked.
   - Admin login-events RBAC and audit path checked.
   - LIFF/WebView login event visible through admin query.
   - Unknown fallback visible instead of blank fields.

Use `docs/evidence/spec-022/README.md` and the two recorder scripts to create
those JSON files:

```bash
node tools/scripts/record-spec-022-liff-evidence.mjs .tmp/spec-022-liff.log
node tools/scripts/record-spec-022-manual-qa-evidence.mjs .tmp/spec-022-manual-qa.log
node tools/scripts/preflight-spec-022-evidence.mjs
node tools/scripts/finalize-spec-022-complete.mjs
node tools/scripts/check-spec-022-complete.mjs
```

## Latest Validation Run

- `USER_SERVICE_TEST_DATABASE_URL=postgres://proph:dev@localhost:5432/prophetica cargo test -p user-service --locked`: 48 passed, 1 ignored.
- `cargo clippy -p user-service --all-targets --locked -- -D warnings`: no issues.
- `cargo test -p user-service --test auth spec_022_login_event_latency_p95_local_budget --locked -- --ignored`: 1 passed.
- `pnpm --filter @prophetica/liff lint`: passed.
- `cargo check -p core-api --locked`: passed.
- `node --test tools/scripts/*.test.mjs`: 17 passed.
- `node tools/scripts/preflight-spec-022-evidence.mjs`: correctly reports missing env/log/evidence prerequisites in the local environment; tests cover evidence files whose required fields are not true.
- `node tools/scripts/finalize-spec-022-complete.mjs`: correctly restores the blocked manifest and exits 1 while the two external evidence files are missing.
- `node tools/scripts/check-spec-022-complete.mjs`: correctly fails closed while the two external evidence blockers remain.
