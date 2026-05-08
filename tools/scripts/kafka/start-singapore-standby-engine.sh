#!/usr/bin/env bash
set -euo pipefail

: "${SINGAPORE_KAFKA_BOOTSTRAP_SERVERS:?SINGAPORE_KAFKA_BOOTSTRAP_SERVERS is required}"
: "${ENGINE_REPLAY_TOPICS:=engine.events.*,ledger.txns.*,settlement.*}"
: "${MATCHING_ENGINE_BIN:=matching-engine}"

export KAFKA_BOOTSTRAP_SERVERS="${SINGAPORE_KAFKA_BOOTSTRAP_SERVERS}"
export ENGINE_REPLAY_TOPICS
export PROPH_REGION="singapore"
export PROPH_ENGINE_STARTUP_MODE="dr-standby-replay"

exec "${MATCHING_ENGINE_BIN}"
