use chrono::{DateTime, Duration, Utc};
use feed_rs::model::Entry;
use serde::Serialize;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const DEFAULT_MODEL: &str = "models/gemma4-e4b-it-Q4_K_M.gguf";
const DEFAULT_PROMPT: &str = "請用繁體中文用三點說明 Gemma 4 E4B Q4_K_M GGUF 適合本機推論的原因。";
const DEFAULT_SYSTEM_PROMPT: &str = "你是精準、簡潔且使用繁體中文回答的助理。";
const DEFAULT_NTFY_TOPIC: &str = "https://ntfy.sh/wangsc_ainews";
const DEFAULT_NEWS_OUTPUT_MD: &str = "data/ai-news/latest.md";
const DEFAULT_NEWS_OUTPUT_JSON: &str = "web/public/latest-news.example.json";
const DEFAULT_NEWS_DAYS: i64 = 2;
const DEFAULT_NEWS_MAX_ITEMS: usize = 12;
const DEFAULT_FEEDS: &[&str] = &[
    "https://openai.com/news/rss.xml",
    "https://blog.google/technology/ai/rss/",
    "https://www.technologyreview.com/topic/artificial-intelligence/feed/",
    "https://venturebeat.com/category/ai/feed/",
];

#[derive(Debug, Clone, PartialEq)]
struct AppConfig {
    llama_cli: PathBuf,
    model: PathBuf,
    prompt: String,
    system_prompt: Option<String>,
    ctx_size: Option<u32>,
    predict: Option<i32>,
    temp: Option<f32>,
    top_p: Option<f32>,
    threads: Option<u32>,
    gpu_layers: Option<i32>,
    dry_run: bool,
    allow_missing_model: bool,
    extra_args: Vec<OsString>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llama_cli: env::var_os("LLAMA_CLI")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("llama-cli")),
            model: env::var_os("GEMMA4_GGUF")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_MODEL)),
            prompt: env::var("PROMPT").unwrap_or_else(|_| DEFAULT_PROMPT.to_owned()),
            system_prompt: env::var("SYSTEM_PROMPT")
                .ok()
                .or_else(|| Some(DEFAULT_SYSTEM_PROMPT.to_owned())),
            ctx_size: env_u32("CTX_SIZE"),
            predict: env_i32("N_PREDICT").or(Some(256)),
            temp: env_f32("TEMP").or(Some(0.7)),
            top_p: env_f32("TOP_P").or(Some(0.95)),
            threads: env_u32("THREADS"),
            gpu_layers: env_i32("GPU_LAYERS"),
            dry_run: env_bool("DRY_RUN"),
            allow_missing_model: env_bool("ALLOW_MISSING_MODEL"),
            extra_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct NewsConfig {
    llama: AppConfig,
    feeds: Vec<String>,
    max_items: usize,
    days: i64,
    output_md: PathBuf,
    output_json: PathBuf,
    ntfy_topic: String,
    send: bool,
    title: String,
}

