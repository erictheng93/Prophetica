# Runbook: API 5xx High

## Alert
API 5xx rate exceeds 5% for 2 minutes.

## Severity
P0

## Investigation
1. Check api-gateway logs grouped by route and upstream service.
2. Verify `/healthz/ready` for all Rust services.
3. Inspect recent deploys, DB pool saturation, Redis errors, and Kafka availability.

## Resolution
1. Roll back the api-gateway or failing upstream if tied to a release.
2. Temporarily disable affected write routes when user funds or orders are at risk.
3. Keep read-only routes available when possible.
