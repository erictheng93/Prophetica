# Runbook: Latency Budget Breach

## Alert
SPEC-007 §6.4.4 latency budget P99 exceeds 1.5x or 2x budget.

## Severity
P1 at 1.5x, P0 at 2x.

## Investigation
1. Open Tempo traces for the alert `span` label.
2. Compare the observed P99 with `proph_latency_budget_hop_budget_seconds`.
3. Identify whether the breach is code, DB, Redis, Kafka, routing, or network related.

## Resolution
1. Freeze prod promotion for the affected path.
2. Optimize the owning hop; do not widen the budget automatically.
3. If topology is the cause, evaluate the SPEC-014 7-binary advisory path.