impl Default for NewsConfig {
    fn default() -> Self {
        let llama = AppConfig {
            prompt: String::new(),
            predict: env_i32("NEWS_N_PREDICT").or(Some(900)),
            temp: env_f32("NEWS_TEMP").or(Some(0.35)),
            top_p: env_f32("NEWS_TOP_P").or(Some(0.9)),
            ..Default::default()
        };

        Self {
            llama,
            feeds: env_list("AI_NEWS_FEEDS").unwrap_or_else(|| {
                DEFAULT_FEEDS
                    .iter()
                    .map(|feed| (*feed).to_owned())
                    .collect()
            }),
            max_items: env_usize("AI_NEWS_MAX_ITEMS").unwrap_or(DEFAULT_NEWS_MAX_ITEMS),
            days: env_i64("AI_NEWS_DAYS").unwrap_or(DEFAULT_NEWS_DAYS),
            output_md: env::var_os("AI_NEWS_OUTPUT_MD")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_NEWS_OUTPUT_MD)),
            output_json: env::var_os("AI_NEWS_OUTPUT_JSON")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_NEWS_OUTPUT_JSON)),
            ntfy_topic: env::var("NTFY_TOPIC").unwrap_or_else(|_| DEFAULT_NTFY_TOPIC.to_owned()),
            send: env_bool("AI_NEWS_SEND"),
            title: env::var("AI_NEWS_TITLE").unwrap_or_else(|_| "AI 每日新聞洞察".to_owned()),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct NewsItem {
    title: String,
    url: String,
    source: String,
    published: Option<String>,
    summary: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct DigestDocument {
    title: String,
    generated_at: String,
    ntfy_topic: String,
    sent: bool,
    items: Vec<NewsItem>,
    markdown: String,
}

#[derive(Debug, Clone, PartialEq)]
enum CliError {
    HelpRequested,
    MissingValue(String),
    InvalidValue {
        flag: String,
        value: String,
        expected: String,
    },
    UnknownFlag(String),
    EmptyPrompt,
    MissingModel(PathBuf),
    CommandFailed(Option<i32>),
    SpawnFailed(String),
    Http(String),
    Feed(String),
    Io(String),
    Json(String),
    NoNews,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpRequested => write!(f, "help requested"),
            Self::MissingValue(flag) => write!(f, "missing value for {flag}"),
            Self::InvalidValue { flag, value, expected } => {
                write!(f, "invalid value for {flag}: {value:?}; expected {expected}")
            }
            Self::UnknownFlag(flag) => write!(f, "unknown flag: {flag}"),
            Self::EmptyPrompt => write!(f, "prompt must not be empty"),
            Self::MissingModel(path) => write!(
                f,
                "GGUF model not found at {}. Put the Gemma 4 E4B Q4_K_M .gguf there, set GEMMA4_GGUF, or pass --model.",
                path.display()
            ),
            Self::CommandFailed(code) => write!(f, "llama-cli exited with status {code:?}"),
            Self::SpawnFailed(msg) => write!(f, "failed to start llama-cli: {msg}"),
            Self::Http(msg) => write!(f, "http error: {msg}"),
            Self::Feed(msg) => write!(f, "feed parse error: {msg}"),
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::Json(msg) => write!(f, "json error: {msg}"),
            Self::NoNews => write!(f, "no AI news items found in the configured feeds"),
        }
    }
}

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(code) => ExitCode::from(code),
        Err(CliError::HelpRequested) => {
            print_help();
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!("run with --help for usage");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<OsString>) -> Result<u8, CliError> {
    if matches!(args.first().and_then(|arg| arg.to_str()), Some("news")) {
        return run_news(parse_news_args(args.into_iter().skip(1).collect())?);
    }

    run_infer(parse_args(args)?)
}

fn run_infer(config: AppConfig) -> Result<u8, CliError> {
    validate(&config)?;
    let llama_args = build_llama_args(&config);

    if config.dry_run {
        println!("{}", shell_join(&config.llama_cli, &llama_args));
        return Ok(0);
    }

    let status = Command::new(&config.llama_cli)
        .args(&llama_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| CliError::SpawnFailed(err.to_string()))?;

    if status.success() {
        Ok(status.code().unwrap_or(0) as u8)
    } else {
        Err(CliError::CommandFailed(status.code()))
    }
}

fn run_news(config: NewsConfig) -> Result<u8, CliError> {
    if config.llama.dry_run {
        print_news_dry_run(&config);
        write_digest_files(&dry_run_digest(&config), &config)?;
        return Ok(0);
    }

    validate_news(&config)?;
    let items = collect_news(&config)?;
    let prompt = build_digest_prompt(&items, Utc::now());
    let insight = generate_llama_text(&config.llama, prompt)?;
    let markdown = render_digest_markdown(&config.title, Utc::now(), &items, &insight);
    let mut document = DigestDocument {
        title: config.title.clone(),
        generated_at: Utc::now().to_rfc3339(),
        ntfy_topic: config.ntfy_topic.clone(),
        sent: false,
        items,
        markdown,
    };

    if config.send {
        publish_ntfy(&config.ntfy_topic, &config.title, &document.markdown)?;
        document.sent = true;
    }

    write_digest_files(&document, &config)?;
    println!("wrote {}", config.output_md.display());
    if document.sent {
        println!("sent digest to {}", config.ntfy_topic);
    } else {
        println!("dry notification: pass --send or AI_NEWS_SEND=1 to publish to ntfy");
    }

    Ok(0)
}

