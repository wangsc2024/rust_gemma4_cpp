# 計畫：Rust wrapper 呼叫 llama-cli

## 技術棧

- Rust 2021 edition。
- 標準函式庫 `std::process::Command` 啟動 `llama-cli`。
- 不加入第三方 crates，以降低範例複雜度。

## 架構

1. `src/main.rs` 定義 `AppConfig` 保存所有 wrapper 設定。
2. CLI 參數與環境變數合併後產生 `AppConfig`。
3. `build_llama_args` 將設定轉換成 `llama-cli` 參數。
4. `validate` 在實際執行前檢查 prompt 與模型檔。
5. `--dry-run` 只列印命令，不檢查模型檔、不啟動行程。

## llama-cli 呼叫策略

預設傳入：

```text
--model <gguf> --jinja --single-turn --prompt <prompt> --system-prompt <system>
```

理由：使用 GGUF 內嵌 chat template，適合現代 instruct/chat 模型的單輪範例。

## 測試策略

- 單元測試參數解析。
- 單元測試 `llama-cli` 參數組裝。
- 單元測試空 prompt 驗證。
- 手動 dry-run 驗證 README 範例。
