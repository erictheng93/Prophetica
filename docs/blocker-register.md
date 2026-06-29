# Blocker Register

Last updated: 2026-06-30

This is the central register for blockers that must be cleared before related
readiness, P0, or launch gates can be declared complete.

Use this file as the working checklist:

- Keep unresolved blockers under **Active Blockers** with `[ ]`.
- Move or change an item to `[x]` only after the success proof has been produced
  and the authoritative gate passes.
- Keep detailed runbooks or evidence instructions linked from each blocker.
- Do not mark a blocker complete from intent, partial implementation, or local
  code-only proof when the blocker explicitly requires external evidence.

## Active Blockers

### AFF-P0-CF: Affiliate / Referral P0 Cloudflare Access Evidence

Source gate:

```bash
rtk node tools/scripts/check-affiliate-p0-complete.mjs
```

Current top-level manifest blocker:

- `cloudflare-access-apply-evidence`

Already verified locally:

- Admin affiliate commission-adjustment and payout review actions work through
  signed gateway calls.
- `apps/admin` targeted tests pass.
- Cloudflare Access Terraform exists structurally.
- Evidence tooling fails closed when required target data, valid log hashes, or
  external evidence are missing.

Checklist:

- [ ] `AFF-P0-CF-001` Target IaC binary is available.
  - Current failing evidence:
    `preflight-cloudflare-admin-access-evidence.mjs --before-apply` reports
    neither `terraform` nor `tofu` is available.
  - Required to unblock:
    run in an environment with Terraform or OpenTofu available on `PATH`.
  - Success proof:
    preflight reports `terraform_or_tofu_available: true`.

- [ ] `AFF-P0-CF-002` Cloudflare target account variables are present.
  - Current failing evidence:
    preflight reports all required target variables missing.
  - Required to unblock:
    export `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN`,
    `TF_VAR_admin_app_domain`, `TF_VAR_admin_allowed_email_domains`,
    `TF_VAR_admin_breakglass_email`, and
    `TF_VAR_internal_service_token_name` in the target environment.
  - Success proof:
    preflight reports no `required_target_variables_missing` entries.

- [ ] `AFF-P0-CF-003` Target-account apply evidence is recorded.
  - Current failing evidence:
    `docs/evidence/cloudflare-admin-access/latest.json` does not exist.
  - Required to unblock:
    run the apply wrapper against the intended Cloudflare account.
  - Success proof:
    `latest.json` exists with valid `target_account`, `plan_log`, and
    `apply_log` fields.

- [ ] `AFF-P0-CF-004` Affiliate / referral P0 manifest is finalized.
  - Current failing evidence:
    `check-affiliate-p0-complete.mjs` reports manifest status `blocked`.
  - Required to unblock:
    run the finalizer after valid Cloudflare evidence exists.
  - Success proof:
    `check-affiliate-p0-complete.mjs` reports the manifest is complete.

Unblock procedure:

```bash
rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs --before-apply
rtk node tools/scripts/apply-cloudflare-admin-access-evidence.mjs --apply
rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs
rtk node tools/scripts/finalize-affiliate-p0-complete.mjs
rtk node tools/scripts/check-affiliate-p0-complete.mjs
```

Evidence contract for `docs/evidence/cloudflare-admin-access/latest.json`:

- `target_account.account_id_sha256`
- `target_account.account_id_suffix`
- `target_account.admin_app_domain`
- `target_account.allowed_email_domains_sha256`
- `target_account.breakglass_email_sha256`
- `target_account.internal_service_token_name`
- `plan_log.sha256`
- `plan_log.bytes`
- `plan_log.summary`
- `apply_log.sha256`
- `apply_log.bytes`
- `apply_log.summary`

Rules enforced by local tooling:

- SHA-256 values must be 64-character hex strings.
- Log byte counts must be positive integers.
- Plan/apply summaries must be non-empty recognized success summaries.
- Raw Terraform logs must not be committed.
- `CLOUDFLARE_API_TOKEN` must not appear in evidence.
- Breakglass email plaintext must not appear in evidence.

Failure triage:

- If preflight reports no Terraform/OpenTofu, install one of `terraform` or
  `tofu`, or run from a build/deploy host that already has one.
- If preflight reports missing target variables, export every variable listed in
  `required_target_variables_missing`.
- If the apply wrapper fails, do not hand-edit `latest.json`; inspect `.tmp/`
  logs locally, fix the Terraform/OpenTofu or Cloudflare account issue, then
  re-run the apply wrapper.
