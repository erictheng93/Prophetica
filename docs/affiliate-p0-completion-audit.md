# Affiliate / Referral P0 Completion Audit

Last updated: 2026-06-30

Objective: continue closing LG-9 affiliate / referral production readiness until
all P0 items can be formally declared complete.

This audit is intentionally strict: an item is only complete when there is a
concrete implementation artifact plus validation or operational evidence. Proxy
signals such as "tests passed" are only accepted when the tested surface covers
the stated requirement.

Machine-readable evidence summary: `docs/affiliate-p0-evidence-manifest.json`.
Formal completion gate: `tools/scripts/check-affiliate-p0-complete.mjs`.
Prompt-to-artifact completion checklist:
`docs/affiliate-p0-completion-checklist.md`.
The completion gate also requires
`docs/evidence/cloudflare-admin-access/latest.json` with target-account metadata,
plan/apply SHA-256 hashes, byte counts, and success summaries before it can pass
after blockers are cleared.
It also requires `docs/evidence/webauthn-passkey/latest.json` proving browser
registration/assertion/replay rejection/fresh hardware-MFA session, and
`docs/evidence/public-signup-referral/latest.json` proving signup submit,
referral-cookie submission, and referral capture.

## Checklist

| P0 requirement | Implementation evidence | Validation / evidence | Status |
| --- | --- | --- | --- |
| Affiliate mock data fails closed in production | `apps/admin/lib/affiliates.ts` production mock fallback guard | `apps/admin/tests/affiliate-credit.test.ts` targeted coverage from earlier LG-9 pass | Complete |
| Affiliate-capable auth rejects weak/plaintext credentials | `services/user-service/src/auth.rs` Argon2id PHC verification and strict credential policy | `cargo test -p user-service --test auth --locked`: 13 passed | Complete |
| User-service issues Ed25519 sessions and publishes JWKS | `services/user-service/src/auth.rs` `/v1/auth/login`, `/v1/auth/me`, `/v1/auth/logout`, JWKS routes | `cargo test -p user-service --test auth --locked`: 13 passed | Complete |
| Admin verifies user-service Ed25519 session JWTs | `apps/admin/lib/session-crypto.ts`, `apps/admin/lib/session.ts`, `apps/admin/tests/session.test.ts` | targeted admin session tests cover Ed25519/JWKS production verification and legacy HMAC rejection | Complete |
| Admin login can submit MFA evidence to user-service | `apps/admin/components/login-form.tsx`, `apps/admin/app/api/auth/login/route.ts`, `apps/admin/tests/login-redirect.test.ts` | targeted admin login redirect/source contract test: 5 passed | Complete for TOTP / recovery-code UI contract |
| Hardware / step-up MFA claims exist and admin sensitive actions enforce recency | `services/user-service/src/auth.rs`, `docs/evidence/webauthn-passkey/latest.json`, `apps/admin/proxy.ts`, `apps/admin/lib/session-crypto.ts`, `apps/admin/tests/session.test.ts` | user-service targeted auth tests passed earlier; recorded WebAuthn evidence has fresh hardware-MFA session and fresh/stale admin step-up markers true; admin session tests cover production Ed25519 session verification | Complete for configured local-account evidence and admin enforcement |
| TOTP step-up and recovery fallback exist for local auth | `services/user-service/src/auth.rs` TOTP verification, one-time recovery-code fallback, optional persisted consumed-code store | `cargo test -p user-service --test auth --locked`: 13 passed | Complete for local provider |
| Web auth client and sign-in UI can submit MFA evidence to user-service | `apps/web/lib/auth.ts`, `apps/web/app/auth/sign-in/page.tsx`, `apps/web/tests/auth.test.ts`, `apps/web/tests/page-aria.test.ts` | targeted apps/web auth test: 8 passed; targeted apps/web page-aria test: 16 passed | Complete for TOTP / recovery-code UI contract |
| Durable WebAuthn credential and challenge-state store exists | `services/user-service/migrations/202605160003_create_webauthn_tables.sql` | migration structural check passed for credential table, challenge table, `credential_id`, `state_json`, `expires_at`, `consumed_at`, and `revoked_at` | Complete for schema |
| WebAuthn store contract exists | `services/user-service/src/webauthn_store.rs`, `services/user-service/tests/webauthn_store.rs` | `cargo test -p user-service --test webauthn_store --locked`: 2 passed | Complete for internal store contract |
| WebAuthn verifier runtime is integrated | `services/user-service/src/webauthn_runtime.rs`, `Cargo.toml`, `services/user-service/Cargo.toml` | `cargo test -p user-service webauthn_runtime --locked`: 4 passed, 22 filtered out | Complete for registration-options generation, challenge-state persistence, and registration-finish verifier support |
| WebAuthn registration start endpoint exists | `services/user-service/src/auth.rs`, `services/user-service/tests/auth.rs` | `cargo test -p user-service --test auth --locked`: 14 passed | Complete for authenticated registration-start API |
| WebAuthn registration finish endpoint exists | `services/user-service/src/auth.rs`, `services/user-service/tests/auth.rs` | `cargo fmt -p user-service`; `cargo test -p user-service --test auth --locked`: 14 passed | Complete for server-side finish API wiring and fail-closed invalid browser payload path |
| WebAuthn assertion start / finish endpoints exist | `services/user-service/src/auth.rs`, `services/user-service/src/webauthn_runtime.rs`, `services/user-service/tests/auth.rs` | `cargo fmt -p user-service`; `cargo test -p user-service webauthn_runtime --locked`: 6 passed, 26 filtered out; `cargo test -p user-service --test auth --locked`: 16 passed | Complete for server-side assertion API wiring and fail-closed invalid browser payload path |
| WebAuthn durable production wiring exists | `services/user-service/src/auth.rs`, `services/user-service/src/webauthn_runtime.rs`, `services/_binaries/core-api/src/main.rs` | `cargo test -p user-service webauthn_runtime --locked`: 6 passed, 26 filtered out; `cargo test -p user-service --test auth --locked`: 16 passed; `cargo check -p core-api --locked`: passed | Complete for env-driven RP config and Postgres challenge / credential store wiring |
| Real WebAuthn / passkey registration and assertion lifecycle exists | Required contract is documented in `docs/webauthn-passkey-p0-contract.md`; fail-closed evidence recorder exists at `tools/scripts/record-webauthn-passkey-evidence.mjs`; durable service-side sources are in `services/user-service/src/auth.rs`, `services/user-service/src/webauthn_runtime.rs`, and `services/user-service/src/webauthn_store.rs`; browser evidence is recorded at `docs/evidence/webauthn-passkey/latest.json` | Recorded evidence contains registration/assertion round trip, replay rejection, unknown-credential rejection, wrong-origin rejection, revoked-credential rejection, fresh hardware-MFA session, and fresh/stale admin step-up markers true. | Complete |
| Cloudflare Access allows `/login*` and `/me/affiliate*`, while internal admin surfaces require SSO + hardware MFA | `apps/admin/lib/cloudflare-access-policy.ts`, `apps/admin/proxy.ts`, `infra/cloudflare/zero-trust/admin-access.tf`, `tools/scripts/validate-cloudflare-admin-access.mjs`, `tools/scripts/preflight-cloudflare-admin-access-evidence.mjs`, `tools/scripts/apply-cloudflare-admin-access-evidence.mjs`, `tools/scripts/record-cloudflare-admin-access-evidence.mjs` | app-side policy tests passed earlier; `node tools/scripts/validate-cloudflare-admin-access.mjs` passed 9 structural checks; evidence recorder now fails closed unless plan/apply logs include recognized success summaries, target-account metadata, and the required Cloudflare API token is present; recorder stores account/email-domain/breakglass hashes plus admin domain/service-token name without the API token or breakglass email plaintext; preflight supports `--before-apply` for target-environment readiness and full-gate mode for recorded evidence, including non-hex SHA-256 rejection; apply wrapper captures target-account plan/apply logs and hands them to the recorder when credentials are available; current environment has no `terraform`/`tofu` binary, no Cloudflare target-account env vars, and no `docs/evidence/cloudflare-admin-access/latest.json` plan/apply evidence | Blocked / incomplete |
| First-touch referral capture exists in referral-service and api-gateway | `services/referral-service/src/lib.rs`, `services/api-gateway/src/lib.rs` | referral-service `admin_affiliates` test passed earlier; api-gateway capture proxy test passed earlier | Complete |
| Admin / affiliate member creation captures first-touch referral and stores registration audit evidence | `apps/admin/lib/member-management-bff.ts`, `services/user-service/src/members.rs` | targeted admin member-management BFF test passed earlier; `cargo test -p user-service --test members --locked` passed earlier | Complete |
| Public web preserves referral code before signup | `apps/web/middleware.ts`, `apps/web/lib/referral-attribution.ts` | referral attribution targeted test: 3 passed | Complete for first-touch preservation |
| Public signup submits preserved referral code to registration and capture flow | `apps/web/app/auth/sign-in/page.tsx` enabled signup form; `apps/web/lib/signup.ts` public registration client sends `referral_code` from the first-touch cookie and fail-closes if referral capture is unavailable; `services/user-service/src/members.rs` exposes `POST /v1/public/registrations` to create an inactive Tier 0 member, persist referral registration audit metadata, hash the submitted password, and provision a durable Postgres login credential; `services/user-service/src/auth.rs` can authenticate stored credentials and resolve `/v1/auth/me`; `services/_binaries/core-api/src/main.rs` wires the Postgres credential store. Fail-closed browser evidence is recorded at `docs/evidence/public-signup-referral/latest.json`. | `pnpm --filter @prophetica/web test`: 177 passed; `cargo test -p user-service --test auth --test members --locked`: 22 passed; `cargo check -p core-api --locked`: passed; `pnpm --filter @prophetica/web test:e2e:signup-referral`: 1 passed; `node tools/scripts/record-public-signup-referral-evidence.mjs .tmp/public-signup-referral-signup.log .tmp/public-signup-referral-capture.log`: recorded evidence with all required markers true. | Complete |
| 30-day qualification evaluator exists with KYC, deposit, trade-count, caps, ledger draft/finalize, and worker orchestration | `services/referral-service/src/lib.rs`, `services/_binaries/referral-worker/src/main.rs`, `infra/cron/referral-workers.cron` | `cargo test -p referral-service --test admin_affiliates --locked` passed earlier; `cargo check -p referral-worker-binary --locked` passed earlier | Complete |
| Production qualification input path includes KYC / wallet-ledger / AML anti-abuse signals | `services/_binaries/referral-worker/src/main.rs` | `cargo check -p referral-worker-binary --locked` passed earlier | Complete |
| Trade commission accrual and payout pipeline exists | `services/referral-service/src/lib.rs`, `services/_binaries/referral-worker/src/main.rs`, `infra/cron/referral-workers.cron` | referral-service `admin_affiliates` test passed earlier; referral-worker targeted check passed earlier | Complete |
| Admin affiliate review decisions work end to end for commission adjustments and payout reviews | `apps/admin/app/affiliate/commissions/page.tsx`, `apps/admin/app/affiliate/payouts/page.tsx`, `apps/admin/app/affiliate/actions.ts`, `apps/admin/lib/affiliate-management-bff.ts`, `apps/admin/tests/e2e/affiliate-review.spec.ts`, `tools/scripts/check-affiliate-p0-complete.mjs`, `tools/scripts/check-affiliate-p0-complete.test.mjs` | `pnpm --filter @prophetica/admin test:e2e tests/e2e/affiliate-review.spec.ts`: 2 passed, covering admin approve of a commission adjustment and manager reject of a payout review through signed referral-service gateway calls; rows become read-only after approval/rejection. `node --test tools/scripts/check-affiliate-p0-complete.test.mjs`: 4 passed, proving the global P0 gate requires `admin-affiliate-review-e2e` and restores the blocked manifest when external evidence is missing. | Complete |

