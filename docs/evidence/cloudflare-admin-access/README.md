# Cloudflare Admin Access P0 Evidence

This directory stores the target-account evidence required to close the
Cloudflare Access P0 blocker.

The P0 gate is blocked until `latest.json` exists and contains SHA-256 digests
and success summaries for real Terraform/OpenTofu plan and apply logs from the
Cloudflare account that protects the admin app.

## Required target environment

Run these commands only in an environment with:

- `terraform` or `tofu` on `PATH`
- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_API_TOKEN`
- `TF_VAR_admin_app_domain`
- `TF_VAR_admin_allowed_email_domains`
- `TF_VAR_admin_breakglass_email`
- `TF_VAR_internal_service_token_name`

Do not commit secrets or raw Cloudflare credentials.

## Procedure

1. Check prerequisites:

   ```bash
   rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs --before-apply
   ```

2. Apply the Access policy and record evidence:

   ```bash
   rtk node tools/scripts/apply-cloudflare-admin-access-evidence.mjs --apply
   ```

3. Confirm the global P0 gate:

   ```bash
   rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs
   rtk node tools/scripts/check-affiliate-p0-complete.mjs
   ```

The apply wrapper writes temporary logs under `.tmp/` and then records
`docs/evidence/cloudflare-admin-access/latest.json` through
`tools/scripts/record-cloudflare-admin-access-evidence.mjs`. The recorder fails
closed if either log is empty, contains an error marker, or lacks a recognized
Terraform/OpenTofu success summary.
