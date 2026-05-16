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

Not yet implemented:

- Browser WebAuthn enrollment UX.
- Server WebAuthn registration start / finish endpoints.
- Server WebAuthn assertion start / finish endpoints.
- End-to-end browser evidence.
