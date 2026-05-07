# SPEC-008 Cross-Region Failover Runbook

This runbook covers planned Tokyo -> Singapore failover, disaster failover when Tokyo is unavailable, third-party provider fallback, and rollback criteria.

## Planned Failover With Maintenance Window

1. Announce the maintenance window and freeze non-essential write operations.
2. Confirm Singapore PostgreSQL standby is streaming async and Kafka MirrorMaker lag is <= 1 second.
3. Lower Cloudflare DNS TTL to 60 seconds before the traffic shift.
4. Switch edge traffic from Tokyo to Singapore.
5. Switch or promote the database primary according to the approved replication mode.
6. Start the Singapore standby engine with mirrored topics.
7. Run `tools/scripts/failover/canary-singapore.sh`.
8. Keep the maintenance window open until canary checks and monitoring are stable for the agreed observation period.

## Disaster Failover When Tokyo Is Unavailable

Automatic failover trigger conditions:

1. Tokyo edge is unhealthy or unreachable.
2. Tokyo DB primary is unhealthy or unreachable.
3. Singapore DR is ready.
4. Kafka MirrorMaker lag is within the SPEC-008 cross-region RPO boundary, <= 5 seconds.

Execution:

1. Run `tools/scripts/failover/failover-tokyo-singapore.sh`.
2. Run `tools/scripts/failover/canary-singapore.sh`.
3. Run third-party fallback scripts for CloudFront, payment, KYC, email, and SMS as needed.
4. Record RTO, RPO, deviations, and improvement actions in `drill_logs`.

## Canary Verification

The staging canary must verify:

1. Singapore edge health endpoint.
2. Singapore database query.
3. Matching-engine replay/read path.
4. Wallet read path.
5. Order book read path.
6. Third-party fallback health checks.

Any failed canary check triggers rollback.

## Rollback Triggers

Rollback is required when any of these are true:

1. Canary fails.
2. Singapore DB query fails after traffic shift.
3. Kafka replay lag exceeds 5 seconds after failover.
4. Payment, KYC, email, or SMS fallback health check fails.
5. Customer-facing error rate exceeds the incident threshold.

Rollback action:

1. Stop additional traffic migration.
2. Run the configured rollback command for the affected component.
3. Restore previous DNS or provider routing when data safety allows it.
4. Record the rollback in `drill_logs`.

## Third-Party Fallback

Each third-party switch script must run a target health check before switching and must have a rollback command configured:

1. Cloudflare -> AWS CloudFront DNS.
2. Payment gateway: ECPay -> NewebPay or Stripe.
3. KYC SaaS: SumSub -> Onfido.
4. Email provider fallback.
5. SMS provider fallback.

## Drill Documentation

Every planned or disaster drill must include:

1. Pre-plan.
2. Execution record.
3. Improvement actions within 24 hours.

Use `GET /api/internal/audit/drill-logs?from=&to=` for regulator review.
