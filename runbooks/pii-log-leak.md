# Runbook: PII Log Leak

## Alert
PII is detected in logs or traces.

## Severity
P0

## Investigation
1. Identify the exact field, service, and time window.
2. Confirm whether Loki, Tempo, or downstream exports contain the value.
3. Locate the code path that emitted the unredacted field.

## Resolution
1. Stop the offending deployment or route.
2. Patch redaction and redeploy.
3. Follow the data retention and incident notification process with Compliance.
