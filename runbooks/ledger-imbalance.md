# Runbook: Ledger Imbalance

## Alert
`proph_ledger_txn_failed_total{reason="imbalance"} > 0`

## Severity
P0

## Symptoms
Ledger invariant validation rejected a transaction or a DB constraint trigger rolled back entries.

## Investigation
1. Check Loki for `service=wallet-ledger-service level=error`.
2. Query `ledger_transactions` by the alert transaction id or idempotency key.
3. Query `ledger_entries` for the same transaction and verify debit and credit totals.

## Resolution
1. Disable new wallet mutations through the feature flag or gateway route block.
2. Escalate to Senior Engineer and Compliance.
3. Apply only approved reversal transactions; do not edit ledger rows in place.

## Postmortem
Create an RCA in `docs/postmortems/` within 24 hours.
