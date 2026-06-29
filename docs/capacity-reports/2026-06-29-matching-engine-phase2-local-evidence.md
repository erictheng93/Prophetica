# Matching Engine Phase 2 Local Evidence

Date: 2026-06-29

Scope: `matching-engine` / infra observability local proof for Kafka production event path, dependency-aware readiness, and compose smoke readiness.

## Verified Locally

| Requirement | Evidence |
| --- | --- |
| Kafka event path is available to the engine service | `InMemoryEngineService::with_projection_sink` publishes projected engine events to a configured `ProjectionSink`; regression test `engine_service_publishes_projection_events_to_configured_sink` passes. |
| Production startup can wire Kafka projection | `MATCHING_ENGINE_KAFKA_BROKERS` configures `KafkaProjectionSink` in `matching_engine::api::run*`; `matching-engine-hot` and `matching-engine-cold` expose a `kafka` feature that passes through to `matching-engine/kafka`. |
| Kafka producer durability settings remain enforced | Existing projection tests cover `acks=all`, idempotence, and topic replication policy; `cargo check -p matching-engine --features kafka --test projection --locked` passes. |
| HTTP and gRPC state divergence is removed for normal startup | `matching_engine::api::run()` constructs one service and passes clones to HTTP and gRPC runners. |
| Dependency-aware readiness exists for Kafka | `sdk::service_stub::router_with_readiness` supports TCP dependency checks; matching-engine maps `MATCHING_ENGINE_KAFKA_BROKERS` into readiness dependencies. |
| Compose smoke verifies Kafka delivery | `tools/scripts/smoke/matching-engine-compose-smoke.sh` starts compose Kafka, creates local projection topics, launches `matching-engine-hot --features kafka`, checks `/healthz/ready`, places an order, reads the book, and verifies an `engine.order_update` Kafka event for the smoke market. The script passed locally after Docker became available. |
| Matching-engine P99 signals are scrapeable | `/metrics` includes `proph_engine_http_request_duration_seconds`, `proph_engine_command_duration_seconds`, and `proph_engine_command_queue_depth`; regression test `metrics_endpoint_exposes_matching_engine_latency_and_queue_signals` passes. |
| Hot-market P99 evidence can be generated repeatably | `tools/scripts/load/matching-engine-hot-market-load.mjs` generates JSON and markdown capacity reports with P50/P95/P99/P999, error rate, workload profile, and PASS/FAIL decision for both order submission and book reads. |
| Production evidence has a fail-closed collector | `tools/scripts/load/matching-engine-production-evidence.mjs` validates a passing hot-market load report, ready hot-market pods across distinct nodes, and Prometheus snapshots for route P99/P999, command P99, queue depth, Kafka lag, and Kafka producer errors. |
| Hot-market topology baseline exists | `infra/k8s/matching-engine-hot-market.yaml` defines hot-market deployment, service, probes, scrape annotations, dedicated node selector, topology spread, HPA, and PDB. |
| Matching-engine alert rules exist and parse | `infra/prometheus/rules/latency-budget.yml` includes alerts for order route P99, internal command P99, and command queue depth; `promtool check rules` passes in the official Prometheus container. |
| CI/static gates cover the evidence harness | `.github/workflows/ci.yml` now runs nested `tools/scripts/**/*.test.mjs` tests, shell syntax, shellcheck, load-script syntax, YAML parsing, and `promtool check rules` for the matching-engine evidence artifacts. `tools/scripts/preflight-ci.sh` mirrors these checks when the local tools are installed. |

## Commands Run

```bash
cargo check -p matching-engine --test projection --test engine_service --locked
cargo test -p matching-engine --test projection engine_service_publishes_projection_events_to_configured_sink --locked
cargo check -p matching-engine --features kafka --test projection --locked
cargo test -p sdk service_stub --locked
bash -n tools/scripts/smoke/matching-engine-compose-smoke.sh
node --test tools/scripts/smoke/matching-engine-compose-smoke.test.mjs
shellcheck tools/scripts/smoke/matching-engine-compose-smoke.sh
cargo check -p matching-engine-hot --features kafka --locked
cargo check -p matching-engine-cold --features kafka --locked
cargo test -p matching-engine --test engine_api metrics_endpoint_exposes_matching_engine_latency_and_queue_signals --locked
node --test tools/scripts/load/matching-engine-hot-market-load.test.mjs
node --check tools/scripts/load/matching-engine-hot-market-load.mjs
node --test tools/scripts/load/matching-engine-production-evidence.test.mjs
node --check tools/scripts/load/matching-engine-production-evidence.mjs
ruby -e 'require "yaml"; YAML.load_stream(File.read("infra/k8s/matching-engine-hot-market.yaml")); puts "yaml ok"'
find tools/scripts -name '*.test.mjs' -print0 | xargs -0 node --test
ruby -e 'require "yaml"; YAML.load_stream(File.read(".github/workflows/ci.yml")); YAML.load_stream(File.read("infra/k8s/matching-engine-hot-market.yaml")); YAML.load_stream(File.read("infra/prometheus/rules/latency-budget.yml")); puts "yaml ok"'
tools/scripts/smoke/matching-engine-compose-smoke.sh
docker run --rm --entrypoint promtool -v "$PWD":/work -w /work prom/prometheus:v2.55.1 check rules infra/prometheus/rules/latency-budget.yml
```

## Not Verified In This Environment

| Requirement | Status |
| --- | --- |
| Production hot-market topology evidence | Pending. A baseline K8s manifest and fail-closed evidence collector now exist, but the manifest has not been applied to staging/prod-like infrastructure and no real topology capture has been recorded. |
| Production P99/P999 evidence | Pending. A load harness, matching-engine metrics, and Prometheus/topology evidence collector now exist, but no production-like load-test run, Prometheus export, Kafka lag capture, or hot-market skew report was available in this environment. |

## Follow-Up Gate

Before claiming Phase 2 production readiness, deploy/apply the hot-market topology, run `tools/scripts/load/matching-engine-hot-market-load.mjs` against the staging/prod-like endpoint, then run `tools/scripts/load/matching-engine-production-evidence.mjs` with either live `kubectl`/`MATCHING_ENGINE_PROMETHEUS_URL` access or captured topology/Prometheus JSON snapshots. Archive the generated report under `docs/capacity-reports/` with P99/P999 latency, Kafka producer errors, Kafka lag, per-market queue depth, CPU saturation, and hot-market routing/failover evidence.
