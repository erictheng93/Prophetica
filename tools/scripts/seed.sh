#!/usr/bin/env bash
set -euo pipefail

cat <<'SEED'
seed bootstrap data:
- users: alice (TIER_2), bob (TIER_1), dmm-001 (TIER_2)
- market: mkt_dev_demo
SEED
