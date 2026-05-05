#!/usr/bin/env bash
set -euo pipefail

if [ ! -f .env ]; then
  cp .env.example .env
  printf 'created .env from .env.example\n'
fi

pnpm install --frozen-lockfile
cargo fetch
buf generate
cargo build --workspace --locked
