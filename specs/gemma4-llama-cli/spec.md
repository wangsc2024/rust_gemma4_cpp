# 規格：Rust + llama-cli + Gemma 4 E4B GGUF Q4_K_M 範例

## 目標

建立一個最小 Rust CLI 範例，讓使用者可透過 Rust 程式呼叫 `llama-cli`，載入本機 Gemma 4 E4B `Q4_K_M` GGUF 模型並輸出回覆。

## 使用者價值

- Rust 開發者可快速理解如何從 Rust 啟動 `llama-cli`。
- 使用者可透過環境變數或 CLI 參數切換 `llama-cli` 與模型路徑。
- 沒有模型或 GPU 的環境仍可透過 `--dry-run` 與單元測試驗證程式行為。

## 範圍

### 包含

- Rust binary crate。
- `llama-cli` 參數組裝。
- Gemma 4 E4B Q4_K_M GGUF 預設模型路徑。
- `--dry-run` 模式。
- README、範例腳本與規格文件。

### 不包含

- 下載模型權重。
- 編譯 `llama.cpp`。
- 直接連結 `llama.cpp` C/C++ library。
- Web UI 或長期服務模式。

## 驗收條件

1. 執行 `cargo test` 必須通過。
2. 執行 `cargo run -- --dry-run --prompt "你好"` 必須印出包含 `llama-cli`、`--model`、`--jinja`、`--single-turn` 與 `--prompt` 的命令。
3. 在模型檔不存在且非 dry-run 時，程式必須回報清楚錯誤。
4. README 必須說明如何設定 `LLAMA_CLI` 與 `GEMMA4_GGUF`。
5. `.gitignore` 必須排除 `models/*.gguf`。
