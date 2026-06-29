# Public Beta External Readiness Blockers

Date: 2026-06-17

This document records the current external-readiness blockers for SPEC-002,
Public Beta launch gates, and SPEC-022. It separates items that are already
locally actionable from items that require external providers, staging
environments, devices, or manual operator evidence.

Central checkbox register: `docs/blocker-register.md`.

## Summary

| Area | Current status | Local action taken | Remaining blocker |
| --- | --- | --- | --- |
| SPEC-002 funds safety external readiness | Not launch-ready | Consolidated blocker map in this document; existing local skeletons noted below | Provider fixtures, real Postgres integration evidence, bank / gateway reconciliation inputs, R2 Object Lock archive, 1M acceptance, and E2E evidence |
| Public Beta launch gate | Not passed | Consolidated blocker map in this document | KYC/AML provider readiness, payment provider LOI/sandbox drills, release ops evidence, support/incident readiness evidence |
| SPEC-022 LIFF/login visibility | Code-complete but evidence-blocked | Existing recorder/preflight/finalize scripts are already present and fail closed | iOS/Android LIFF login evidence and manual QA marker logs |

## Explicit External Blockers

These blockers require external credentials, provider sandboxes, staging
deployments, target-account access, manual device runs, or dedicated acceptance
infrastructure. Do not mark the related gate complete until the corresponding
evidence artifact exists and the fail-closed checker passes.

- LINE / LIFF:
  - Real LINE Login / LIFF channel configuration.
  - iOS and Android LIFF login runs.
  - Sanitized LIFF evidence log and
    `docs/evidence/spec-022-liff/latest.json`.
- Manual QA:
  - Desktop and mobile login-event UI review.
  - Admin login-events RBAC, audit, query, and LIFF/WebView visibility review.
  - Sanitized manual QA log and
    `docs/evidence/spec-022-manual-qa/latest.json`.
- Cloudflare Access:
  - Terraform or OpenTofu installed in the target environment.
  - Cloudflare target-account variables and token.
  - Real plan / apply evidence at
    `docs/evidence/cloudflare-admin-access/latest.json`.
- Payment provider:
  - LOI or signed terms covering the intended business model, MCC,
    chargeback policy, and payout jurisdiction.
  - Sandbox webhook fixtures for authorization, capture, refund, dispute, and
    chargeback.
  - Refund and chargeback operational drill evidence.
  - PCI SAQ A scope evidence.
- KYC / AML provider:
  - Provider sandbox credentials.
  - Provider webhook evidence for HMAC validation, GREEN / RED / PENDING
    replay, RED-to-Frozen handling, and PEP / sanctions rescan behavior.
  - iProov real adapter readiness evidence, or a fallback ADR before reducing
    the gate level.
- R2 / WORM audit archive:
  - Cloudflare R2 bucket provisioning with Object Lock / retention.
  - Dev/staging/prod bucket naming, credentials, and endpoint configuration.
  - Archive manifest-chain evidence.
- Staging / release operations:
  - HTTPS staging deploy for LG-8 Lighthouse PWA evidence.
  - Release workflow run against staging or a release-candidate tag.
  - Rollback drill evidence and SPEC-010 MS-12 approval snapshot.
- Performance and E2E acceptance:
  - Dedicated 1M ledger-entry acceptance environment.
  - Recorded Layer 2 reconciliation timing and resource profile.
  - End-to-end evidence for wallet-ledger, payment webhook, reconciliation, and
    archive flows.

## SPEC-002 Funds Safety

Source checklist:
`docs/integrations/2026-05-06-spec-002-external-readiness-todo.md`.

### What Is Already Locally Present

- Wallet-ledger Postgres integration tests exist and are gated by
  `WALLET_LEDGER_TEST_DATABASE_URL`.
- `services/wallet-ledger-service/src/gateway.rs` defines the
  `PaymentGatewayAdapter` boundary and pure settlement reconciliation helpers.
- `services/wallet-ledger-service/src/reconciliation.rs` defines Layer 2 / 3 / 4
  layer identifiers and can persist `reconciliation_findings`.
- `services/wallet-ledger-service/src/archive.rs` provides a local
  `AuditArchive` implementation and manifest-chain verification.
- `services/api-gateway/src/lib.rs` includes EdDSA JWKS / JWT verification and
  an empty caller/path allowlist for non-production transition `x-user-id`
  fallback. Player order placement, affiliate dashboard/referral summary,
  responsible-gaming cooldown/self-exclusion, and wallet/ledger reads have all
  been cut over to Bearer-only gateway calls; downstream services still rely on
  gateway-asserted identity headers.

### Can Be Handled Locally Now

- Keep the blocker state documented and linked from the original readiness TODO.
- Run wallet-ledger integration tests when a migrated Postgres URL is available:

```bash
WALLET_LEDGER_TEST_DATABASE_URL=postgres://user:pass@localhost:5432/prophetica_wallet_ledger_test \
  rtk cargo nextest run -p wallet-ledger-service --locked
```

This workstation cannot truthfully complete the integration item without a
dedicated database URL and resulting evidence.

### Blockers For Manual Resolution

- Provision a dedicated wallet-ledger Postgres test database and capture the
  integration run proving migrations, INV-1 imbalance rejection, idempotency
  persistence, 1000 concurrent deposits, and `ledger_entries` immutability.
