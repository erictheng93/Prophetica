# SPEC-008 DR Gap Fill PR Notes

## SPEC Refs

- `8981764 feat(matching-engine): add spec-008 durability contracts`
  - SPEC-008 §2.1 / §4.1 / §4.2
  - SPEC-003 §8.1-§8.4
- `e020efa feat(api-gateway): add halt governance and drill logs`
  - SPEC-008 §9 / §10 / §11
  - SPEC-010 §9.3 MS-9
- `fa99851 feat(wallet-ledger-service): enforce idempotency keys`
  - SPEC-002 §6.1
  - SPEC-008 §7.2
- `bd9b988 feat(dr): add spec-008 failover and backup automation`
  - SPEC-008 §3 / §5 / §6 / §8
- `9822dda fix(governance): close kill switch release loop`
  - SPEC-008 §9.3
  - SPEC-010 §9.3 MS-9
- `3d41b81 fix(governance): wire halt flag fanout`
  - SPEC-008 §9.2 / §9.3 / §12
  - SPEC-010 §9.3 MS-9

- `7a64748 fix(governance): add halt polling integration`
  - SPEC-008 §9.2 / §9.3 / §12
  - SPEC-010 §9.3 MS-9
- `5019454 fix(dr): tighten spec 008 merge blockers`
  - SPEC-008 §9 / §12
  - SPEC-010 §9.3.1 MS-10

## CI / Review Support

- `4d82b5e fix(ci): pin cargo-deny for rust 1.85`
  - Keeps the CI cargo-deny install compatible with the pinned Rust 1.85 toolchain.
- `67633e7 fix(ci): prepare buf breaking main ref`
  - Ensures detached pull-request checkouts have a local `main` ref for `buf breaking`.
- `9c0da2a fix(web): make test output portable`
  - Cleans stale `.test-out` output before web tests and avoids a quoted glob that fails on Linux CI.

## Merge Governance

- SPEC-010 §9.3.1 MS-10 applies because this branch touches matching-engine, ledger, settlement,
  and kill switch surfaces.
- Merge requires two reviewers, including one module owner, recorded in the PR review system.

## Commit History Note

`services/wallet-ledger-service/migrations/202605080001_create_drill_logs.sql` was committed in
`fa99851 feat(wallet-ledger-service): enforce idempotency keys`, while the API surface
`services/api-gateway/src/drill_logs.rs` landed in
`e020efa feat(api-gateway): add halt governance and drill logs`.

That ordering is a commit hygiene issue only. The follow-up feature flag migration is
`202605080002_create_feature_flags.sql`; reviewers should evaluate the final branch state rather
than infer ownership from the earlier drill log migration commit.
