# Runbook: KYC Provider Unavailable

## Alert
KYC SaaS API is unavailable for more than 5 minutes.

## Severity
P0

## Investigation
1. Check provider status page and API error responses.
2. Inspect kyc-service retry and queue depth.
3. Confirm whether tier upgrades or compliance reviews are blocked.

## Resolution
1. Switch the product flow to pending/manual review messaging.
2. Stop retry storms by applying backoff.
3. Reconcile queued provider events after provider recovery.
