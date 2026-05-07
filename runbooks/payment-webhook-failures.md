# Runbook: Payment Webhook Failures

## Alert
Payment gateway webhook failures exceed 10 per minute.

## Severity
P0

## Investigation
1. Check payment webhook signature verification errors.
2. Compare provider delivery logs against internal idempotency records.
3. Confirm wallet-ledger-service readiness and ledger commit latency.

## Resolution
1. Pause automatic crediting if ledger commit status is ambiguous.
2. Replay only idempotent webhook events after root cause is known.
3. Reconcile provider settlement exports against ledger transactions.
