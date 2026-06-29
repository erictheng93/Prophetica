#!/usr/bin/env bash
set -euo pipefail

export SQLX_OFFLINE=true

if [ "${PREFLIGHT_USE_CARGO_CONFIG_WRAPPER:-0}" != "1" ]; then
    export RUSTC_WRAPPER=
fi

run() {
    printf '\n==> %s\n' "$*"
    "$@"
}

run node --test tools/scripts/validate-landing-variants.test.mjs
run node tools/scripts/validate-landing-variants.mjs
find tools/scripts -name '*.test.mjs' -print0 | xargs -0 node --test
run bash -n tools/scripts/smoke/matching-engine-compose-smoke.sh
if command -v shellcheck >/dev/null 2>&1; then
    run shellcheck tools/scripts/smoke/matching-engine-compose-smoke.sh
fi
run node --check tools/scripts/load/matching-engine-hot-market-load.mjs
run node --check tools/scripts/load/matching-engine-production-evidence.mjs
run ruby -e 'require "yaml"; YAML.load_stream(File.read("infra/k8s/matching-engine-hot-market.yaml")); YAML.load_stream(File.read("infra/prometheus/rules/latency-budget.yml")); puts "yaml ok"'
if command -v promtool >/dev/null 2>&1; then
    run promtool check rules infra/prometheus/rules/latency-budget.yml
fi
run cargo check --workspace --locked
run cargo clippy --workspace --all-targets --locked -- -D warnings
run cargo fmt --all -- --check
