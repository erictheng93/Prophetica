# TODO

Last updated: 2026-05-18

## MVP Phase 2: Local Infra And Observability Stack

Phase 1 MVP development intentionally starts with the player-facing web app and
admin app only. Full local infrastructure is deferred to Phase 2 so the initial
user/admin workflow can move without requiring Kafka, Grafana, Prometheus, or
the full Docker dependency stack.

- Promote `make dev-infra` into the Phase 2 local integration entrypoint and
  keep `make dev` focused on web + admin.
- Bring up and smoke-test PostgreSQL, Redis, Kafka, Vault, Mailhog,
  Prometheus, Grafana, Tempo, Loki, and the OTel Collector through
  `infra/docker-compose.local.yaml`.
- Wire matching-engine projection events to Kafka in the production event path,
  with `acks=all` and idempotent producer settings.
- Replace static readiness with dependency-aware checks for services that need
  PostgreSQL, Redis, Kafka, or downstream service calls.
- Add a lightweight Phase 2 smoke command that verifies compose health, key
  service `/healthz/ready` responses, Prometheus scrape reachability, and a
  Grafana dashboard import.
- Document resource expectations for macOS/OrbStack and keep full-stack checks
  out of the default MVP loop.

## SPEC-008 Deferred Production Topology

- Resolve the Railway + Neon MVP deployment tradeoff for
  `matching-engine-hot`: Railway shared/general vCPU deployments may run the
  current-thread `LocalSet` binary correctly, but `core_affinity` is only a
  best-effort latency hint without dedicated-node / static CPU-manager
  guarantees. Before claiming production hot-market P99 latency, move the hot
  matching topology to dedicated Kubernetes nodes with static
  `cpuManagerPolicy` or equivalent `taskset`-style pinning, then capture
  production-grade latency evidence.

## SPEC-022 Completion Blockers

- Capture live LIFF identity-bridge evidence before declaring SPEC-022 complete.
  The code path now exchanges a LINE ID token for a `user-service` session and
  records `login_surface = liff`, but formal completion still needs a LINE
  channel configured with `USER_SERVICE_LINE_LOGIN_CHANNEL_ID`, a member linked
  through `user_members.line_id`, and an iOS / Android LIFF login run showing
  the resulting `login_success` event. Track this in
  `docs/spec-022-evidence-manifest.json` and keep
  `tools/scripts/check-spec-022-complete.mjs` failing until evidence is
  recorded.
- Run and record manual QA for desktop web, mobile web, and LIFF/WebView login
  surfaces after the LIFF session path exists, confirming admin and user UIs
  show approximate location language and `未知` fallback instead of blank
  fields. Use `tools/scripts/record-spec-022-liff-evidence.mjs` and
  `tools/scripts/record-spec-022-manual-qa-evidence.mjs` to produce the two
  required `docs/evidence/spec-022-*/latest.json` files; follow
  `docs/evidence/spec-022/README.md` for the exact markers and completion
  process.

## Public Beta Launch Gate Blockers

These items are not complete enough to count the Public Beta gate as passed.
They are tracked here so the current completed work can be committed without
losing the remaining launch work.

### LG-3: KYC / AML Provider Readiness

- Run `PostgresKycStore` against a real migrated PostgreSQL database in CI or
  staging, not only unit-contract coverage.
- Add transaction-boundary tests for SumSub webhook record updates plus
  screening-event inserts.
- Refresh SQLx offline metadata if the KYC service adopts compile-time checked
  SQL queries.
- Finish SumSub sandbox credentials and Tier 2 iframe E2E coverage.
- Capture webhook evidence for HMAC validation, GREEN / RED / PENDING replay,
  RED-to-Frozen handling, and PEP / sanctions rescan behavior.
- Complete iProov real adapter readiness, or publish a fallback ADR before
  reducing the gate level.

### LG-4: PoC Evidence

- Complete PoC-A live EdDSA / LIFF verification on iOS and Android devices.
- Retry PoC-B on Linux or CI with reachable `ws-gateway`, SumSub sandbox, and
  `next-on-pages` build path.

### LG-2: Payment Provider Readiness

- Obtain provider LOI or terms that explicitly allow the intended business
  model, MCC, chargeback policy, and required payout jurisdiction.
- Complete sandbox webhook verification for authorization, capture, refund,
  dispute, and chargeback events.
- Run a refund and chargeback operational drill.
- Confirm PCI SAQ A scope and evidence requirements.

### LG-5: Release / Operations Gate

