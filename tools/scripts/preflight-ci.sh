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
run node --test tools/scripts/*.test.mjs
run cargo check --workspace --locked
run cargo clippy --workspace --all-targets --locked -- -D warnings
run cargo fmt --all -- --check
