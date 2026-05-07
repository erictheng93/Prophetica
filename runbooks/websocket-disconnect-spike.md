# Runbook: WebSocket Disconnect Spike

## Alert
WebSocket disconnect rate exceeds 50%.

## Severity
P0

## Investigation
1. Check ws-gateway connection counts and disconnect reasons.
2. Inspect Kafka consumer lag for market event topics.
3. Compare Cloudflare WebSocket errors against origin logs.

## Resolution
1. Enable fail-soft polling for clients.
2. Drain unhealthy ws-gateway instances.
3. Rebalance subscribers or scale ws-gateway capacity.
