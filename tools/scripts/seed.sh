#!/usr/bin/env bash
set -euo pipefail

SQL=$(cat <<'SEED'
INSERT INTO user_kyc_records
    (user_id, tier, status, provider, provider_applicant_id, document_id_hash, country, declared_monthly_income, idempotency_key)
VALUES
    ('alice', 2, 'passed', 'iproov', 'applicant_seed_alice', NULL, 'TW', 6000000, 'seed:kyc:alice:tier2'),
    ('bob', 1, 'passed', 'manual', NULL, repeat('b', 64), 'TW', NULL, 'seed:kyc:bob:tier1'),
    ('dmm-001', 2, 'passed', 'manual', NULL, NULL, 'TW', NULL, 'seed:kyc:dmm-001:tier2')
ON CONFLICT (idempotency_key) DO NOTHING;
SEED
)

if [[ -n "${DATABASE_URL:-}" ]] && command -v psql >/dev/null 2>&1; then
  printf '%s\n' "$SQL" | psql "$DATABASE_URL"
else
  cat <<SEED
seed bootstrap data (dry run; set DATABASE_URL and install psql to apply):
- users: alice (TIER_2), bob (TIER_1), dmm-001 (TIER_2)
- market: mkt_dev_demo

$SQL
SEED
fi
