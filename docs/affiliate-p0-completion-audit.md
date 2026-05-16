# Affiliate / Referral P0 Completion Audit

Last updated: 2026-05-16

Objective: continue closing LG-9 affiliate / referral production readiness until
all P0 items can be formally declared complete.

This audit is intentionally strict: an item is only complete when there is a
concrete implementation artifact plus validation or operational evidence. Proxy
signals such as "tests passed" are only accepted when the tested surface covers
the stated requirement.

Machine-readable evidence summary: `docs/affiliate-p0-evidence-manifest.json`.
Formal completion gate: `tools/scripts/check-affiliate-p0-complete.mjs`.

## Checklist

| P0 requirement | Implementation evidence | Validation / evidence | Status |
| --- | --- | --- | --- |
| Affiliate mock data fails closed in production | `apps/admin/lib/affiliates.ts` production mock fallback guard | `apps/admin/tests/affiliate-credit.test.ts` targeted coverage from earlier LG-9 pass | Complete |
| Affiliate-capable auth rejects weak/plaintext credentials | `services/user-service/src/auth.rs` Argon2id PHC verification and strict credential policy | `rtk cargo test -p user-service --test auth --locked`: 13 passed | Complete |
| User-service issues Ed25519 sessions and publishes JWKS | `services/user-service/src/auth.rs` `/v1/auth/login`, `/v1/auth/me`, `/v1/auth/logout`, JWKS routes | `rtk cargo test -p user-service --test auth --locked`: 13 passed | Complete |
| Admin verifies user-service Ed25519 session JWTs | `apps/admin/lib/session-crypto.ts`, `apps/admin/lib/session.ts` | targeted admin session test: 13 passed in session / step-up suite | Complete |
| Admin login can submit MFA evidence to user-service | `apps/admin/components/login-form.tsx`, `apps/admin/app/api/auth/login/route.ts`, `apps/admin/tests/login-redirect.test.ts` | targeted admin login redirect/source contract test: 5 passed | Complete for TOTP / recovery-code UI contract |
| Hardware / step-up MFA claims exist and admin sensitive actions enforce 30-minute recency | `services/user-service/src/auth.rs`, `apps/admin/lib/step-up-mfa-policy.ts`, `apps/admin/middleware.ts` | `rtk cargo test -p user-service --test auth --locked`: 13 passed; targeted admin session / step-up suite: 13 passed | Complete for configured local-account evidence and admin enforcement |
| TOTP step-up and recovery fallback exist for local auth | `services/user-service/src/auth.rs` TOTP verification, one-time recovery-code fallback, optional persisted consumed-code store | `rtk cargo test -p user-service --test auth --locked`: 13 passed | Complete for local provider |
| Web auth client and sign-in UI can submit MFA evidence to user-service | `apps/web/lib/auth.ts`, `apps/web/app/auth/sign-in/page.tsx`, `apps/web/tests/auth.test.ts`, `apps/web/tests/page-aria.test.ts` | targeted apps/web auth test: 8 passed; targeted apps/web page-aria test: 16 passed | Complete for TOTP / recovery-code UI contract |
| Durable WebAuthn credential and challenge-state store exists | `services/user-service/migrations/202605160003_create_webauthn_tables.sql` | migration structural check passed for credential table, challenge table, `credential_id`, `state_json`, `expires_at`, `consumed_at`, and `revoked_at` | Complete for schema |
| WebAuthn store contract exists | `services/user-service/src/webauthn_store.rs`, `services/user-service/tests/webauthn_store.rs` | `rtk cargo test -p user-service --test webauthn_store --locked`: 2 passed | Complete for internal store contract |
| Real WebAuthn / passkey registration and assertion lifecycle exists | Not implemented; required contract is documented in `docs/webauthn-passkey-p0-contract.md` | No registration / assertion endpoints, assertion verification, browser ceremony, or user-facing enrollment surface | Blocked / incomplete |
| Cloudflare Access allows `/login*` and `/me/affiliate*`, while internal admin surfaces require SSO + hardware MFA | `apps/admin/lib/cloudflare-access-policy.ts`, `apps/admin/middleware.ts`, `infra/cloudflare/zero-trust/admin-access.tf`, `tools/scripts/validate-cloudflare-admin-access.mjs`, `tools/scripts/record-cloudflare-admin-access-evidence.mjs` | app-side policy tests passed earlier; `rtk node tools/scripts/validate-cloudflare-admin-access.mjs` passed 9 structural checks; evidence recorder syntax check passed; Terraform not applied because local `terraform` is unavailable and no target account evidence exists | Blocked / incomplete |
| First-touch referral capture exists in referral-service and api-gateway | `services/referral-service/src/lib.rs`, `services/api-gateway/src/lib.rs` | referral-service `admin_affiliates` test passed earlier; api-gateway capture proxy test passed earlier | Complete |
| Admin / affiliate member creation captures first-touch referral and stores registration audit evidence | `apps/admin/lib/member-management-bff.ts`, `services/user-service/src/members.rs` | targeted admin member-management BFF test passed earlier; `rtk cargo test -p user-service --test members --locked` passed earlier | Complete |
| Public web preserves referral code before signup | `apps/web/middleware.ts`, `apps/web/lib/referral-attribution.ts` | referral attribution targeted test: 3 passed | Complete for first-touch preservation |
| Public signup submits preserved referral code to registration and capture flow | No public signup form exists; current `/auth/sign-in?mode=signup` is a closed-beta gate | `apps/web/tests/page-aria.test.ts` documents signup mode as beta gate | Blocked / incomplete |
| 30-day qualification evaluator exists with KYC, deposit, trade-count, caps, ledger draft/finalize, and worker orchestration | `services/referral-service/src/lib.rs`, `services/_binaries/referral-worker/src/main.rs`, `infra/cron/referral-workers.cron` | `rtk cargo test -p referral-service --test admin_affiliates --locked` passed earlier; `rtk cargo check -p referral-worker-binary --locked` passed earlier | Complete |
| Production qualification input path includes KYC / wallet-ledger / AML anti-abuse signals | `services/_binaries/referral-worker/src/main.rs` | `rtk cargo check -p referral-worker-binary --locked` passed earlier | Complete |
| Trade commission accrual and payout pipeline exists | `services/referral-service/src/lib.rs`, `services/_binaries/referral-worker/src/main.rs`, `infra/cron/referral-workers.cron` | referral-service `admin_affiliates` test passed earlier; referral-worker targeted check passed earlier | Complete |

