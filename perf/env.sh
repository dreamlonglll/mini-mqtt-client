#!/usr/bin/env bash
# Run one perf python script inside the e2e environment (private broker, Xvfb, AT-SPI).
#   perf/env.sh perf/perf_run.py         # env PERF_EXP=1,2 selects experiments
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$(realpath "$1")"
NAME="$(basename "$SCRIPT" .py)-${PERF_EXP:-all}"
PERF_WORK="${PERF_WORK:-$ROOT/target/perf}"
export PERF_LOADGEN="${PERF_LOADGEN:-$PERF_WORK/loadgen-target/release/mqtt-load}"
if [[ ! -x "$PERF_LOADGEN" ]]; then
  (cd "$ROOT/perf/loadgen" && CARGO_TARGET_DIR="$PERF_WORK/loadgen-target" cargo build --release)
fi
export E2E_SCRIPT="$SCRIPT"
export E2E_BIN="${E2E_BIN:-$ROOT/target/release/mini-mqtt-client}"
export E2E_WORK_DIR="$PERF_WORK/env-${NAME//,/_}"
export PERF_OUT="${PERF_OUT:-$PERF_WORK/results}"
export E2E_TCP_PORT="${E2E_TCP_PORT:-28830}" E2E_WS_PORT="${E2E_WS_PORT:-28083}" E2E_TLS_PORT="${E2E_TLS_PORT:-28883}"
export E2E_DISPLAY="${E2E_DISPLAY:-:96}"
exec "$ROOT/e2e/run.sh"
