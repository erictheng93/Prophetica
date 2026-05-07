# Runbook: Logging Pipeline Down

## Alert
Log ingestion pipeline is unavailable.

## Severity
P0

## Investigation
1. Check Loki `/ready` and storage errors.
2. Verify application stdout still emits JSON logs.
3. Check collector or promtail queue and retry state.

## Resolution
1. Restore Loki or switch ingestion to the standby target.
2. Preserve raw container logs until Loki ingestion is healthy.
3. Do not delete local logs before the gap is backfilled.