fn parse_args(args: Vec<OsString>) -> Result<AppConfig, CliError> {
    let mut config = AppConfig::default();
    parse_infer_flags(args, &mut config)?;
    Ok(config)
}

fn parse_news_args(args: Vec<OsString>) -> Result<NewsConfig, CliError> {
    let mut config = NewsConfig::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        let arg_text = arg.to_string_lossy();
        if arg_text == "--" {
            config.llama.extra_args.extend(iter);
            break;
        }

        match arg_text.as_ref() {
            "-h" | "--help" => return Err(CliError::HelpRequested),
            "--feed" => config.feeds.push(next_string("--feed", &mut iter)?),
            "--clear-feeds" => config.feeds.clear(),
            "--max-items" => {
                config.max_items = parse_next("--max-items", "a positive integer", &mut iter)?;
            }
            "--days" => config.days = parse_next("--days", "a positive integer", &mut iter)?,
            "--output-md" => {
                config.output_md = PathBuf::from(next_value("--output-md", &mut iter)?)
            }
            "--output-json" => {
                config.output_json = PathBuf::from(next_value("--output-json", &mut iter)?);
            }
            "--ntfy-topic" => config.ntfy_topic = next_string("--ntfy-topic", &mut iter)?,
            "--send" => config.send = true,
            "--no-send" => config.send = false,
            "--title" => config.title = next_string("--title", &mut iter)?,
            flag => parse_infer_flag(flag, &mut iter, &mut config.llama)?,
        }
    }

    Ok(config)
}

fn parse_infer_flags(args: Vec<OsString>, config: &mut AppConfig) -> Result<(), CliError> {
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        let arg_text = arg.to_string_lossy();
        if arg_text == "--" {
            config.extra_args.extend(iter);
            break;
        }

        parse_infer_flag(&arg_text, &mut iter, config)?;
    }

    Ok(())
}

fn parse_infer_flag(
    flag: &str,
    iter: &mut impl Iterator<Item = OsString>,
    config: &mut AppConfig,
) -> Result<(), CliError> {
    match flag {
        "-h" | "--help" => Err(CliError::HelpRequested),
        "--dry-run" => {
            config.dry_run = true;
            Ok(())
        }
        "--allow-missing-model" => {
            config.allow_missing_model = true;
            Ok(())
        }
        "--no-system-prompt" => {
            config.system_prompt = None;
            Ok(())
        }
        "--llama-cli" => {
            config.llama_cli = PathBuf::from(next_value("--llama-cli", iter)?);
            Ok(())
        }
        "-m" | "--model" => {
            config.model = PathBuf::from(next_value("--model", iter)?);
            Ok(())
        }
        "-p" | "--prompt" => {
            config.prompt = next_string("--prompt", iter)?;
            Ok(())
        }
        "--system-prompt" | "-sys" => {
            config.system_prompt = Some(next_string("--system-prompt", iter)?);
            Ok(())
        }
        "-c" | "--ctx-size" => {
            config.ctx_size = Some(parse_next("--ctx-size", "a positive integer", iter)?);
            Ok(())
        }
        "-n" | "--predict" | "--n-predict" => {
            config.predict = Some(parse_next("--predict", "an integer token count", iter)?);
            Ok(())
        }
        "--temp" => {
            config.temp = Some(parse_next("--temp", "a floating point number", iter)?);
            Ok(())
        }
        "--top-p" => {
            config.top_p = Some(parse_next("--top-p", "a floating point number", iter)?);
            Ok(())
        }
        "-t" | "--threads" => {
            config.threads = Some(parse_next("--threads", "a positive integer", iter)?);
            Ok(())
        }
        "-ngl" | "--gpu-layers" => {
            config.gpu_layers = Some(parse_next("--gpu-layers", "an integer", iter)?);
            Ok(())
        }
        value if value.starts_with('-') => Err(CliError::UnknownFlag(value.to_owned())),
        value => {
            config.extra_args.push(OsString::from(value));
            Ok(())
        }
    }
}

