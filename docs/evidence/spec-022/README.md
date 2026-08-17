# SPEC-022 Evidence Runbook

This directory documents the external evidence required before SPEC-022 can be
declared complete. The implementation is intentionally blocked until real LIFF
and manual QA evidence is recorded.

Do not commit LINE secrets, raw ID tokens, access tokens, session cookies,
passwords, OTPs, or raw user personal data in the logs used by these recorders.
The recorders fail closed if they see common token / credential markers.

## Required Environment

- A real LINE Login / LIFF channel for the target environment.
- `USER_SERVICE_LINE_LOGIN_CHANNEL_ID` configured for `user-service`.
- `USER_SERVICE_STORE=postgres` with migrations applied.
- A test player row in `user_members` whose `line_id` matches the LINE user ID
  returned by LINE Verify ID token.
- iOS and Android devices that can open the LIFF endpoint in the LINE app.
- Admin / manager credentials that can query `/security/login-events`.

## LIFF Identity Bridge Evidence

Copy `docs/evidence/spec-022/liff-log.example` to `.tmp/spec-022-liff.log`.
Flip these markers to `true` only after the real checks have been performed:

```text
line_channel_configured=true
ios_liff_login_success_event=true
android_liff_login_success_event=true
login_surface_liff_confirmed=true
user_members_line_id_link_confirmed=true
```

The operator should verify:

- `apps/liff` sends the LIFF ID token to `/api/auth/liff/session`.
- `user-service` verifies the ID token with LINE and does not trust frontend
  profile fields.
- The linked member is found through `user_members.line_id`.
- Both iOS and Android LIFF logins produce queryable `login_success` events.
- The stored events have `login_surface = liff`, `auth_method = liff`,
  `principal_type = user`, and the expected `user_id`.

Record evidence:

```bash
node tools/scripts/record-spec-022-liff-evidence.mjs .tmp/spec-022-liff.log
```

This writes `docs/evidence/spec-022-liff/latest.json`.

## Manual QA Evidence

Copy `docs/evidence/spec-022/manual-qa-log.example` to
`.tmp/spec-022-manual-qa.log`. Flip these markers to `true` only after the real
checks have been performed:

```text
desktop_web_recent_login_approximate_location=true
mobile_web_recent_login_approximate_location=true
admin_login_events_rbac_and_audit_checked=true
liff_webview_login_event_visible=true
unknown_fallback_visible=true
```

The operator should verify:

- Desktop web account security page shows recent login entries with masked IP
  and approximate location language.
- Mobile web account security page renders the same information without layout
  overlap.
- Admin login events page is RBAC-protected, can query login events, and full
  detail access increments / emits the audit path.
- LIFF/WebView login event is visible through the admin query path.
- Failed geolocation / device parsing displays `未知` rather than blank fields.

Record evidence:

```bash
node tools/scripts/record-spec-022-manual-qa-evidence.mjs .tmp/spec-022-manual-qa.log
```

This writes `docs/evidence/spec-022-manual-qa/latest.json`.

## Completion Check

After both evidence files are recorded, run the preflight, finalize the
manifest, and run the completion check:

```bash
node tools/scripts/preflight-spec-022-evidence.mjs
node tools/scripts/finalize-spec-022-complete.mjs
node tools/scripts/check-spec-022-complete.mjs
```

The preflight checks for the required local LIFF configuration, both `.tmp`
input logs, both generated evidence files, and the required evidence fields. It
does not replace the completion checker.

The finalize script fails closed and restores the blocked manifest when either
external evidence file is missing or does not satisfy every required marker.
SPEC-022 may only be declared complete if the checker passes and the manifest
still maps every SPEC-022 requirement to concrete implementation or evidence.
