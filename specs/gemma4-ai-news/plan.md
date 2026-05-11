# 計畫：AI 每日新聞洞察

## 技術棧

- Rust 2021：CLI、RSS/Atom 收集、llama-cli 啟動、ntfy 發送。
- `reqwest` blocking client：抓取 feeds 與發送 ntfy。
- `feed-rs`：解析 RSS/Atom。
- `serde`/`serde_json`：產生 React dashboard JSON。
- React + Vite：瀏覽最新洞察。

## 架構

1. `cargo run -- news` 讀取新聞設定與 llama-cli 設定。
2. 收集最近 `AI_NEWS_DAYS` 天、最多 `AI_NEWS_MAX_ITEMS` 則新聞。
3. 將新聞標題、來源、日期、摘要組成繁體中文分析 prompt。
4. 呼叫 `llama-cli --jinja --single-turn` 搭配 Gemma 4 E4B Q4_K_M GGUF 產生洞察。
5. 輸出 Markdown 與 JSON。
6. 使用 `--send` 時 POST 到 ntfy topic。
7. React dashboard 讀取 JSON 顯示摘要與來源。

## 排程

`./scripts/install_daily_cron.sh` 安裝每日 cron，預設每天 08:15 執行 `./scripts/run_daily_news.sh`。
