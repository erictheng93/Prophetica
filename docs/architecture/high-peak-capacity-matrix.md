# High Peak Capacity Matrix

> Status: Proposed capacity gate
> Last updated: 2026-05-09
> Scope: concert, election, presidential election, World Cup, and other burst traffic events

## 1. Purpose

This document defines Prophetica's high-peak capacity levels and the code, deployment,
observability, and load-test evidence required before each level can be claimed.

The current Rust + tokio + axum/tonic + PostgreSQL + Redis + Kafka direction is a
valid technical foundation for high-concurrency systems. The current implementation,
however, must not be represented as ready for national-scale burst traffic until the
gates below are implemented and verified.

## 2. Capacity Terms

Concurrent users are not enough to prove capacity. Every capacity claim must state all
of these workload inputs:

| Term | Definition | Required in reports |
| --- | --- | --- |
| Concurrent users | Users connected or active in the same test window | Yes |
| Active order users | Users submitting orders or cancellations in the same minute | Yes |
| WebSocket subscribers | Open WS connections receiving market or user streams | Yes |
| Order TPS | Sustained `PlaceOrder` commands per second | Yes |
| Cancel TPS | Sustained `CancelOrder` commands per second | Yes |
| Read QPS | REST/gRPC read requests per second, including order book snapshots | Yes |
| Fan-out rate | WebSocket messages delivered per second after market events | Yes |
| Hot-market skew | Share of traffic hitting the top 1, 5, and 10 markets | Yes |
| Test duration | Warm-up, steady-state, and spike duration | Yes |

No document or PR may claim "supports 10k concurrent" or "supports 100k concurrent"
without the corresponding workload profile and the evidence artifact.

## 3. Current Baseline Assessment

As of 2026-05-09, the repo should be treated as below Level 1 for public peak events.

Positive foundation:

- The Rust workspace uses axum, tonic, tokio, sqlx, rdkafka, and fred as the intended
  backend stack.
- The matching engine has an in-process order book, per-market command channel,
  broadcast events, WAL/snapshot recovery primitives, and Kafka projection types.
- The wallet ledger service has PostgreSQL transactions, idempotency records,
  advisory locks, balance validation, deferred balance triggers, and immutable ledger
  entry triggers.
- Infrastructure includes local PostgreSQL, Redis, Kafka, Prometheus, Grafana, Tempo,
  Loki, PostgreSQL replication files, and Kafka MirrorMaker 2 files.

Current blockers:

- `matching-engine` startup uses a single `current_thread` runtime pinned to one CPU
  core. That can be valid for a hot-shard model, but it is not yet a complete horizontal
  scaling model.
- HTTP and gRPC now share one `InMemoryEngineService` when launched through
  `matching_engine::api::run()`. Production evidence is still required for the
  intended hot/cold deployment topology and routing model.
- Kafka projection code is wired into the matching event path when
  `MATCHING_ENGINE_KAFKA_BROKERS` is set and the binary is built with the `kafka`
  feature. Production broker/topic evidence is still required.
- `ws-gateway` is still a shared service stub plus payload helpers, not a production
  fan-out gateway.
- `matching-engine` readiness now checks configured Kafka broker TCP dependencies.
  Other services still need dependency-aware readiness coverage for PostgreSQL, Redis,
  Kafka, and downstream service dependencies.
- `matching-engine` now emits HTTP route latency, internal command latency, and
  per-market command queue depth metrics. Remaining observability gaps include
  production dashboard snapshots, Kafka lag capture, DB contention, backpressure,
  and WS fan-out metrics across the rest of the critical path.
- Production K8s, HPA, pod disruption budgets, rollout manifests, GitOps wiring, and
  release automation are not present as executable deployment artifacts.

## 4. Capacity Levels

| Level | Event class | Target scale | Claim allowed? | Required gate |
| --- | --- | ---: | --- | --- |
| L0 | Local and internal development | Developer traffic only | Yes | Existing local compose and service tests |
| L1 | Small public burst | 1k concurrent users | No, until gate passes | Single-region staging load test with core hot paths instrumented |
| L2 | Major campaign or regional event | 10k concurrent users | No, until gate passes | Horizontally scaled services, production WS path, and dependency-aware readiness |
| L3 | National event | 100k concurrent users | No, until gate passes | Multi-node hot-market routing, proven fan-out, HA data layer, and chaos drills |
| L4 | Exceptional global spike | Above 100k concurrent users | Not in current scope | Requires separate architecture review and pre-event capacity reservation |

