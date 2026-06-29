# Cloudflare Admin Access Evidence

This directory stores the target-account evidence required to close the
affiliate / referral P0 Cloudflare Access blocker.

For the full unblock checklist and failure triage, see
`docs/blocker-register.md`.

Run evidence collection only in the intended Cloudflare account environment:

```bash
rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs --before-apply
rtk node tools/scripts/apply-cloudflare-admin-access-evidence.mjs --apply
rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs
rtk node tools/scripts/finalize-affiliate-p0-complete.mjs
rtk node tools/scripts/check-affiliate-p0-complete.mjs
```

Required environment:

- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_API_TOKEN`
- `TF_VAR_admin_app_domain`
- `TF_VAR_admin_allowed_email_domains`
- `TF_VAR_admin_breakglass_email`
- `TF_VAR_internal_service_token_name`

`latest.json` must include Terraform/OpenTofu plan and apply log hashes, byte
counts, and success summaries. It also records target-account metadata:

- `account_id_sha256` and `account_id_suffix`
- `admin_app_domain`
- `allowed_email_domains_sha256`
- `breakglass_email_sha256`
- `internal_service_token_name`

Do not commit raw Terraform logs, Cloudflare API tokens, or breakglass email
plaintext. The recorder intentionally hashes sensitive target identifiers where
plain text is not required for review.