- If the recorder rejects logs, confirm the plan log has a recognized plan
  summary, the apply log has `Apply complete! Resources: ...`, and neither log
  contains `Error:`.
- If finalization fails, run
  `rtk node tools/scripts/check-affiliate-p0-complete.mjs`, fix the first
  missing or malformed artifact, then re-run the finalizer.

### SPEC-022: Login Event Intelligence External Evidence

Source manifest: `docs/spec-022-evidence-manifest.json`

Source gate:

```bash
rtk node tools/scripts/check-spec-022-complete.mjs
```

Checklist:

- [ ] `SPEC-022-001` Live LIFF identity bridge evidence is recorded.
  - Current blocker:
    real LINE Login / LIFF channel, linked `user_members.line_id` player, and
    iOS plus Android LIFF login runs are missing.
  - Required evidence:
    `docs/evidence/spec-022-liff/latest.json`
  - Recorder:
    `tools/scripts/record-spec-022-liff-evidence.mjs`
  - Existing runbook:
    `docs/evidence/spec-022/README.md`

- [ ] `SPEC-022-002` Manual QA evidence is recorded.
  - Current blocker:
    desktop web, mobile web, admin, and LIFF/WebView manual QA evidence must
    confirm approximate location copy and unknown fallback rendering.
  - Required evidence:
    `docs/evidence/spec-022-manual-qa/latest.json`
  - Recorder:
    `tools/scripts/record-spec-022-manual-qa-evidence.mjs`
  - Existing runbook:
    `docs/evidence/spec-022/README.md`

### Public Beta External Readiness

Source document:
`docs/launch-blockers/2026-06-17-public-beta-external-readiness.md`

These items require external credentials, provider sandboxes, staging
deployments, devices, or manual operator evidence.

- [ ] `PB-EXT-001` SPEC-002 funds safety external readiness evidence.
  - Required to unblock:
    provider fixtures, real Postgres integration evidence, bank/gateway
    reconciliation inputs, R2 Object Lock archive, 1M acceptance, and E2E
    evidence.
  - Detailed checklist:
    `docs/launch-blockers/2026-06-17-public-beta-external-readiness.md`

- [ ] `PB-EXT-002` Public Beta launch gate evidence.
  - Required to unblock:
    KYC/AML provider readiness, payment provider LOI/sandbox drills, release
    ops evidence, and support/incident readiness evidence.
  - Detailed checklist:
    `docs/launch-blockers/2026-06-17-public-beta-external-readiness.md`

- [ ] `PB-EXT-003` Cloudflare Access target-account evidence.
  - This is the same operational blocker as `AFF-P0-CF-001` through
    `AFF-P0-CF-004`.
  - Required evidence:
    `docs/evidence/cloudflare-admin-access/latest.json`

### High Peak Capacity Claims

Source document: `docs/architecture/high-peak-capacity-matrix.md`

- [ ] `CAP-001` Capacity levels above local/internal development remain
  unclaimable until their level gates have evidence.
  - Current blocker:
    the project must not claim L1/L2/L3/L4 public burst readiness without the
    workload profile and evidence required by the matrix.
  - Required evidence:
    see the L1-L4 gates in `docs/architecture/high-peak-capacity-matrix.md`.

## Completed Blockers

- [x] `AFF-P0-REVIEW-001` Admin affiliate P1 review actions and E2E flow.
  - Completed evidence:
    `apps/admin/tests/e2e/affiliate-review.spec.ts`
  - Validation:
    `rtk pnpm --filter @prophetica/admin test:e2e tests/e2e/affiliate-review.spec.ts`
    passed with 2 tests covering commission adjustment approval, payout
    rejection, signed gateway calls, and read-only rows after decision.
  - Gate coverage:
    `tools/scripts/check-affiliate-p0-complete.test.mjs` requires
    `admin-affiliate-review-e2e` before completion.

## Local Verification Commands

These commands validate blocker tooling without satisfying external blockers:

```bash
rtk node --test tools/scripts/*.test.mjs
rtk pnpm --filter @prophetica/admin test:e2e tests/e2e/affiliate-review.spec.ts
rtk node tools/scripts/preflight-cloudflare-admin-access-evidence.mjs --before-apply
rtk node tools/scripts/check-affiliate-p0-complete.mjs
```

On this local machine, the last two commands are expected to fail closed until
the Cloudflare target account apply evidence exists.