The target scale is a planning label. The passing threshold is the full workload profile
and SLO evidence, not the raw concurrent-user number.

## 5. Level Gates

### L1: 1k Concurrent Users

Required before claiming L1:

| Area | Gate |
| --- | --- |
| Matching engine | HTTP and gRPC must share one service state instance or have explicitly separated read/write roles. |
| Matching engine | Per-market command queue depth must be exported as a metric. |
| Matching engine | Order placement, cancellation, and book snapshot routes must emit P50/P95/P99 latency metrics. |
| Wallet ledger | PostgreSQL pool size must be configurable by environment, not fixed in code. |
| Wallet ledger | DB transaction latency and advisory-lock wait time must be measured. |
| WebSocket | One real WS delivery path must support market book/trade streams with reconnect and sequence-gap detection. |
| Readiness | Service readiness must fail when required PostgreSQL, Redis, Kafka, or downstream dependencies are unavailable. |
| Load test | Staging must run a 30-minute steady-state test plus a 5-minute spike test. |
| Evidence | A report must include workload profile, P99/P999 latency, error rate, queue depth, DB pool saturation, and Kafka lag. |

Minimum L1 SLOs:

| SLI | Threshold |
| --- | ---: |
| API 5xx rate | < 0.1% over steady-state window |
| Order submit end-to-end P99 | < 30ms when the matching path is in scope |
| Matching engine internal P99 | < 5ms |
| Wallet ledger post transaction P99 | < 50ms |
| WebSocket message delivery P99 | < 30ms for subscribed market streams |
| Kafka producer error rate | 0 acknowledged data-loss events |

### L2: 10k Concurrent Users

Required before claiming L2:

| Area | Gate |
| --- | --- |
| Matching engine | Hot markets must be sharded by routing key. The routing rule must be deterministic and observable. |
| Matching engine | A hot-market owner failover or read-only halt procedure must be documented and exercised. |
| Matching engine | Kafka projection must be wired into the production event path with `acks=all` and idempotent producer settings. |
| API gateway | Rate limiting, request shedding, and backpressure responses must be implemented for order and snapshot endpoints. |
| Wallet ledger | Ledger writes must be profiled under realistic contention for the top accounts and settlement flows. |
| WebSocket | WS gateway must run as a horizontally scaled service with external pub/sub or partitioned fan-out ownership. |
| Data layer | PostgreSQL connection budget, pool sizing, indexes, and slow-query thresholds must be documented per service. |
| Deployment | K8s or equivalent manifests must include readiness/liveness/startup probes, HPA, resource requests, and PDBs. |
| Observability | Dashboards must show per-route latency, error ratio, queue depth, Kafka lag, DB locks, pool saturation, CPU, and memory. |
| Operations | On-call runbooks must cover engine degradation, Kafka lag, DB contention, WS reconnect storm, and emergency halt. |
| Load test | Staging must run 60 minutes steady state, 15 minutes spike, and reconnect-storm simulation. |

Minimum L2 SLOs:

| SLI | Threshold |
| --- | ---: |
| API 5xx rate | < 0.1% over steady-state window |
| Order submit end-to-end P99 | < 30ms |
| Order submit end-to-end P999 | < 100ms |
| Matching engine internal P99 | < 5ms |
| Kafka consumer/producer lag for critical topics | Within the SPEC-008/SPEC-007 alert thresholds |
| WS delivery success | > 99.9% excluding client network disconnects |
| Recovery from one non-data-node pod loss | No data loss and no manual database repair |

### L3: 100k Concurrent Users

Required before claiming L3:

| Area | Gate |
| --- | --- |
| Traffic protection | CDN/WAF/bot/rate-limit rules must be configured and load-tested with abuse traffic mixed into normal traffic. |
| Matching engine | Hot/cold market topology must support moving hot markets to dedicated nodes without state loss. |
| Matching engine | The routing plane must be highly available and must expose stale-route detection. |
| Event pipeline | Kafka topic partitions, replication factor, retention, ISR health, producer acks, and consumer lag must be verified under load. |
| WebSocket | Fan-out must be partitioned and tested for reconnect storms, slow consumers, sequence gaps, and regional disconnects. |
| PostgreSQL | Primary, replica, backup, failover, and connection-pooler behavior must be tested under write pressure. |
| Disaster recovery | Region failover drill must prove RTO/RPO targets using PostgreSQL and Kafka MirrorMaker evidence. |
| Observability | P99/P999 latency, saturation, queue depth, lag, lock waits, and error budget burn must be queryable for every critical service. |
| Load test | Full dress rehearsal must run with production-like data volume, hot-market skew, reconnect storms, and dependency fault injection. |
| Operations | Event-day command center, freeze window, rollback path, emergency halt, and vendor escalation contacts must be documented. |

