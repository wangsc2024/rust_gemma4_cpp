#!/usr/bin/env bash
set -euo pipefail

cargo run -- \
  --model "${GEMMA4_GGUF:-models/gemma4-e4b-it-Q4_K_M.gguf}" \
  --prompt "${PROMPT:-請用繁體中文用三點說明 Gemma 4 E4B Q4_K_M GGUF 的本機推論用途。}" \
  "${@}"
