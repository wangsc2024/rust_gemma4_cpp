# Rust + llama-cli + Gemma 4 E4B GGUF Q4_K_M 範例

這是一個最小可執行的 Rust 範例，用 Rust binary 組裝並呼叫 `llama-cli`，載入本機的 Gemma 4 E4B `GGUF` `Q4_K_M` 模型檔進行單輪推論。

> 本專案不重新實作推論引擎；Rust 程式負責參數管理、檔案檢查與行程啟動，實際推論由 `llama.cpp` 的 `llama-cli` 執行。

## 專案結構

```text
.
├── Cargo.toml
├── README.md
├── scripts/run_example.sh
├── src/main.rs
├── models/.gitkeep
└── specs/gemma4-llama-cli/
    ├── spec.md
    ├── plan.md
    └── tasks.md
```

## 前置需求

1. 安裝 Rust 1.89+。
2. 安裝或編譯 `llama.cpp`，並確保 `llama-cli` 在 `PATH` 中，或設定 `LLAMA_CLI=/path/to/llama-cli`。
3. 下載 Gemma 4 E4B 的 `Q4_K_M` GGUF 檔，放到：

```text
models/gemma4-e4b-it-Q4_K_M.gguf
```

如果檔名不同，請用 `--model` 或 `GEMMA4_GGUF` 指定實際路徑。

## 快速開始

先用 dry run 檢查實際會執行的 `llama-cli` 指令：

```bash
cargo run -- --dry-run --prompt "請用繁體中文自我介紹。"
```

確認模型檔已放好後執行：

```bash
cargo run -- --prompt "請用繁體中文用三點說明量化模型的優缺點。"
```

也可以使用輔助腳本：

```bash
GEMMA4_GGUF=models/gemma4-e4b-it-Q4_K_M.gguf ./scripts/run_example.sh
```

## 常用參數

| Rust wrapper 參數 | 對應 llama-cli 參數 | 說明 |
| --- | --- | --- |
| `--llama-cli <path>` | 程式路徑 | 指定 `llama-cli`，也可用 `LLAMA_CLI`。 |
| `--model <path>` | `--model` | 指定 Gemma 4 E4B Q4_K_M GGUF 檔，也可用 `GEMMA4_GGUF`。 |
| `--prompt <text>` | `--prompt` | 指定使用者提示，也可用 `PROMPT`。 |
| `--system-prompt <text>` | `--system-prompt` | 指定系統提示，也可用 `SYSTEM_PROMPT`。 |
| `--no-system-prompt` | 無 | 不傳送系統提示。 |
| `--ctx-size <n>` | `--ctx-size` | 指定上下文長度。 |
| `--predict <n>` | `--predict` | 指定輸出 token 數，預設 `256`。 |
| `--temp <n>` | `--temp` | 指定 temperature，預設 `0.7`。 |
| `--top-p <n>` | `--top-p` | 指定 top-p，預設 `0.95`。 |
| `--threads <n>` | `--threads` | 指定 CPU threads。 |
| `--gpu-layers <n>` | `--gpu-layers` | 指定 GPU offload layers。 |
| `--dry-run` | 無 | 只印出指令，不執行模型。 |

額外的 `llama-cli` 參數可放在 `--` 後面，例如啟用 Flash Attention：

```bash
cargo run -- --prompt "你好" -- -fa on
```

## 設計重點

- 使用 `--jinja --single-turn`，優先採用 GGUF 內嵌 chat template 執行單輪對話。
- 預設模型路徑為 `models/gemma4-e4b-it-Q4_K_M.gguf`，避免將大型模型納入 Git。
- 執行前檢查模型檔是否存在；若只是測試指令組裝，可使用 `--dry-run`。
- 沒有 Rust 第三方 crate 依賴，方便在乾淨環境中建置與測試。

## 疑難排解

### 找不到模型

請確認路徑存在，或用以下方式指定：

```bash
GEMMA4_GGUF=/absolute/path/to/model.gguf cargo run -- --prompt "你好"
```

### 找不到 llama-cli

請確認 `llama-cli` 在 `PATH`，或指定：

```bash
LLAMA_CLI=/path/to/llama-cli cargo run -- --prompt "你好"
```

### 只想確認參數

```bash
cargo run -- --dry-run --model /tmp/model.gguf --prompt "測試"
```
