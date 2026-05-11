#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
schedule="${AI_NEWS_CRON:-15 8 * * *}"
job="${schedule} cd ${repo_dir} && GEMMA4_GGUF=\"${GEMMA4_GGUF:-models/gemma4-e4b-it-Q4_K_M.gguf}\" NTFY_TOPIC=\"${NTFY_TOPIC:-https://ntfy.sh/wangsc_ainews}\" ./scripts/run_daily_news.sh >> data/ai-news/cron.log 2>&1"

( crontab -l 2>/dev/null | grep -v 'run_daily_news.sh' || true; printf '%s\n' "${job}" ) | crontab -
printf 'Installed cron job:\n%s\n' "${job}"
