#!/usr/bin/env bash
set -euo pipefail

check() {
  local name="$1"
  local command="$2"
  if eval "$command" >/dev/null 2>&1; then
    printf '[ok] %s\n' "$name"
  else
    printf '[missing] %s\n' "$name"
    return 1
  fi
}

status=0
check "rust 1.85" "rustc --version | grep -E '1\\.85\\.'" || status=1
check "cargo" "cargo --version" || status=1
check "node 20+" "node --version | grep -E '^v2[0-9]\\.'" || status=1
check "pnpm 9+" "pnpm --version | grep -E '^9\\.'" || status=1
check "docker" "docker --version" || status=1
check "docker compose v2" "docker compose version" || status=1
check "buf" "buf --version" || status=1
check "sqlx-cli" "cargo sqlx --version" || status=1
check "sccache" "sccache --version" || status=1

if [ -z "${KUMO_NPM_TOKEN:-}" ]; then
  printf '[deferred] KUMO_NPM_TOKEN is not set; required for Cloudflare Kumo registry access in CI\n'
fi

exit "$status"