## Formal declaration blockers

- Implement and validate real WebAuthn / passkey enrollment and assertion:
  challenge generation, challenge persistence, credential storage, assertion
  verification, recovery / revocation, and user-facing enrollment UX.
- Apply the Cloudflare Zero Trust policy in the target account and archive plan
  / apply evidence. Local validation could not run because `terraform` is not
  installed in this environment.
- Replace the closed-beta signup gate with a real public registration flow, then
  submit `proph_referral_code` with registration and prove it reaches the
  existing referral capture API.

## Latest validation run in this thread

- `rtk cargo test -p user-service --test auth --locked`: 13 passed.
- `apps/web` referral attribution targeted test: 3 passed.
- Earlier targeted admin session / step-up suite: 13 passed.
- `rtk node tools/scripts/validate-cloudflare-admin-access.mjs`: 9 checks
  passed.
- `apps/web` auth targeted test: 8 passed.
- `apps/web` page-aria targeted test: 16 passed.
- `rtk node tools/scripts/check-affiliate-p0-complete.mjs`: correctly failed
  closed while 3 blocking items remain.
- `apps/admin` login-redirect targeted test: 5 passed.
- `rtk node --check tools/scripts/record-cloudflare-admin-access-evidence.mjs`:
  passed.
- WebAuthn migration structural check: passed.
- `rtk cargo test -p user-service --test webauthn_store --locked`: 2 passed.
