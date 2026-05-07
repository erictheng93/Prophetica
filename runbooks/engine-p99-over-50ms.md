# Runbook: Engine P99 Over 50ms

## Alert
`histogram_quantile(0.99, proph_engine_match_latency_seconds) > 50ms`

## Severity
P0

## Investigation
1. Check matching-engine spans for `engine.order.apply` and `engine.event.publish`.
2. Compare hot market routing against SPEC-003 §17.3.
3. Inspect Kafka producer latency and WAL fsync duration.

## Resolution
1. Stop noncritical market launches.
2. Move hot markets to dedicated nodes or reduce batch contention.
3. Roll back the latest matching-engine release if the regression is release-linked.
