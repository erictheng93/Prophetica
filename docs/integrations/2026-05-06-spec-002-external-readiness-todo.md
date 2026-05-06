# SPEC-002 External Readiness TODO

> Date: 2026-05-06
> Scope: funds safety external dependencies, integration verification, and acceptance testing.

This checklist tracks what must be prepared outside the core Rust unit-test layer before SPEC-002 can be considered fully production-ready.

## 1. Postgres Integration Test Environment

- [ ] Create a dedicated test database, for example `prophetica_wallet_ledger_test`.
- [ ] Set `WALLET_LEDGER_TEST_DATABASE_URL` for local and CI runs.
- [ ] Run wallet-ledger Postgres integration tests with migrations enabled.
- [ ] Confirm the following tests execute rather than skip:
  - [ ] migration application
  - [ ] INV-1 deferred trigger rejection for imbalanced transactions
  - [ ] idempotency records persistence
  - [ ] 1000 concurrent deposits
  - [ ] `ledger_entries` immutability trigger
- [ ] Add a CI job that provisions Postgres and runs `rtk cargo nextest run -p wallet-ledger-service --locked`.

Suggested local command:

```powershell
$env:WALLET_LEDGER_TEST_DATABASE_URL="postgres://user:pass@localhost:5432/prophetica_wallet_ledger_test"
rtk cargo nextest run -p wallet-ledger-service --locked
```

## 2. Payment Gateway Fixtures

- [ ] Collect webhook payload samples for successful deposits.
- [ ] Collect webhook payload samples for refunds and chargebacks.
- [ ] Collect duplicate webhook samples.
- [ ] Define provider transaction id format and idempotency mapping.
- [ ] Prepare HMAC signing secrets for local/staging.
- [ ] Collect settlement report samples with:
  - [ ] successful deposits
  - [ ] refunds
  - [ ] chargebacks
  - [ ] amount mismatch cases
  - [ ] missing transaction cases
- [ ] Implement provider-specific `PaymentGatewayAdapter`.
- [ ] Add integration tests from fixture files.

## 3. Bank Reconciliation Inputs

- [ ] Choose first bank reconciliation source: API sandbox or CSV/Excel statement.
- [ ] Collect sample bank statement files.
- [ ] Define required fields:
  - [ ] date/time
  - [ ] transaction id or reference
  - [ ] credit amount
  - [ ] debit amount
  - [ ] ending balance
  - [ ] account identifier
- [ ] Define ledger aggregate used for bank trust reconciliation.
- [ ] Implement BankTrust adapter.
- [ ] Persist Layer 3 findings into `reconciliation_findings`.

## 4. Reconciliation Worker Completion

- [ ] Extend worker beyond Layer 2 player liability.
- [ ] Add Layer 3 BankTrust run path.
- [ ] Add Layer 4 GatewaySettlement run path.
- [ ] Mark reconciliation runs as `failed` when worker execution fails.
- [ ] Decide alerting target for P0 findings.
- [ ] Emit P0 alert for critical unresolved findings.
- [ ] Decide whether and when to post `Suspense.Reconciliation` ledger entries.
- [ ] Add tests for finding persistence and run status transitions.

## 5. WORM / Audit Archive

- [ ] Create Cloudflare R2 bucket for ledger audit archive.
- [ ] Enable Object Lock / retention policy for the bucket.
- [ ] Define dev/staging/prod bucket names.
- [ ] Prepare R2 credentials and endpoint configuration.
- [ ] Implement R2 `AuditArchive` adapter.
- [ ] Add daily job to export canonical `ledger_entries` for a date.
- [ ] Compute daily hash from exported entries.
- [ ] Store manifest with date, object key, hash, previous hash, and entry count.
- [ ] Add verification command for a 30-day manifest chain.
- [ ] Decide whether cold archive also needs S3 Glacier dual-write.

## 6. Performance / Acceptance Suite

- [ ] Prepare a Postgres environment large enough for 1M ledger entries.
- [ ] Add a benchmark or acceptance test for Layer 2 reconciliation over 1M entries.
- [ ] Verify reconciliation completes within 5 minutes.
- [ ] Generate 30 days of audit manifests and ledger-entry payloads.
- [ ] Verify 30-day hash-chain validation completes within 30 minutes.
- [ ] Record hardware/container resources used for the benchmark.
- [ ] Add acceptance results to PR notes before marking SPEC-002 complete.

## 7. Auth / Authorization

- [ ] Choose JWT issuer and validation model.
- [ ] Provide JWKS URL or shared secret for local/staging.
- [ ] Define required claims:
  - [ ] `sub`
  - [ ] `roles`
  - [ ] `scopes`
  - [ ] `kyc_tier`
  - [ ] `risk_tier`
- [ ] Replace api-gateway `x-user-id` stub with JWT/session verification.
- [ ] Define trusted internal identity headers sent from api-gateway to services.
- [ ] Add authorization guard for `/v1/ledger/entries`.
- [ ] Add tests for unauthenticated, user, support, and admin access.

## 8. E2E Coverage

- [ ] Add E2E path: UI -> api-gateway -> wallet-ledger-service -> Postgres.
- [ ] Add E2E path: payment webhook -> gateway adapter -> ledger transaction -> idempotency replay.
- [ ] Add E2E path: reconciliation worker -> finding persistence -> alert/log.
- [ ] Add E2E path: archive job -> WORM/local archive -> verify command.
- [ ] Document which E2E tests run locally, in CI, and only in staging.

## 9. ts-sdk Follow-ups

- [ ] After LINE `id_token -> prophetica` session/JWT bridge exists, make `apps/liff` the first non-web player-facing consumer of `@prophetica/ts-sdk` wallet data.
- [ ] Defer `apps/admin` as a ts-sdk consumer until admin-only services/endpoints exist; its API surface should not be forced through player wallet/market clients.
- [ ] When there are at least three real consumers or a concrete duplication pain point, extract remaining `apps/web/lib/{market-api,engine-api,position-api}.ts` client logic into `packages/ts-sdk/src/{market,engine,positions}/`.
- [ ] Keep `apps/web/lib/*` adapters as UI-facing compatibility wrappers while migrating to ts-sdk clients, preserving fallback behavior during each step.

## Current Testing Status

- Unit tests: mostly covered for the Phase 1 ledger core.
- Integration tests: partially covered, but DB-dependent tests require `WALLET_LEDGER_TEST_DATABASE_URL`.
- E2E tests: not complete.
- Performance acceptance: not complete.
