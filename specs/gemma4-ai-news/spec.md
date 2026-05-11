# 規格：Rust + llama-cli + Gemma 4 E4B + React AI 每日新聞洞察

## 目標

建立一個每日 AI 新聞洞察系統，使用 Rust 收集新聞、呼叫本機 `llama-cli` + Gemma 4 E4B Q4_K_M GGUF 產出繁體中文洞察，輸出 React dashboard JSON，並可發送至 `https://ntfy.sh/wangsc_ainews`。

## 使用者價值

- 每日自動取得 AI 產業重點新聞。
- 用本機模型生成繁體中文分析，避免依賴雲端 LLM API。
- 透過 ntfy 取得推播，並透過 React dashboard 回顧摘要與來源。

## 範圍

### 包含

- Rust CLI `news` 子命令。
- RSS/Atom 新聞收集。
- Gemma 4 E4B Q4_K_M GGUF prompt 產生與 `llama-cli` 呼叫。
- Markdown 與 React JSON 輸出。
- ntfy.sh 發送。
- React dashboard。
- cron 安裝腳本。
- 斷點檔與 resume/force/test-send 工作流閉環。

### 不包含

- 模型權重下載。
- llama.cpp 編譯。
- 使用者帳號系統。
- 資料庫儲存歷史。

## 驗收條件

1. `cargo test` 必須通過。
2. `cargo run -- news --dry-run` 必須產生示範 Markdown 與 JSON，且不得抓取 RSS、不得執行模型、不得發送 ntfy。
3. `cargo run -- news --send` 在模型與網路可用時，必須收集新聞、生成洞察並發送到 `https://ntfy.sh/wangsc_ainews`。
4. React dashboard 必須能讀取 `web/public/latest-news.example.json`。
5. README 必須說明每日排程、ntfy topic、模型設定與 React 使用方式。
6. `cargo run -- news --test-send` 必須能不依賴模型發送一次示範新聞以確認 ntfy 功能。
7. `--resume` 必須能讀取斷點並避免重複發送已送出的新聞。