- Run the release workflow against a staging or release-candidate tag and keep
  the resulting artifact evidence.
- Add or verify production-targeting gates for secret scanning, SAST, SCA,
  image scanning, IaC validation, migration dry-run, and E2E smoke checks.
- Complete rollback drill evidence for the first public-beta deployment path.
- Capture the SPEC-010 MS-12 approval snapshot required by the launch gate.

### LG-6: Support / Incident Readiness

- Stand up the support queue, escalation owner mapping, and public-beta SLA.
- Prepare a sample customer-support evidence pack.
- Run and document an S0 / S1 incident drill.
- Link final runbooks from the launch-gate evidence package.

### LG-1: Legal / Jurisdiction Readiness

- Complete legal memo coverage for the initial market list.
- Confirm jurisdiction allow / deny rules for the beta launch scope.
- Publish any required market-specific restrictions before launch.

### LG-7: admin RBAC Phase 3 — production auth

Tracked here because Phase 2 (2026-05-10) ships only a mock auth surface
inside `apps/admin/`. None of the items below can ride to production as-is.

- Stand up real auth endpoints in `services/user-service` (`/v1/auth/login`,
  `/v1/auth/logout`, `/v1/auth/me`) is implemented for the local provider;
  remaining work is deprecating `apps/admin/app/api/auth/*` mock and
  `apps/admin/lib/mock-users.ts` once admin sessions move to the real provider.
- Sign sessions with Ed25519 (private key in Vault per SPEC-010 §3.3); admin
  middleware verifies via JWKS endpoint instead of HMAC. Update
  `apps/admin/lib/session.ts` accordingly.
- Migrate affiliate passwords to Argon2id PHC hashes. Runtime verification with
  minimum `m >= 19456, t >= 2, p >= 1` is implemented in `user-service`;
  remaining work is credential migration / secret management.
- Add hardware MFA across all roles per SPEC-010 §9.3.3 (TOTP + WebAuthn).
  `user-service` local auth can now fail closed for admin / manager /
  affiliate accounts unless `USER_SERVICE_REQUIRE_HARDWARE_MFA=true` accounts
  include hardware MFA evidence (`webauthn` / passkey / FIDO2 security-key
  method plus `mfa_at_epoch_seconds`), and Ed25519 session JWTs carry `mfa_at`
  plus `amr=["pwd","hwk"]`. Admin middleware now enforces 30-minute hardware
  MFA step-up for mutating high-sensitivity paths using those claims.
  `user-service` local auth now also supports TOTP step-up for accounts with
  `totp_secret_base32`: login fails with `ERROR_CODE_MFA_REQUIRED` until a
  valid TOTP code or configured recovery code is supplied, then signs a fresh
  `mfa_at` claim with `amr=["pwd","otp"]` or recovery evidence. Recovery codes
  are consumed once per `user-service` process so repeated use is rejected, and
  can be persisted across process restarts through
  `USER_SERVICE_RECOVERY_CODE_CONSUMED_PATH`.
  Durable WebAuthn credential and challenge-state tables are defined in
  `services/user-service/migrations/202605160003_create_webauthn_tables.sql`.
  `services/user-service/src/webauthn_store.rs` defines the store trait and
  in-memory lifecycle implementation for one-time challenge consumption and
  credential revocation. `services/user-service/src/webauthn_runtime.rs`
  integrates the official `webauthn-rs` verifier crate and can generate
  passkey registration options plus serializable registration state from
  relying-party configuration, then persist that state into `WebauthnStore`
  with a short challenge TTL. `POST /v1/auth/webauthn/register/start` is
  implemented for authenticated sessions and returns a challenge id plus browser
  `publicKey` creation options. Runtime support for
  `finish_passkey_registration` is wired through `webauthn-rs` payload/state
  types. `POST /v1/auth/webauthn/register/finish` now consumes the stored
  registration challenge, verifies submitted browser credential payloads through
  `webauthn-rs`, and stores the resulting credential record; invalid browser
  payloads fail closed through the verifier path.
  `POST /v1/auth/webauthn/assert/start` and
  `POST /v1/auth/webauthn/assert/finish` now persist assertion challenge state,
  verify browser assertion payloads through `webauthn-rs`, update credential
  metadata, and issue a fresh hardware-MFA session on successful assertion;
  invalid browser payloads fail closed through the verifier path.
  `AuthConfig::from_env()` now enables WebAuthn when
  `USER_SERVICE_WEBAUTHN_RP_ID` and `USER_SERVICE_WEBAUTHN_RP_ORIGIN` are
  configured, and core-api Postgres mode wires `PostgresWebauthnStore` into the
  auth router so challenge / credential state is durable in production mode.
  Admin browser enrollment / assertion UX now exists in
  `apps/admin/components/passkey-security-panel.tsx`, with WebAuthn option /
  credential conversion in `apps/admin/lib/passkey-client.ts` and a BFF proxy in
  `apps/admin/app/api/auth/passkeys/[action]/route.ts` so the user-service
  session token stays in httpOnly cookies. `user-service` also exposes
  authenticated credential revocation at
  `POST /v1/auth/webauthn/credentials/revoke`, and
  `apps/admin/tests/e2e/passkey-webauthn.spec.ts` defines the Chromium virtual
  authenticator evidence flow. Remaining work is running that browser evidence
  flow, recording `docs/evidence/webauthn-passkey/latest.json`, plus
  recovery-code rotation.
  Browser ceremony evidence should be recorded with
  `tools/scripts/record-webauthn-passkey-evidence.mjs`.
