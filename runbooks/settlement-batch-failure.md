# Runbook: Settlement Batch Failure

## Alert
Settlement batch fails or misses the required completion window.

## Severity
P0

## Investigation
1. Check settlement-worker logs for failed market ids.
2. Verify wallet-ledger-service readiness and transaction failures.
3. Inspect DB locks and batch size against SPEC-004 off-peak rules.

## Resolution
1. Pause new settlement batch starts.
2. Resume from the last idempotent checkpoint.
3. Escalate disputed or partially settled markets to Compliance.