## Formal declaration blockers

- Apply the Cloudflare Zero Trust policy in the target account and archive plan
  / apply evidence. Current local inspection found no `terraform`/`tofu`
  binary, no Cloudflare target-account env vars, and no archived
  `docs/evidence/cloudflare-admin-access/latest.json`.

## Latest validation run in this thread

- `cargo fmt -p user-service`: passed.
- `cargo test -p user-service webauthn_runtime --locked`: 6 passed, 23
  filtered out.
- `cargo test -p user-service --test auth --locked`: 15 passed, including
  WebAuthn registration finish and assertion finish fail-closed coverage for
  invalid browser payloads.
- `pnpm --filter @prophetica/web test`: 177 passed, including public signup
  registration client referral payload, referral capture call, capture
  fail-closed coverage, and signup form source contract.
- `cargo test -p user-service --test members --locked`: 6 passed, including
  public registration member creation and referral audit persistence.
- `cargo test -p user-service --test auth --test members --locked`: 22
  passed, including stored credential login, `/v1/auth/me`, public
  registration member creation, and referral audit persistence.
- `cargo check -p core-api --locked`: passed.
- `node --check tools/scripts/record-webauthn-passkey-evidence.mjs &&
  node --check tools/scripts/record-public-signup-referral-evidence.mjs &&
  node --check tools/scripts/check-affiliate-p0-complete.mjs`: passed.