- Configure Cloudflare Access with bypass policies for `/login` and
  `/me/affiliate/*` so external affiliates can reach the app surface, while
  `/`, `/dashboard`, and the six internal domain routes still require SSO.
  Application-side path policy is implemented in admin middleware. Cloudflare
  Zero Trust Terraform for the account-side policy now exists under
  `infra/cloudflare/zero-trust`; remaining work is applying it in the target
  account and capturing plan / apply evidence.
- Replace `ADMIN_SESSION_SECRET` dev fallback in `lib/session.ts` with
  fail-closed behavior (no default secret in production) is implemented; local
  development still gets an explicit dev-only fallback.
- Remove the dev-credential hint block from `app/login/page.tsx` once real
  user-service is in place.

### LG-8: PWA / RWD followups

Phase A-C of the responsive + PWA pass landed 2026-05-10; the remaining
items below are deliberately deferred because they require either a
production deploy or backend coordination.

- Generalized background sync allowlist framework. Phase 1 now limits offline
  queuing to trading order placement / cancellation with api-gateway
  `Idempotency-Key` replay protection. A later phase should promote this into
  a generic api-gateway mutation allowlist with per-route reconciliation
  policies before any ledger, payment, admin, KYC, or support mutation is
  allowed into silent background replay.
- Pending mutation UI for trading. The Phase 1 queue returns a `202 queued`
  response and drains through Background Sync, but the trading surface still
  needs explicit pending / replayed / failed order state once the order form is
  wired to the real api-gateway mutation client.
- Lighthouse PWA score validation. Local `next dev` cannot reach 100/100
  because Lighthouse insists on HTTPS for installability. Validate after
  the staging deploy and capture the run as evidence under SPEC-007.
- Real maskable icon artwork. `app/icon{0,1}.tsx` currently render a
  placeholder "P" glyph against Action Blue. Brand will supply a final
  mark; swap into the same ImageResponse generators (or precomputed
  PNGs) without changing the manifest URLs.

### LG-9: Affiliate / referral production readiness

Current state as of 2026-05-17: SPEC-011 data models, referral-service
migrations, admin affiliate CRUD, affiliate portal pages, RBAC, and frontend
API clients exist. The remaining work below is ordered by production risk and
dependency. Strict P0 completion evidence is tracked in
`docs/affiliate-p0-completion-audit.md` and
`docs/affiliate-p0-evidence-manifest.json`; the WebAuthn / passkey
contract is specified in `docs/webauthn-passkey-p0-contract.md`. Formal
completion must pass `tools/scripts/check-affiliate-p0-complete.mjs`, which
fails closed while blocking items remain.

- P0: Fail closed for affiliate mock data in production. `apps/admin` must not
  return static affiliate snapshots when `PROPHETICA_ENV=production`, even if
  `AFFILIATE_ADMIN_ALLOW_MOCK_FALLBACK=true` is misconfigured.
