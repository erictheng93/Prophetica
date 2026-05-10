# TODO

Last updated: 2026-05-09

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
  `/v1/auth/logout`, `/v1/auth/me`); deprecate `apps/admin/app/api/auth/*`
  mock and `apps/admin/lib/mock-users.ts`.
- Sign sessions with Ed25519 (private key in Vault per SPEC-010 §3.3); admin
  middleware verifies via JWKS endpoint instead of HMAC. Update
  `apps/admin/lib/session.ts` accordingly.
- Hash affiliate passwords with Argon2id (memory ≥ 19 MiB, t=2, p=1 minimum).
- Add hardware MFA across all roles per SPEC-010 §9.3.3 (TOTP + WebAuthn).
  Session JWT must carry `mfa_at` claim; high-sensitivity actions (MS-3 /
  MS-9 / MS-12) require `mfa_at` within the last 30 minutes.
- Configure Cloudflare Access with bypass policies for `/login` and
  `/me/affiliate/*` so external affiliates can reach the app surface, while
  `/`, `/dashboard`, and the six internal domain routes still require SSO.
- Replace `ADMIN_SESSION_SECRET` dev fallback in `lib/session.ts` with
  fail-closed behavior (no default secret in production).
- Remove the dev-credential hint block from `app/login/page.tsx` once real
  user-service is in place.

### LG-8: PWA / RWD followups

Phase A-C of the responsive + PWA pass landed 2026-05-10; the remaining
items below are deliberately deferred because they require either a
production deploy or backend coordination.

- Background sync for mutating requests. The web service worker
  (`apps/web/public/sw.js`) deliberately passes POST/PUT/DELETE through to
  the network — SPEC-002 funds-safety forbids silent retries of ledger
  writes. A real offline queue must (a) co-design idempotency keys with
  api-gateway, (b) surface pending mutations in the UI, and (c) reconcile
  failures without double-posting.
- Lighthouse PWA score validation. Local `next dev` cannot reach 100/100
  because Lighthouse insists on HTTPS for installability. Validate after
  the staging deploy and capture the run as evidence under SPEC-007.
- Real maskable icon artwork. `app/icon{0,1}.tsx` currently render a
  placeholder "P" glyph against Action Blue. Brand will supply a final
  mark; swap into the same ImageResponse generators (or precomputed
  PNGs) without changing the manifest URLs.
- Mobile-first sweep for legacy /web pages (`how-it-works`, `about`,
  `legal/*`) — they still rely on default `md:` breakpoints without `sm:`
  intermediate steps. Apply the same ladder pattern that the admin shell
  and login page now use.
- Web app `<noscript>` fallback explaining why the app needs JavaScript;
  currently only the LIFF shell carries one.