- `pnpm --filter @prophetica/web test:e2e:signup-referral`: 1 passed.
- `node tools/scripts/record-public-signup-referral-evidence.mjs
  .tmp/public-signup-referral-signup.log
  .tmp/public-signup-referral-capture.log`: recorded
  `docs/evidence/public-signup-referral/latest.json` with all required public
  signup markers true.
- `cargo test -p user-service webauthn_runtime --locked`: 6 passed, 26
  filtered out.
- `cargo test -p user-service --test auth --locked`: 16 passed.
- `cargo check -p core-api --locked`: passed with WebAuthn Postgres store
  wiring.
- `apps/web` referral attribution targeted test: 3 passed.
- Earlier targeted admin session / step-up suite: 13 passed.
- `node tools/scripts/validate-cloudflare-admin-access.mjs`: 9 checks
  passed.
- `apps/web` auth targeted test: 8 passed.
- `apps/web` page-aria targeted test: 16 passed.
- `pnpm --filter @prophetica/admin test:e2e:passkey`: 1 passed.
- `node tools/scripts/record-webauthn-passkey-evidence.mjs
  .tmp/webauthn-passkey.log`: recorded
  `docs/evidence/webauthn-passkey/latest.json` with all required passkey
  markers true.
- `node tools/scripts/check-affiliate-p0-complete.mjs`: correctly failed
  closed while the Cloudflare target-account apply evidence blocker remains.
