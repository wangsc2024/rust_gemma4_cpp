#!/usr/bin/env bash
set -euo pipefail

cargo run -- news \
  --resume \
  --model "${GEMMA4_GGUF:-models/gemma4-e4b-it-Q4_K_M.gguf}" \
  --ntfy-topic "${NTFY_TOPIC:-https://ntfy.sh/wangsc_ainews}" \
  --send \
  "${@}"
