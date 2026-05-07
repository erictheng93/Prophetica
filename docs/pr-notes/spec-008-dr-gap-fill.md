# SPEC-008 DR Gap Fill PR Notes

## SPEC refs

- SPEC-008 §9.2 / §9.3: kill switch halt and MS-9 release.
- SPEC-010 §9.3 MS-9: 2-of-3 CTO / Compliance / Senior Engineer release governance.
- SPEC-010 §9.3.1 MS-10: sensitive module 4-eye review before merge.

## Commit history note

`services/wallet-ledger-service/migrations/202605080001_create_drill_logs.sql` was committed in
`fa99851 feat(wallet-ledger-service): enforce idempotency keys`, while the API surface
`services/api-gateway/src/drill_logs.rs` landed in
`e020efa feat(api-gateway): add halt governance and drill logs`.

That ordering is a commit hygiene issue only. The follow-up feature flag migration is
`202605080002_create_feature_flags.sql`; reviewers should evaluate the final branch state rather
than infer ownership from the earlier drill log migration commit.