- P0: Complete production auth prerequisites that block external affiliates:
  Argon2id PHC hash verification with minimum parameters
  (`m >= 19456, t >= 2, p >= 1`) has landed in `user-service` local auth for
  affiliate-capable accounts, and `/v1/auth/login`, `/v1/auth/me`,
  `/v1/auth/logout` are available in `user-service`. App-side Ed25519 compact
  JWS verification with configured JWKS / public JWK support has landed in
  `apps/admin` while preserving local HMAC session compatibility. User-service
  Ed25519 compact JWS issuance and JWKS publishing have landed behind
  `USER_SERVICE_ED25519_SESSION_PRIVATE_JWK`, with local dev token fallback
  preserved when no signer is configured. Strict credential policy
  (`USER_SERVICE_REQUIRE_ARGON2ID_CREDENTIALS=true`) rejects plaintext
  admin/manager/affiliate local-login credentials so configured affiliate
  accounts must be Argon2id PHC hashes. Strict hardware MFA policy
  (`USER_SERVICE_REQUIRE_HARDWARE_MFA=true`) rejects admin / manager /
  affiliate local-login accounts without WebAuthn / passkey / FIDO2 security-key
  evidence, and user-service session JWTs now emit `mfa_at` / `amr` claims.
  Admin middleware enforces 30-minute hardware MFA step-up on mutating
  high-sensitivity paths. TOTP step-up for configured local accounts is
  implemented in `user-service`, including `ERROR_CODE_MFA_REQUIRED`, valid
  TOTP verification, one-time recovery-code fallback, and fresh `mfa_at`
  issuance. Consumed recovery codes can be persisted with
  `USER_SERVICE_RECOVERY_CODE_CONSUMED_PATH` so reuse is rejected after service
  restart. `apps/web` auth client forwards optional `mfa_code` /
  `mfa_recovery_code` evidence to `user-service` so the frontend contract can
  complete TOTP step-up; the sign-in page now exposes optional verification-code
  and recovery-code fields with localized copy. `apps/admin` login form and
  login API route also collect and forward optional MFA evidence to
  `user-service` for admin / manager / affiliate sessions.
  Cloudflare Zero Trust Terraform for the required `/login` and
  `/me/affiliate*` bypass policies plus internal SSO / hardware-MFA policy is
  available under `infra/cloudflare/zero-trust`, with a local structural
  verifier in `tools/scripts/validate-cloudflare-admin-access.mjs` and an
  evidence recorder in
  `tools/scripts/record-cloudflare-admin-access-evidence.mjs`. WebAuthn /
  passkey browser evidence is recorded at
  `docs/evidence/webauthn-passkey/latest.json`. Remaining work: rotate recovery
  codes operationally, run
  `rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs --before-apply`
  in the target environment, install/provide Terraform or OpenTofu plus Cloudflare
  target-account variables, then run
  `rtk node tools/scripts/apply-cloudflare-admin-access-evidence.mjs --apply`
  to capture plan/apply logs and record the evidence. The operator procedure is
  documented in `docs/evidence/cloudflare-admin-access/README.md`.
  target-account variables, apply Cloudflare policy in the target account, and
  capture real plan / apply evidence.
- P0: Wire first-touch referral capture into all registration surfaces. Phase A
  landed the referral-service capture API (`POST /internal/referrals/capture`)
  with first-touch idempotency, self-referral rejection, and Postgres
  `referred_id` conflict handling. Phase B landed api-gateway internal proxy
  (`POST /api/internal/referrals/capture`) and admin/affiliate member creation
  wiring so attributed member registration captures the first-touch referral.
  Phase C landed persisted registration audit evidence for attributed
  admin/affiliate member creation: `apps/admin` forwards
  `registration_audit` metadata and `user-service` stores it in
  `user_members.registration_audit`. Phase D landed public web first-touch
  referral attribution capture: `apps/web` stores valid `referral_code`, `ref`,
  or `affiliate_code` query values in a 30-day `proph_referral_code` cookie
  without overwriting an existing cookie. Remaining work: when the public signup
  form replaces the current closed-beta gate, submit the cookie value with the
  registration request and call the existing referral capture path. `apps/web`
  now replaces the closed-beta signup gate with an enabled registration form,
  reads `proph_referral_code` server-side, and sends it as `referral_code`
  through the public registration client contract. On successful registration,
  the web client calls the existing referral capture path and fail-closes when
  capture is unavailable so attribution is not silently lost. `user-service`
  now exposes `POST /v1/public/registrations` to create an inactive Tier 0
  member, persist referral registration audit metadata, hash the submitted
  password with Argon2id, and provision a durable Postgres login credential.
  `user-service` auth can authenticate those stored credentials and resolve
  `/v1/auth/me` from the credential store when running with Postgres-backed
  core-api wiring. Browser submit-to-referral-capture evidence is recorded at
  `docs/evidence/public-signup-referral/latest.json` through the fail-closed
  recorder `tools/scripts/record-public-signup-referral-evidence.mjs`.