fn build_llama_args(config: &AppConfig) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--model"),
        config.model.as_os_str().to_owned(),
        OsString::from("--jinja"),
        OsString::from("--single-turn"),
        OsString::from("--prompt"),
        OsString::from(&config.prompt),
    ];

    if let Some(system_prompt) = &config.system_prompt {
        args.push(OsString::from("--system-prompt"));
        args.push(OsString::from(system_prompt));
    }

    push_optional(&mut args, "--ctx-size", config.ctx_size);
    push_optional(&mut args, "--predict", config.predict);
    push_optional(&mut args, "--temp", config.temp);
    push_optional(&mut args, "--top-p", config.top_p);
    push_optional(&mut args, "--threads", config.threads);
    push_optional(&mut args, "--gpu-layers", config.gpu_layers);
    args.extend(config.extra_args.iter().cloned());
    args
}

fn validate(config: &AppConfig) -> Result<(), CliError> {
    if config.prompt.trim().is_empty() {
        return Err(CliError::EmptyPrompt);
    }

    validate_model(config)
}

fn validate_model(config: &AppConfig) -> Result<(), CliError> {
    if !config.dry_run && !config.allow_missing_model && !config.model.is_file() {
        return Err(CliError::MissingModel(config.model.clone()));
    }

    Ok(())
}

fn validate_news(config: &NewsConfig) -> Result<(), CliError> {
    validate_model(&config.llama)?;
    if config.feeds.is_empty() {
        return Err(CliError::NoNews);
    }
    if config.max_items == 0 {
        return Err(CliError::InvalidValue {
            flag: "--max-items".to_owned(),
            value: "0".to_owned(),
            expected: "a positive integer".to_owned(),
        });
    }
    if config.days <= 0 {
        return Err(CliError::InvalidValue {
            flag: "--days".to_owned(),
            value: config.days.to_string(),
            expected: "a positive integer".to_owned(),
        });
    }
    Ok(())
}

fn collect_news(config: &NewsConfig) -> Result<Vec<NewsItem>, CliError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("rust-gemma4-ai-news/0.2")
        .build()
        .map_err(|err| CliError::Http(err.to_string()))?;
    let cutoff = Utc::now() - Duration::days(config.days);
    let mut items = Vec::new();

    for feed_url in &config.feeds {
        let body = client
            .get(feed_url)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|err| CliError::Http(format!("{feed_url}: {err}")))?
            .bytes()
            .map_err(|err| CliError::Http(format!("{feed_url}: {err}")))?;
        let feed = feed_rs::parser::parse(Cursor::new(body))
            .map_err(|err| CliError::Feed(format!("{feed_url}: {err}")))?;
        let source = feed
            .title
            .map(|title| title.content)
            .unwrap_or_else(|| feed_url.to_owned());

        items.extend(
            feed.entries
                .iter()
                .filter_map(|entry| news_item_from_entry(entry, &source, cutoff)),
        );
    }

    items.sort_by(|a, b| {
        b.published
            .cmp(&a.published)
            .then_with(|| a.title.cmp(&b.title))
    });
    items.dedup_by(|a, b| a.url == b.url || a.title == b.title);
    items.truncate(config.max_items);

    if items.is_empty() {
        Err(CliError::NoNews)
    } else {
        Ok(items)
    }
}

fn news_item_from_entry(entry: &Entry, source: &str, cutoff: DateTime<Utc>) -> Option<NewsItem> {
    let title = entry.title.as_ref()?.content.trim().to_owned();
    let url = entry
        .links
        .first()
        .map(|link| link.href.clone())
        .unwrap_or_default();
    let published_at = entry.published.or(entry.updated);

    if let Some(published) = published_at {
        if published < cutoff {
            return None;
        }
    }

    Some(NewsItem {
        title,
        url,
        source: source.to_owned(),
        published: published_at.map(|date| date.to_rfc3339()),
        summary: clean_summary(
            entry
                .summary
                .as_ref()
                .map(|summary| summary.content.as_str())
                .or_else(|| {
                    entry
                        .content
                        .as_ref()
                        .and_then(|content| content.body.as_deref())
                })
                .unwrap_or_default(),
        ),
    })
}

