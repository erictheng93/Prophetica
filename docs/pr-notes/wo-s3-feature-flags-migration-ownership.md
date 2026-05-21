# WO-S3 Feature Flags Migration Ownership

## Decision

Choose option A: api-gateway owns an idempotent `feature_flags` migration in its
own migration set, matching the existing wallet-ledger definition byte-for-byte.

## Why

`api-gateway` reads and polls `payments.real_money_enabled`, so a clean database
must not require wallet-ledger to migrate first before the gateway can establish
the fail-closed flag state. Keeping the migration in api-gateway makes that
dependency explicit while preserving the existing wallet-ledger behavior for
`engine.halted`.

## Trade-off

- Rejected B for this work order: extracting a shared SQLx migration source is
  architecturally cleaner long term, but it would require changing how existing
  service migration sets are packaged and adopted. That is broader than WO-S3.
- Rejected C: documenting deployment order alone preserves the hidden coupling
  that caused the core-api Postgres migration collision class of failures.
- Accepted A's downside: two services now carry the same idempotent DDL. The
  drift risk is bounded by keeping both migrations `CREATE TABLE IF NOT EXISTS`
  plus `ON CONFLICT DO NOTHING`, and by an sdk test that asserts the two SQL
  files remain identical, including the check constraint and JSONB metadata
  default.

## Security

LG-2 remains fail-closed. `payments.real_money_enabled` is seeded to `FALSE`;
read failures still default to disabled outside production and bail during
production boot.