Minimum L3 SLOs:

| SLI | Threshold |
| --- | ---: |
| Critical order path correctness | 0 lost accepted orders |
| Ledger invariant violations | 0 |
| API 5xx rate | < 0.1% outside deliberate load shedding |
| Order submit end-to-end P99 | < 30ms for admitted traffic |
| Order submit end-to-end P999 | < 150ms for admitted traffic |
| WS sequence-gap recovery | > 99.9% successful automatic recovery |
| Kafka critical lag | Within alert threshold, with no unbounded growth during steady state |
| DR drill | Meets documented RTO/RPO before event-day approval |

## 6. Required Evidence Artifact

Every capacity test must produce a report under `docs/capacity-reports/` using this
filename pattern:

```text
YYYY-MM-DD-L<level>-<scenario>-capacity-report.md
```

The report must include:

- Git commit SHA and deployed artifact versions.
- Test environment topology, instance sizes, resource limits, and region.
- Workload profile table from section 2.
- Dataset size, number of markets, hot-market skew, order book depth distribution,
  and number of funded users.
- Test phases: warm-up, steady state, spike, reconnect storm, and recovery.
- P50/P95/P99/P999 latency for each critical route and background consumer.
- Error rates by endpoint and error class.
- Matching command queue depth and event broadcast lag.
- Kafka producer error rate, consumer lag, ISR status, and topic partition count.
- PostgreSQL CPU, IO, lock waits, slow queries, connection pool saturation, and
  transaction latency.
- Redis CPU, memory, command latency, and eviction count when Redis is on the path.
- WebSocket open connections, messages/sec, slow consumers, dropped connections,
  reconnect rate, and sequence-gap recovery rate.
- Screenshots or exported JSON from Grafana/Prometheus for the test window.
- Explicit pass/fail against the target level.

## 7. Release Policy

The platform must not be marketed or approved for an event class above the latest
passing capacity report.

| Latest passing report | Allowed public claim |
| --- | --- |
| No L1 report | Internal development only |
| L1 | Small public burst only |
| L2 | Major regional event only |
| L3 | National event only |

If a code or infrastructure change touches matching, ledger, API gateway, WS gateway,
Kafka, PostgreSQL, Redis, readiness, deployment, or observability, the previous capacity
report is stale unless the change is proven not to affect that path.

## 8. Immediate Implementation Backlog

The next work should land in this order:

1. Share one matching-engine service instance across HTTP and gRPC, or split the
   surfaces into explicit read/write roles.
2. Make wallet-ledger PostgreSQL pool sizing configurable by environment.
3. Replace static readiness with dependency-aware readiness checks.
4. Extend the new matching-engine route latency / command latency / queue depth
   metrics across DB pool saturation, advisory-lock wait, Kafka lag, and WS fan-out.
5. Wire Kafka projection into the matching-engine production event path.
6. Implement a production WS gateway fan-out path with backpressure and sequence-gap
   recovery.
7. Add executable deployment manifests with probes, HPA, resource requests, and PDBs.
8. Run the repeatable matching-engine hot-market load harness plus
   `tools/scripts/load/matching-engine-production-evidence.mjs` in staging/prod-like
   infrastructure and attach the generated `docs/capacity-reports/` report.

## 9. References

- `services/matching-engine/src/main.rs`
- `services/matching-engine/src/api.rs`
- `services/matching-engine/src/service.rs`
- `services/matching-engine/src/projection.rs`
- `services/wallet-ledger-service/src/main.rs`
- `services/wallet-ledger-service/src/ledger/mod.rs`
- `services/ws-gateway/src/main.rs`
- `crates/sdk/src/service_stub.rs`
- `crates/sdk/src/observability.rs`
- `infra/docker-compose.local.yaml`
- `infra/docker-compose.kafka-mm2.yaml`
- `infra/prometheus/rules/latency-budget.yml`
- `specs/SPEC-003-matching-engine.html`
- `specs/SPEC-007-observability.html`
- `specs/SPEC-008-disaster-recovery.html`
- `specs/SPEC-013-cicd-release.html`
- `specs/SPEC-014-tech-architecture.html`
