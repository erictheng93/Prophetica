# Affiliate / Referral P0 Completion Checklist

Last updated: 2026-05-17

This checklist maps the P0 completion objective to concrete artifacts, commands,
and evidence. Treat the P0 objective as incomplete until every row is complete
and `tools/scripts/check-affiliate-p0-complete.mjs` exits successfully.

## Success criteria

| Requirement | Required artifact or command | Completion evidence | Current status |
| --- | --- | --- | --- |
| Public signup captures and persists referral attribution through durable login | `docs/evidence/public-signup-referral/latest.json` | JSON markers: `signup_submit_path`, `referral_cookie_submitted`, `registration_endpoint_confirmed`, `durable_login_confirmed`, `referral_capture_confirmed` all `true` | Complete based on existing evidence manifest |
| Browser WebAuthn/passkey lifecycle works end to end | `docs/evidence/webauthn-passkey/latest.json` | JSON markers: registration, assertion, replay rejection, wrong-origin rejection, unknown credential rejection, revoked credential rejection, fresh MFA session, fresh admin step-up accept, stale admin step-up reject all `true` | Complete based on recorded browser evidence |
| User service supports WebAuthn credential revocation | `services/user-service/src/auth.rs`, `services/user-service/tests/auth.rs` | Targeted Rust test `rtk cargo test -p user-service --test auth webauthn_credential_revoke_requires_session_and_removes_active_credential --locked` passed earlier | Complete based on targeted test evidence |
| Admin passkey UI and BFF proxy support register/assert/revoke and session refresh | `apps/admin/components/passkey-security-panel.tsx`, `apps/admin/app/api/auth/passkeys/[action]/route.ts`, `apps/admin/lib/passkey-client.ts`, `apps/admin/tests/passkey-security.test.ts` | `rtk pnpm --filter @prophetica/admin test` and `rtk pnpm --filter @prophetica/admin test:e2e:passkey` passed earlier | Complete based on targeted test evidence |
| Cloudflare Access policy exists structurally for public login/affiliate routes and protected internal admin routes | `infra/cloudflare/zero-trust/admin-access.tf`, `tools/scripts/validate-cloudflare-admin-access.mjs` | `rtk node tools/scripts/validate-cloudflare-admin-access.mjs` passed 9 checks earlier | Structurally complete |
| Cloudflare Access policy has been applied to the real target account | `docs/evidence/cloudflare-admin-access/latest.json` | `plan_log` and `apply_log` each contain a 64-character SHA-256, positive byte count, and non-empty success summary from real Terraform/OpenTofu logs | Blocked: target-account evidence missing |
| Cloudflare target environment is ready before apply | `tools/scripts/preflight-cloudflare-admin-access-evidence.mjs --before-apply` | Command exits successfully in the target environment with `terraform` or `tofu` and required Cloudflare variables present | Blocked in this environment |
| Cloudflare apply logs are captured and recorded through the approved path | `tools/scripts/apply-cloudflare-admin-access-evidence.mjs --apply`, `tools/scripts/record-cloudflare-admin-access-evidence.mjs` | Wrapper completes, recorder writes `docs/evidence/cloudflare-admin-access/latest.json` and rejects failed or malformed logs | Blocked until target-account apply |
| Global P0 gate accepts the current state | `tools/scripts/check-affiliate-p0-complete.mjs` | Command exits successfully and reports complete manifest evidence count | Blocked by Cloudflare apply evidence |

## Final declaration procedure

Run the Cloudflare sequence in the target account environment:

```bash
rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs --before-apply
rtk node tools/scripts/apply-cloudflare-admin-access-evidence.mjs --apply
rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs
rtk node tools/scripts/check-affiliate-p0-complete.mjs
```

Only after the global P0 gate succeeds should
`docs/affiliate-p0-evidence-manifest.json` be changed to `status: "complete"`
with no `blocking_items`, followed by another successful
`rtk node tools/scripts/check-affiliate-p0-complete.mjs`.
