# Rust + llama-cli + Gemma 4 E4B GGUF Q4_K_M + React AI 每日新聞洞察

這是一個端到端範例：

1. Rust CLI 每日收集 AI RSS/Atom 新聞。
2. 使用本機 `llama-cli` 載入 Gemma 4 E4B `GGUF` `Q4_K_M` 模型，撰寫繁體中文 AI 每日新聞洞察。
3. 輸出 Markdown 與 React dashboard JSON。
4. 可發送至 `https://ntfy.sh/wangsc_ainews`。

> 本專案不重新實作推論引擎；Rust 程式負責新聞收集、prompt 組裝、檔案輸出、ntfy 發送與啟動 `llama-cli`。

## 專案結構

```text
.
├── Cargo.toml
├── src/main.rs
├── scripts/
│   ├── run_example.sh
│   ├── run_daily_news.sh
│   └── install_daily_cron.sh
├── web/
│   ├── package.json
│   ├── index.html
│   └── src/App.jsx
├── models/.gitkeep
├── data/ai-news/.gitkeep
└── specs/gemma4-ai-news/
```

## 前置需求

1. 安裝 Rust 1.89+。
2. 安裝 Node.js 20+（React dashboard 使用）。
3. 安裝或編譯 `llama.cpp`，並確保 `llama-cli` 在 `PATH` 中，或設定 `LLAMA_CLI=/path/to/llama-cli`。
4. 下載 Gemma 4 E4B 的 `Q4_K_M` GGUF 檔，放到：

```text
models/gemma4-e4b-it-Q4_K_M.gguf
```

如果檔名不同，請用 `--model` 或 `GEMMA4_GGUF` 指定實際路徑。

## 模式一：單次 llama-cli 推論

先用 dry run 檢查實際會執行的 `llama-cli` 指令：

```bash
cargo run -- --dry-run --prompt "請用繁體中文自我介紹。"
```

確認模型檔已放好後執行：

```bash
cargo run -- --prompt "請用繁體中文用三點說明量化模型的優缺點。"
```

額外的 `llama-cli` 參數可放在 `--` 後面，例如啟用 Flash Attention：

```bash
cargo run -- --prompt "你好" -- -fa on
```

## 模式二：每日 AI 新聞洞察

### Dry run，不抓 RSS、不跑模型、不發 ntfy

```bash
cargo run -- news --dry-run
```

此命令會：

- 印出新聞工作流設定。
- 印出即將呼叫的 `llama-cli` 命令。
- 產生示範 Markdown 與 React JSON。

### 正式產生洞察，但不發送 ntfy

```bash
cargo run -- news --no-send --gpu-layers 99
```

### 正式產生洞察並發送 ntfy.sh/wangsc_ainews

```bash
cargo run -- news --send --gpu-layers 99
```

或使用腳本：

```bash
GEMMA4_GGUF=models/gemma4-e4b-it-Q4_K_M.gguf ./scripts/run_daily_news.sh
```

## 每日排程

安裝 cron，每天 08:15 執行（可用 `AI_NEWS_CRON` 覆寫）：

```bash
./scripts/install_daily_cron.sh
```

也可以手動建立 crontab：

```cron
15 8 * * * cd /path/to/repo && GEMMA4_GGUF=models/gemma4-e4b-it-Q4_K_M.gguf ./scripts/run_daily_news.sh >> data/ai-news/cron.log 2>&1
```

## React dashboard

React dashboard 會讀取 `web/public/latest-news.example.json` 並顯示最新洞察、新聞來源與 ntfy 狀態。

```bash
cd web
npm install
npm run dev
```

正式建置：

```bash
cd web
npm run build
```

## 常用參數

| 參數 | 說明 |
| --- | --- |
| `--llama-cli <path>` | 指定 `llama-cli`，也可用 `LLAMA_CLI`。 |
| `--model <path>` | 指定 Gemma 4 E4B Q4_K_M GGUF 檔，也可用 `GEMMA4_GGUF`。 |
| `--prompt <text>` | 單次推論提示，也可用 `PROMPT`。 |
| `--system-prompt <text>` | 指定系統提示，也可用 `SYSTEM_PROMPT`。 |
| `--dry-run` | 單次推論只印命令；新聞模式產生示範資料且不做網路/模型/ntfy 操作。 |
| `--feed <url>` | 新增新聞 RSS/Atom 來源。 |
| `--clear-feeds` | 清除預設來源，搭配自訂 `--feed`。 |
| `--max-items <n>` | 每日最多分析新聞數，預設 `12`。 |
| `--days <n>` | 收集最近幾天新聞，預設 `2`。 |
| `--output-md <path>` | Markdown 輸出，預設 `data/ai-news/latest.md`。 |
| `--output-json <path>` | React JSON 輸出，預設 `web/public/latest-news.example.json`。 |
| `--ntfy-topic <url>` | ntfy topic，預設 `https://ntfy.sh/wangsc_ainews`。 |
| `--send` | 發送到 ntfy。 |

## 預設新聞來源

內建 RSS/Atom 來源包含 OpenAI、Google AI、MIT Technology Review AI 與 VentureBeat AI。可使用 `AI_NEWS_FEEDS` 以逗號分隔覆寫。

## 輸出檔案

- `data/ai-news/latest.md`：人類可讀的 Markdown 洞察。
- `web/public/latest-news.example.json`：React dashboard 使用的 JSON。

## 安全與隱私

- `.gguf` 模型權重不會提交到 Git。
- 預設 `AI_NEWS_SEND=0`，必須明確使用 `--send` 或腳本才會發送 ntfy。
- ntfy topic 是公開 URL；若包含敏感內容請改用私人 topic 或自架 ntfy。
