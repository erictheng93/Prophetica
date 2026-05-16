# Prophetica Admin Cloudflare Access

This directory contains the account-side Cloudflare Zero Trust Access policy for
the admin app.

## Policy intent

- `/login*` is bypassed so unauthenticated users can reach the application login.
- `/me/affiliate*` is bypassed at Cloudflare Access so external affiliates can
  reach the affiliate portal; application session and RBAC still apply.
- The hostname-level admin application requires SSO for every other path.
- Internal SSO sessions require hardware MFA and expire after 30 minutes, matching
  the SPEC-010 step-up requirement for sensitive admin actions.

## Apply

Set a Cloudflare API token with Zero Trust Access application and policy write
permissions, then run:

```sh
export CLOUDFLARE_API_TOKEN=...
terraform -chdir=infra/cloudflare/zero-trust init
terraform -chdir=infra/cloudflare/zero-trust plan \
  -var cloudflare_account_id=... \
  -var admin_hostname=admin.example.com \
  -var internal_email_domain=example.com
terraform -chdir=infra/cloudflare/zero-trust apply \
  -var cloudflare_account_id=... \
  -var admin_hostname=admin.example.com \
  -var internal_email_domain=example.com
```

If the target tenant uses a dedicated identity provider allowlist, pass
`-var 'allowed_idp_ids=["<idp-id>"]'`.

Capture the plan and apply logs, then record evidence:

```sh
rtk node tools/scripts/validate-cloudflare-admin-access.mjs
rtk node tools/scripts/record-cloudflare-admin-access-evidence.mjs \
  path/to/terraform-plan.log \
  path/to/terraform-apply.log
```

The recorder writes `docs/evidence/cloudflare-admin-access/latest.json` with
SHA-256 hashes and byte counts for the logs. Do not generate this evidence from
dry-run or fake logs.

After apply, verify that:

- `https://<admin_hostname>/login` reaches the app without Cloudflare SSO.
- `https://<admin_hostname>/me/affiliate` reaches the app without Cloudflare SSO.
- `https://<admin_hostname>/`, `/dashboard`, and internal admin domains redirect
  to Cloudflare Access SSO and require hardware MFA.
