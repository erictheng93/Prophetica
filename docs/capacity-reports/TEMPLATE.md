# Capacity Report Template

> Copy this template to `YYYY-MM-DD-L<level>-<scenario>-capacity-report.md`.
> A capacity level is not considered passed until every required section is filled
> with measured evidence.

## 1. Summary

| Field | Value |
| --- | --- |
| Target level | L1 / L2 / L3 |
| Scenario | Small burst / regional event / national event |
| Result | PASS / FAIL |
| Test date | YYYY-MM-DD |
| Git commit SHA | `<sha>` |
| Deployed artifacts | `<service>:<version>` |
| Test owner | `<name>` |

## 2. Environment

| Component | Version / size / region |
| --- | --- |
| API gateway | |
| Matching engine | |
| Wallet ledger | |
| WS gateway | |
| PostgreSQL | |
| Redis | |
| Kafka | |
| Observability stack | |
| Network / CDN / WAF | |

## 3. Workload Profile

| Input | Value |
| --- | ---: |
| Concurrent users | |
| Active order users | |
| WebSocket subscribers | |
| Order TPS | |
| Cancel TPS | |
| Read QPS | |
| Fan-out messages/sec | |
| Markets | |
| Hot-market skew, top 1 | |
| Hot-market skew, top 5 | |
| Hot-market skew, top 10 | |
| Funded users | |
| Test duration | |

## 4. Test Phases

| Phase | Duration | Expected behavior | Result |
| --- | ---: | --- | --- |
| Warm-up | | No SLO judgment | PASS / FAIL |
| Steady state | | Meets target level SLOs | PASS / FAIL |
| Spike | | No unbounded queue or lag growth | PASS / FAIL |
| Reconnect storm | | WS clients recover sequence gaps | PASS / FAIL |
| Dependency fault | | Readiness/load shedding behaves as designed | PASS / FAIL |
| Recovery | | System returns to steady-state SLOs | PASS / FAIL |

## 5. Latency And Errors

| Route / operation | P50 | P95 | P99 | P999 | Error rate |
| --- | ---: | ---: | ---: | ---: | ---: |
| `POST /api/v1/markets/:market_id/orders` | | | | | |
| `DELETE /api/v1/orders/:order_id` | | | | | |
| `GET /api/v1/markets/:market_id/book` | | | | | |
| matching internal command handling | | | | | |
| wallet ledger post transaction | | | | | |
| WS engine-to-client delivery | | | | | |

## 6. Saturation And Backpressure

| Signal | Peak | Steady-state average | Pass threshold | Result |
| --- | ---: | ---: | ---: | --- |
| Matching command queue depth | | | | PASS / FAIL |
| Matching event broadcast lag | | | | PASS / FAIL |
| API gateway rejected/shedded requests | | | | PASS / FAIL |
| PostgreSQL pool saturation | | | | PASS / FAIL |
| PostgreSQL lock wait P99 | | | | PASS / FAIL |
| Kafka producer error count | | | | PASS / FAIL |
| Kafka critical consumer lag | | | | PASS / FAIL |
| WS slow consumers | | | | PASS / FAIL |
| WS reconnect rate | | | | PASS / FAIL |

## 7. Data Integrity

| Check | Result | Evidence |
| --- | --- | --- |
| Accepted orders lost | PASS / FAIL | |
| Duplicate order effects | PASS / FAIL | |
| Ledger INV-1 through INV-6 violations | PASS / FAIL | |
| Idempotency replay correctness | PASS / FAIL | |
| Kafka replay gap | PASS / FAIL | |
| WS sequence-gap recovery | PASS / FAIL | |

## 8. Evidence Links

- Prometheus query export:
- Grafana dashboard snapshot:
- Load generator logs:
- Service logs:
- Database slow query log:
- Kafka lag export:
- Incident notes, if any:

## 9. Decision

Target level: L1 / L2 / L3

Decision: PASS / FAIL

Reason:

```text
Write the concrete reason here. Do not use "looks good" or "seems stable".
Reference the exact metrics that justify the decision.
```

Follow-up work:

- [ ] Item 1
- [ ] Item 2
- [ ] Item 3