fn build_digest_prompt(items: &[NewsItem], now: DateTime<Utc>) -> String {
    let mut prompt = format!(
        "你是資深 AI 產業分析師。請根據以下新聞，在 {} 產出繁體中文《AI 每日新聞洞察》。\n\n要求：\n1. 先用 5 點摘要最重要變化。\n2. 分析對模型、開發者工具、企業採用、監管/安全的影響。\n3. 給出 3 個明日應追蹤問題。\n4. 不要捏造未提供的事實；每個重點引用新聞標題。\n\n新聞：\n",
        now.format("%Y-%m-%d")
    );

    for (index, item) in items.iter().enumerate() {
        let published = item.published.as_deref().unwrap_or("unknown date");
        prompt.push_str(&format!(
            "{}. [{}] {} ({})\n來源：{}\n摘要：{}\n\n",
            index + 1,
            published,
            item.title,
            item.url,
            item.source,
            item.summary
        ));
    }

    prompt
}

fn generate_llama_text(base_config: &AppConfig, prompt: String) -> Result<String, CliError> {
    let config = AppConfig {
        prompt,
        ..base_config.clone()
    };
    let output = Command::new(&config.llama_cli)
        .args(build_llama_args(&config))
        .stdin(Stdio::null())
        .output()
        .map_err(|err| CliError::SpawnFailed(err.to_string()))?;

    if !output.status.success() {
        return Err(CliError::CommandFailed(output.status.code()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn render_digest_markdown(
    title: &str,
    generated_at: DateTime<Utc>,
    items: &[NewsItem],
    insight: &str,
) -> String {
    let mut markdown = format!(
        "# {title}\n\n產生時間：{}\n\n## Gemma 4 E4B 洞察\n\n{}\n\n## 今日新聞來源\n",
        generated_at.to_rfc3339(),
        insight.trim()
    );

    for item in items {
        let published = item.published.as_deref().unwrap_or("unknown date");
        markdown.push_str(&format!(
            "\n- **{}** — {} — [{}]({})\n  - {}\n",
            item.source, published, item.title, item.url, item.summary
        ));
    }

    markdown
}

fn publish_ntfy(topic: &str, title: &str, markdown: &str) -> Result<(), CliError> {
    reqwest::blocking::Client::new()
        .post(topic)
        .header("Title", title)
        .header("Tags", "newspaper,robot")
        .header("Priority", "default")
        .body(markdown.to_owned())
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| CliError::Http(format!("ntfy publish failed: {err}")))?;
    Ok(())
}

fn write_digest_files(document: &DigestDocument, config: &NewsConfig) -> Result<(), CliError> {
    write_text_file(&config.output_md, &document.markdown)?;
    let json =
        serde_json::to_string_pretty(document).map_err(|err| CliError::Json(err.to_string()))?;
    write_text_file(&config.output_json, &json)?;
    Ok(())
}

fn write_text_file(path: &Path, contents: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::Io(err.to_string()))?;
    }
    fs::write(path, contents).map_err(|err| CliError::Io(err.to_string()))
}

fn dry_run_digest(config: &NewsConfig) -> DigestDocument {
    let items = sample_news_items();
    let insight = "- Dry run：這是一份範例洞察，不會抓取 RSS、不會執行 llama-cli、不會發送 ntfy。\n- 正式排程會收集 AI RSS，交給 Gemma 4 E4B Q4_K_M GGUF 產出繁體中文洞察。\n- 使用 --send 或 AI_NEWS_SEND=1 才會推送到 ntfy。";
    DigestDocument {
        title: config.title.clone(),
        generated_at: Utc::now().to_rfc3339(),
        ntfy_topic: config.ntfy_topic.clone(),
        sent: false,
        markdown: render_digest_markdown(&config.title, Utc::now(), &items, insight),
        items,
    }
}

fn sample_news_items() -> Vec<NewsItem> {
    vec![
        NewsItem {
            title: "範例：新模型發布帶動開發者工具更新".to_owned(),
            url: "https://example.com/model-update".to_owned(),
            source: "Example AI Feed".to_owned(),
            published: Some("2026-05-11T00:00:00Z".to_owned()),
            summary: "示範資料：用於 React dashboard 與 dry-run 檔案輸出。".to_owned(),
        },
        NewsItem {
            title: "範例：企業 AI 採用從試點走向工作流整合".to_owned(),
            url: "https://example.com/enterprise-ai".to_owned(),
            source: "Example Enterprise Feed".to_owned(),
            published: Some("2026-05-11T01:00:00Z".to_owned()),
            summary: "示範資料：正式執行時會由 RSS 內容取代。".to_owned(),
        },
    ]
}

fn print_news_dry_run(config: &NewsConfig) {
    let prompt = build_digest_prompt(&sample_news_items(), Utc::now());
    let llama_config = AppConfig {
        prompt,
        ..config.llama.clone()
    };
    println!("# news dry-run");
    println!("feeds: {}", config.feeds.join(", "));
    println!("output_md: {}", config.output_md.display());
    println!("output_json: {}", config.output_json.display());
    println!("ntfy_topic: {}", config.ntfy_topic);
    println!("send: {}", config.send);
    println!(
        "llama: {}",
        shell_join(&llama_config.llama_cli, &build_llama_args(&llama_config))
    );
}

fn clean_summary(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_tag = false;

    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn next_value(flag: &str, iter: &mut impl Iterator<Item = OsString>) -> Result<OsString, CliError> {
    iter.next()
        .ok_or_else(|| CliError::MissingValue(flag.to_owned()))
}

fn next_string(flag: &str, iter: &mut impl Iterator<Item = OsString>) -> Result<String, CliError> {
    next_value(flag, iter).map(|value| value.to_string_lossy().into_owned())
}

fn parse_next<T>(
    flag: &str,
    expected: &str,
    iter: &mut impl Iterator<Item = OsString>,
) -> Result<T, CliError>
where
    T: std::str::FromStr,
{
    let value = next_string(flag, iter)?;
    value.parse::<T>().map_err(|_| CliError::InvalidValue {
        flag: flag.to_owned(),
        value,
        expected: expected.to_owned(),
    })
}

fn env_u32(key: &str) -> Option<u32> {
    env::var(key).ok()?.parse().ok()
}

fn env_i32(key: &str) -> Option<i32> {
    env::var(key).ok()?.parse().ok()
}

fn env_i64(key: &str) -> Option<i64> {
    env::var(key).ok()?.parse().ok()
}

fn env_usize(key: &str) -> Option<usize> {
    env::var(key).ok()?.parse().ok()
}

fn env_f32(key: &str) -> Option<f32> {
    env::var(key).ok()?.parse().ok()
}

fn env_bool(key: &str) -> bool {
    matches!(
        env::var(key).as_deref(),
        Ok("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn env_list(key: &str) -> Option<Vec<String>> {
    let values = env::var(key).ok()?;
    let list = values
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (!list.is_empty()).then_some(list)
}

fn push_optional<T: ToString>(args: &mut Vec<OsString>, flag: &str, value: Option<T>) {
    if let Some(value) = value {
        args.push(OsString::from(flag));
        args.push(OsString::from(value.to_string()));
    }
}

fn shell_join(program: &Path, args: &[OsString]) -> String {
    std::iter::once(program.as_os_str().to_owned())
        .chain(args.iter().cloned())
        .map(|part| shell_quote(&part.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "@%_+=:,./-".contains(c))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn print_help() {
    println!(
        "Rust + llama-cli + Gemma 4 E4B GGUF Q4_K_M AI news example\n\n\
Usage:\n  cargo run -- [infer-options] [-- llama-cli-extra-args...]\n  cargo run -- news [news-options] [infer-options] [-- llama-cli-extra-args...]\n\n\
Infer options:\n  --llama-cli <path>        Path to llama-cli (or LLAMA_CLI)\n  -m, --model <path>        Path to Gemma 4 E4B Q4_K_M GGUF (or GEMMA4_GGUF)\n  -p, --prompt <text>       Prompt text (or PROMPT)\n  --system-prompt <text>    System prompt (or SYSTEM_PROMPT)\n  --no-system-prompt        Do not send a system prompt\n  -c, --ctx-size <tokens>   Context size\n  -n, --predict <tokens>    Tokens to generate (default: 256; news default: 900)\n  --temp <value>            Temperature (default: 0.7; news default: 0.35)\n  --top-p <value>           Top-p (default: 0.95; news default: 0.9)\n  -t, --threads <count>     CPU thread count\n  -ngl, --gpu-layers <n>    Layers to offload to GPU\n  --dry-run                 Print command/plan without running network or model work\n  --allow-missing-model     Skip model-file validation\n\n\
News options:\n  --feed <url>              Add an RSS/Atom source\n  --clear-feeds             Remove default feeds before adding custom feeds\n  --max-items <n>           Maximum headlines to summarize (default: 12)\n  --days <n>                Include items from recent days (default: 2)\n  --output-md <path>        Markdown digest output (default: data/ai-news/latest.md)\n  --output-json <path>      JSON output for React (default: web/public/latest-news.example.json)\n  --ntfy-topic <url>        ntfy topic URL (default: https://ntfy.sh/wangsc_ainews)\n  --send                    Publish digest to ntfy after generation\n  --no-send                 Do not publish to ntfy\n  --title <text>            Digest title\n  -h, --help                Show this help\n\n\
Examples:\n  cargo run -- --model models/gemma4-e4b-it-Q4_K_M.gguf --prompt \"你好\" -- -fa on\n  cargo run -- news --dry-run\n  cargo run -- news --send --gpu-layers 99"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn dry_run_builds_jinja_single_turn_command() {
        let config = AppConfig {
            dry_run: true,
            model: PathBuf::from("models/example.gguf"),
            prompt: "hi".to_owned(),
            system_prompt: Some("sys".to_owned()),
            gpu_layers: Some(99),
            extra_args: os_args(&["-fa", "on"]),
            ..Default::default()
        };

        let args = build_llama_args(&config);
        let rendered = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            rendered,
            vec![
                "--model",
                "models/example.gguf",
                "--jinja",
                "--single-turn",
                "--prompt",
                "hi",
                "--system-prompt",
                "sys",
                "--predict",
                "256",
                "--temp",
                "0.7",
                "--top-p",
                "0.95",
                "--gpu-layers",
                "99",
                "-fa",
                "on"
            ]
        );
    }

    #[test]
    fn parses_known_flags_and_passthrough() {
        let config = parse_args(os_args(&[
            "--dry-run",
            "--llama-cli",
            "./llama-cli",
            "--model",
            "model.gguf",
            "--prompt",
            "hello",
            "--ctx-size",
            "4096",
            "--",
            "--seed",
            "42",
        ]))
        .expect("valid args");

        assert!(config.dry_run);
        assert_eq!(config.llama_cli, PathBuf::from("./llama-cli"));
        assert_eq!(config.model, PathBuf::from("model.gguf"));
        assert_eq!(config.prompt, "hello");
        assert_eq!(config.ctx_size, Some(4096));
        assert_eq!(config.extra_args, os_args(&["--seed", "42"]));
    }

    #[test]
    fn parses_news_flags_and_llama_passthrough() {
        let config = parse_news_args(os_args(&[
            "--clear-feeds",
            "--feed",
            "https://example.com/feed.xml",
            "--max-items",
            "5",
            "--days",
            "3",
            "--send",
            "--ntfy-topic",
            "https://ntfy.sh/wangsc_ainews",
            "--dry-run",
            "--",
            "--seed",
            "7",
        ]))
        .expect("valid news args");

        assert_eq!(config.feeds, vec!["https://example.com/feed.xml"]);
        assert_eq!(config.max_items, 5);
        assert_eq!(config.days, 3);
        assert!(config.send);
        assert!(config.llama.dry_run);
        assert_eq!(config.llama.extra_args, os_args(&["--seed", "7"]));
    }

    #[test]
    fn validates_non_empty_prompt() {
        let config = AppConfig {
            prompt: "   ".to_owned(),
            dry_run: true,
            ..Default::default()
        };

        assert_eq!(validate(&config), Err(CliError::EmptyPrompt));
    }

    #[test]
    fn builds_digest_prompt_with_titles_and_guardrails() {
        let items = sample_news_items();
        let prompt = build_digest_prompt(
            &items,
            DateTime::parse_from_rfc3339("2026-05-11T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        );

        assert!(prompt.contains("AI 每日新聞洞察"));
        assert!(prompt.contains("不要捏造"));
        assert!(prompt.contains("新模型發布"));
    }

    #[test]
    fn clean_summary_removes_html_tags() {
        assert_eq!(
            clean_summary("<p>Hello <b>AI</b></p>\nworld"),
            "Hello AI world"
        );
    }
}
