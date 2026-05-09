# TODO

Last updated: 2026-05-09

## Public Beta Launch Gate Blockers

These items are not complete enough to count the Public Beta gate as passed.
They are tracked here so the current completed work can be committed without
losing the remaining launch work.

### LG-3: KYC / AML Provider Readiness

- Run `PostgresKycStore` against a real migrated PostgreSQL database in CI or
  staging, not only unit-contract coverage.
- Add transaction-boundary tests for SumSub webhook record updates plus
  screening-event inserts.
- Refresh SQLx offline metadata if the KYC service adopts compile-time checked
  SQL queries.
- Finish SumSub sandbox credentials and Tier 2 iframe E2E coverage.
- Capture webhook evidence for HMAC validation, GREEN / RED / PENDING replay,
  RED-to-Frozen handling, and PEP / sanctions rescan behavior.
- Complete iProov real adapter readiness, or publish a fallback ADR before
  reducing the gate level.

### LG-4: PoC Evidence

- Complete PoC-A live EdDSA / LIFF verification on iOS and Android devices.
- Retry PoC-B on Linux or CI with reachable `ws-gateway`, SumSub sandbox, and
  `next-on-pages` build path.

### LG-2: Payment Provider Readiness

- Obtain provider LOI or terms that explicitly allow the intended business
  model, MCC, chargeback policy, and required payout jurisdiction.
- Complete sandbox webhook verification for authorization, capture, refund,
  dispute, and chargeback events.
- Run a refund and chargeback operational drill.
- Confirm PCI SAQ A scope and evidence requirements.

### LG-5: Release / Operations Gate

- Run the release workflow against a staging or release-candidate tag and keep
  the resulting artifact evidence.
- Add or verify production-targeting gates for secret scanning, SAST, SCA,
  image scanning, IaC validation, migration dry-run, and E2E smoke checks.
- Complete rollback drill evidence for the first public-beta deployment path.
- Capture the SPEC-010 MS-12 approval snapshot required by the launch gate.

### LG-6: Support / Incident Readiness

- Stand up the support queue, escalation owner mapping, and public-beta SLA.
- Prepare a sample customer-support evidence pack.
- Run and document an S0 / S1 incident drill.
- Link final runbooks from the launch-gate evidence package.

### LG-1: Legal / Jurisdiction Readiness

- Complete legal memo coverage for the initial market list.
- Confirm jurisdiction allow / deny rules for the beta launch scope.
- Publish any required market-specific restrictions before launch.