- P0: Complete the 30-day qualification evaluator. Phase A landed the
  referral-service evaluator API
  (`POST /internal/referrals/qualification-evaluate`) for single-referral
  matured evaluation, KYC / deposit / trade-count / anti-abuse enforcement,
  Postgres status persistence, bonus amount calculation, and monthly/lifetime
  signup cap enforcement that sends over-cap referrals to `pending_eddreview`.
  Phase B landed batch selection for matured `pending` rows
  (`GET /internal/referrals/qualification-due`). Phase C landed signup bonus
  ledger finalization
  (`POST /internal/referrals/signup-bonus-finalize`) so the worker can
  idempotently record the successful ledger transaction id after posting. Phase
  D landed a wallet-ledger compatible signup bonus draft endpoint
  (`GET /internal/referrals/signup-bonus-ledger-draft/:referred_id`) with a
  stable idempotency key and balanced referrer / referred bonus entries. Phase
  E landed a trait-based daily worker orchestration path that selects due
  referrals, asks an injected qualification input provider for fresh KYC /
  deposit / trade inputs, evaluates the referral, posts the signup bonus via an
  injected ledger poster, and finalizes the returned ledger transaction id.
  Phase F landed the production wallet-ledger-service HTTP poster for
  `/internal/ledger/transactions`, including `idempotency-key` forwarding and
  returned transaction id parsing.
  Phase G landed `referral-worker` daily-qualification mode plus cron scheduling
  support; it pulls per-referral `QualificationEvent` data from
  `REFERRAL_QUALIFICATION_INPUT_BASE_URL` or directly composes inputs from
  `KYC_SERVICE_DATABASE_URL` and `WALLET_LEDGER_DATABASE_URL`, evaluates due
  referrals, posts signup bonus ledger transactions, and finalizes returned
  transaction ids. Phase H added Postgres-backed anti-abuse signals from KYC /
  AML data for same KYC identity, chargeback / void flags, abnormal referrer
  AML activity, same-IP flags, and same-device flags. The production KYC /
  deposit / trade-count and AML input paths are implemented.
- P0: Complete the trade commission accrual and payout pipeline. Phase A landed
  referral-service single-trade accrual API
  (`POST /internal/referrals/commission-accruals`) with qualified-referral
  enforcement, anti-abuse checks, 12-month commission window, 10% fee
  calculation, and `trade_id` idempotency / Postgres persistence. Phase I
  landed `referral-worker` trade-accrual mode plus cron scheduling; it consumes
  wallet-ledger `trade` / `settle` transactions for qualified referrals and
  writes idempotent commission accruals with `trade_id=ledger:{txn_id}`.
  Phase H landed the `referral-worker` binary plus
  `infra/cron/referral-workers.cron` for scheduled daily commission payout
  execution against Postgres referral data and wallet-ledger-service. Phase G landed the due payout selector
  (`GET /internal/referrals/commission-accruals-payout-due`) so the daily
  worker can enumerate referrers with unpaid accruals before posting payouts.
  Phase F landed trait-based daily payout worker
  orchestration that builds the payout draft, posts it through the same
  wallet-ledger-service HTTP poster, and marks accruals paid with the returned
  transaction id. Phase E landed a wallet-ledger compatible
  commission payout draft endpoint
  (`GET /internal/referrals/commission-accruals-payout-ledger-draft/:referrer_id`)
  with a stable daily idempotency key, unpaid accrual aggregation, and balanced
  payout entries. Phase D landed persisted `referrer_group_id` storage plus group monthly trade
  commission cap enforcement in the accrual path. Phase C landed referrer
  monthly trade commission cap enforcement before persistence. Phase B landed
  mark-paid payout finalization
  (`POST /internal/referrals/commission-accruals-payout`) that aggregates unpaid
  accruals by referrer, requires a successful ledger transaction id from the
  caller, marks rows paid, and is idempotent by `paid_out_txn_id`.
- P1: Add admin approve / reject actions for affiliate commission adjustment
  reviews and payout reviews. Current pages list queues but do not finalize
  review decisions.
- P1: Replace or strictly gate remaining affiliate demo/fallback surfaces.
  Staging and production should make upstream unavailability explicit instead
  of silently presenting static affiliate data.
- P1: Align affiliate portal language with the single-level referral model.
  Avoid product-facing "sub-tree" wording unless the page is explicitly an
  internal B2B operational view and cannot imply MLM-style depth.
- P1: Add end-to-end affiliate flow coverage: affiliate login, referred member
  creation, qualification, commission accrual, payout review, and dashboard
  display.
- P2: Add operational evidence capture for affiliate audit logs, rate changes,
  credit rating changes, credit account limits, and payout decisions.