- Obtain payment gateway sandbox fixtures for successful deposits, refunds,
  chargebacks, duplicates, HMAC signatures, settlement reports, amount
  mismatches, and missing transaction cases.
- Implement the provider-specific `PaymentGatewayAdapter` only after the
  provider transaction-id format, idempotency mapping, and signing model are
  known.
- Choose and collect bank reconciliation inputs, either API sandbox or
  CSV/Excel statement samples, then implement the `BankTrust` adapter.
- Complete the reconciliation worker run paths for Layer 3 BankTrust and Layer
  4 GatewaySettlement, including failed-run status transitions and P0 alerting.
- Provision Cloudflare R2 buckets with Object Lock / retention, define
  dev/staging/prod bucket names, credentials, and endpoint configuration, then
  implement the R2 `AuditArchive` adapter.
- Run 1M ledger-entry acceptance for Layer 2 reconciliation and record hardware
  or container resources, runtime, and pass/fail evidence.
- Keep external `x-user-id` trust paths on verified JWT/session auth and
  explicitly define trusted internal headers from api-gateway to services. The
  player order placement path is cut over:
  `apps/web` must send a user-service Bearer token to api-gateway, and
  api-gateway rejects `x-transition-caller` + bare `x-user-id` for
  `POST /v1/markets/:market_id/orders`. Affiliate dashboard, referrals, and
  commission-summary are also cut over to Bearer-only gateway calls.
  Responsible-gaming cooldown and self-exclusion calls are Bearer-only as well.
  Wallet balance and ledger entry reads are Bearer-only as well; the transition
  allowlist is empty.
- Add E2E evidence for UI -> api-gateway -> wallet-ledger -> Postgres, payment
  webhook -> ledger -> idempotency replay, reconciliation worker -> finding /
  alert, and archive job -> WORM/local archive -> verify command.

## Public Beta Launch Gate

Source checklist: `TODO.md`, "Public Beta Launch Gate Blockers".

### Can Be Handled Locally Now

- Keep the gate failed in documentation until external evidence exists.
- Use existing local tests and evidence scripts as supporting proof only; they
  do not replace provider or release-operation evidence.

### Blockers For Manual Resolution

- LG-3 KYC / AML provider readiness:
  - Run `PostgresKycStore` against a migrated PostgreSQL database in CI or
    staging.
  - Capture provider sandbox webhook evidence for HMAC validation, GREEN / RED /
    PENDING replay, RED-to-Frozen handling, and PEP / sanctions rescan behavior.
  - Complete iProov real adapter readiness or publish a fallback ADR before
    lowering the gate.
- LG-2 Payment provider readiness:
  - Obtain LOI or signed terms that allow the intended business model, MCC,
    chargeback policy, and payout jurisdiction.
  - Complete sandbox webhook verification for authorization, capture, refund,
    dispute, and chargeback.
  - Run refund and chargeback operational drills.
  - Confirm PCI SAQ A scope and evidence requirements.
- LG-5 Release / Operations gate:
  - Run the release workflow against staging or a release-candidate tag and keep
    artifact evidence.
  - Verify production-targeting gates for secret scanning, SAST, SCA, image
    scanning, IaC validation, migration dry-run, and E2E smoke.
  - Complete rollback drill evidence and capture the SPEC-010 MS-12 approval
    snapshot.
- Support / incident readiness:
  - Capture evidence that support entrypoints, routing, SLA alerts, incident
    escalation, and ledger-impacting support operations are ready for public
    beta.

## SPEC-022

Source checklist: `TODO.md`, "SPEC-022 Completion Blockers".

### What Is Already Locally Present

- `docs/evidence/spec-022/README.md` documents the external evidence runbook.
- `tools/scripts/record-spec-022-liff-evidence.mjs` records sanitized LIFF
  evidence.
- `tools/scripts/record-spec-022-manual-qa-evidence.mjs` records sanitized
  manual QA evidence.
- `tools/scripts/preflight-spec-022-evidence.mjs`,
  `tools/scripts/finalize-spec-022-complete.mjs`, and
  `tools/scripts/check-spec-022-complete.mjs` fail closed while evidence is
  missing.

### Blockers For Manual Resolution

- Configure a target environment with `NEXT_PUBLIC_LIFF_ID`,
  `USER_SERVICE_LINE_LOGIN_CHANNEL_ID`, and `USER_SERVICE_BASE_URL`.
- Use a target member linked through `user_members.line_id`.
- Capture iOS and Android LINE LIFF login runs that produce `login_success`
  events with `login_surface = liff`.
- Save sanitized marker logs:
  - `.tmp/spec-022-liff.log`
  - `.tmp/spec-022-manual-qa.log`
- Generate:
  - `docs/evidence/spec-022-liff/latest.json`
  - `docs/evidence/spec-022-manual-qa/latest.json`
- Run:

```bash
rtk node tools/scripts/record-spec-022-liff-evidence.mjs .tmp/spec-022-liff.log
rtk node tools/scripts/record-spec-022-manual-qa-evidence.mjs .tmp/spec-022-manual-qa.log
rtk node tools/scripts/preflight-spec-022-evidence.mjs
rtk node tools/scripts/finalize-spec-022-complete.mjs
rtk node tools/scripts/check-spec-022-complete.mjs
```

SPEC-022 must remain incomplete until those commands pass against real external
evidence.
