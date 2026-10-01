#!/usr/bin/env bash
# Capture -> assemble -> evaluate, in one command.
#
#   ./scripts/run-eval.sh                          # all platforms, heuristic scoring
#   ./scripts/run-eval.sh --judge llm              # score with the local model
#   ./scripts/run-eval.sh --platform claude_code   # one platform only
#   ./scripts/run-eval.sh --no-inline --force      # test real skill discovery
#
# Captures already on disk are reused; pass --force to re-run them.
set -euo pipefail

cd "$(dirname "$0")/.."

CAPTURE_ARGS=()
JUDGE="heuristic"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --judge)    JUDGE="$2"; shift 2 ;;
    --platform) CAPTURE_ARGS+=(--platform "$2"); shift 2 ;;
    --case)     CAPTURE_ARGS+=(--case "$2"); shift 2 ;;
    --prompting) CAPTURE_ARGS+=(--prompting "$2"); shift 2 ;;
    --force)     CAPTURE_ARGS+=(--force); shift ;;
    --no-inline) CAPTURE_ARGS+=(--no-inline); shift ;;
    *) echo "unknown option: $1" >&2; exit 64 ;;
  esac
done

echo "==> 1/3  Capturing responses"
python3 scripts/capture.py "${CAPTURE_ARGS[@]+"${CAPTURE_ARGS[@]}"}"

echo
echo "==> 2/3  Assembling traces"
python3 scripts/build-traces.py

echo
echo "==> 3/3  Evaluating (judge: $JUDGE)"
cargo run --quiet -- samples/captured.json --judge "$JUDGE"