- `apps/admin` login-redirect targeted test: 5 passed.
- `node --check tools/scripts/record-cloudflare-admin-access-evidence.mjs`:
  passed.
- WebAuthn migration structural check: passed.
- `cargo test -p user-service --test webauthn_store --locked`: 2 passed.
- `node --check tools/scripts/record-public-signup-referral-evidence.mjs`:
  passed.
- `node --check tools/scripts/record-webauthn-passkey-evidence.mjs`: passed.
- `cargo test -p user-service webauthn_runtime --locked`: 4 passed, 22
  filtered out.
- `pnpm --filter @prophetica/admin test:e2e
  tests/e2e/affiliate-review.spec.ts`: 2 passed, covering commission
  adjustment approval and payout rejection through signed admin gateway calls;
  approved / rejected rows render as read-only with no repeat decision buttons.
- `node --test tools/scripts/check-affiliate-p0-complete.test.mjs`: 2
  passed, including fail-closed coverage when `admin-affiliate-review-e2e` is
  absent from the completion manifest.
- `node --test tools/scripts/record-cloudflare-admin-access-evidence.test.mjs`:
  2 passed, proving Cloudflare evidence records target metadata without storing
  the API token or breakglass email plaintext and refuses to record when the
  API token is absent.
- `node --test tools/scripts/preflight-cloudflare-admin-access-evidence.test.mjs`:
  3 passed, proving full-gate preflight validates recorded target-account
  metadata and rejects non-hex SHA-256 hashes.
- `node --test tools/scripts/check-affiliate-p0-complete.test.mjs`: 4
  passed, including finalize coverage proving the affiliate manifest is restored
  when Cloudflare evidence is missing and only completed after evidence exists.
- `node --test tools/scripts/*.test.mjs`: 28 passed.
- Affiliate manifest artifact sweep: 0 missing completed-evidence artifacts.
- `node tools/scripts/finalize-affiliate-p0-complete.mjs`: correctly
  failed closed and restored the blocked manifest while
  `docs/evidence/cloudflare-admin-access/latest.json` is missing.
- `node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs
  --before-apply`: correctly reported `status: "blocked"` because this
  environment has no `terraform`/`tofu`, no target-account Cloudflare variables,
  and no recorded Cloudflare Access evidence JSON.
- `node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs`:
  correctly reported `status: "blocked"` because full-gate evidence is missing.
