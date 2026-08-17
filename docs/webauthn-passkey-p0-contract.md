# WebAuthn / Passkey P0 Contract

Last updated: 2026-05-16

This contract defines the remaining code-side requirement before the affiliate
auth P0 can be formally declared complete. Existing `mfa_at` / `amr` claims and
TOTP fallback are not a substitute for a real WebAuthn ceremony.

## Required server-side surfaces

The implementation must use a vetted WebAuthn verifier such as `webauthn-rs`.
Do not hand-roll challenge, client-data, authenticator-data, signature, counter,
or origin verification.

Required endpoints:

- `POST /v1/auth/webauthn/register/start`
  - Authenticated user starts enrollment.
  - Server generates WebAuthn creation options.
  - Server stores the returned registration state server-side with a short TTL.
- `POST /v1/auth/webauthn/register/finish`
  - Browser submits `RegisterPublicKeyCredential`.
  - Server verifies it with the stored registration state.
  - Server persists the resulting passkey credential to the user account.
- `POST /v1/auth/webauthn/assert/start`
  - User starts step-up / assertion.
  - Server generates WebAuthn request options using the user's registered
    passkeys.
  - Server stores the returned authentication state with a short TTL.
- `POST /v1/auth/webauthn/assert/finish`
  - Browser submits `PublicKeyCredential`.
  - Server verifies it with the stored authentication state and registered
    passkeys.
  - Server updates credential counter / metadata and issues a session with
    fresh `mfa_at` and `amr=["pwd","hwk"]`.

## Required persistence

The server must persist:

- User WebAuthn stable UUID.
- Credential id.
- Public key / COSE credential data.
- Signature counter.
- Backup eligibility / backup state if exposed by the verifier.
- User verification status / policy.
- Enrollment metadata: created time, display name, last used time, revoked time.
- Short-lived challenge state for registration and assertion, keyed by opaque
  challenge id with expiry.

## Required validation

The P0 declaration must include evidence for:

- Successful passkey enrollment round trip against a browser-generated
  `RegisterPublicKeyCredential`.
- Replayed registration finish is rejected after the stored state is consumed.
- Successful passkey assertion round trip against a browser-generated
  `PublicKeyCredential`.
- Replayed assertion finish is rejected after state consumption.
- Wrong origin / RP id is rejected.
- Unknown credential id is rejected.
- Revoked credential id is rejected.
- Session issued after assertion contains fresh `mfa_at` and `amr=["pwd","hwk"]`.
- Admin mutating high-sensitivity route accepts fresh hardware MFA and rejects
  stale or non-hardware MFA.

## Current repo status

Implemented:

- `user-service` can emit and verify session `mfa_at` claims.
- Admin middleware enforces 30-minute hardware step-up for mutating
  high-sensitivity routes.
- `user-service` local provider supports TOTP step-up and persisted recovery-code
  consumption.
- Strict local-account policy can reject admin / manager / affiliate accounts
  that do not have configured hardware MFA evidence.
- Durable WebAuthn credential and challenge-state tables are defined in
  `services/user-service/migrations/202605160003_create_webauthn_tables.sql`.
- `services/user-service/src/webauthn_store.rs` defines the store trait and an
  in-memory implementation covering one-time challenge consumption and
  credential revocation lifecycle.
- `services/user-service/src/webauthn_runtime.rs` integrates the official
  `webauthn-rs` verifier crate and can generate passkey registration options
  plus serializable registration state from relying-party configuration, then
  persist that state into `WebauthnStore` with a short challenge TTL.
- `POST /v1/auth/webauthn/register/start` is available for authenticated
  sessions and returns a challenge id plus browser `publicKey` creation options.
- Runtime support for `finish_passkey_registration` is wired through
  `webauthn-rs` payload/state types, so the finish endpoint can use the official
  verifier instead of hand-rolled validation.
- `POST /v1/auth/webauthn/register/finish` consumes the stored registration
  challenge, verifies submitted browser credential payloads through
  `webauthn-rs`, and persists the resulting credential record. Invalid browser
  payload coverage confirms the endpoint fails closed through the verifier path.
- `POST /v1/auth/webauthn/assert/start` returns browser `publicKey` request
  options for active credentials and stores short-lived assertion state.
- `POST /v1/auth/webauthn/assert/finish` consumes assertion state, verifies
  submitted browser credential payloads through `webauthn-rs`, updates
  credential metadata, and issues a fresh hardware-MFA session on successful
  assertion. Invalid browser payload coverage confirms the endpoint fails
  closed through the verifier path.
- `POST /v1/auth/webauthn/credentials/revoke` is available for authenticated
  users and revokes one of their active passkeys so revoked credentials are
  excluded from subsequent assertion starts.
- `AuthConfig::from_env()` enables WebAuthn when
  `USER_SERVICE_WEBAUTHN_RP_ID` and `USER_SERVICE_WEBAUTHN_RP_ORIGIN` are set.
  Core-api Postgres mode wires `PostgresWebauthnStore` into the auth router so
  challenge / credential state is durable outside local tests.
- Admin passkey UX exists in `apps/admin/components/passkey-security-panel.tsx`
  and calls `apps/admin/app/api/auth/passkeys/[action]/route.ts`; the BFF proxy
  keeps the user-service bearer session in httpOnly cookies while the browser
  performs WebAuthn ceremonies.
- `apps/admin/tests/e2e/passkey-webauthn.spec.ts` defines the browser evidence
  flow and writes `.tmp/webauthn-passkey.log` markers for the fail-closed
  recorder.
- `tools/scripts/record-webauthn-passkey-evidence.mjs` now fails closed unless
  the browser evidence log contains explicit markers for registration round
  trip, registration replay rejection, assertion round trip, assertion replay
  rejection, wrong-origin rejection, unknown-credential rejection,
  revoked-credential rejection, fresh hardware-MFA session, and admin step-up
  fresh/stale outcomes.

Completion evidence:

- `pnpm --filter @prophetica/admin test:e2e:passkey`: 1 passed.
- `node tools/scripts/record-webauthn-passkey-evidence.mjs
  .tmp/webauthn-passkey.log`: recorded
  `docs/evidence/webauthn-passkey/latest.json` with every required marker true.
