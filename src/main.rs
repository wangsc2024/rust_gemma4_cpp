use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const DEFAULT_MODEL: &str = "models/gemma4-e4b-it-Q4_K_M.gguf";
const DEFAULT_PROMPT: &str = "請用繁體中文用三點說明 Gemma 4 E4B Q4_K_M GGUF 適合本機推論的原因。";
const DEFAULT_SYSTEM_PROMPT: &str = "你是精準、簡潔且使用繁體中文回答的助理。";

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
    let config = parse_args(args)?;
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

fn parse_args(args: Vec<OsString>) -> Result<AppConfig, CliError> {
    let mut config = AppConfig::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        let arg_text = arg.to_string_lossy();
        if arg_text == "--" {
            config.extra_args.extend(iter);
            break;
        }

        match arg_text.as_ref() {
            "-h" | "--help" => return Err(CliError::HelpRequested),
            "--dry-run" => config.dry_run = true,
            "--allow-missing-model" => config.allow_missing_model = true,
            "--no-system-prompt" => config.system_prompt = None,
            "--llama-cli" => {
                config.llama_cli = PathBuf::from(next_value("--llama-cli", &mut iter)?)
            }
            "-m" | "--model" => config.model = PathBuf::from(next_value("--model", &mut iter)?),
            "-p" | "--prompt" => config.prompt = next_string("--prompt", &mut iter)?,
            "--system-prompt" | "-sys" => {
                config.system_prompt = Some(next_string("--system-prompt", &mut iter)?)
            }
            "-c" | "--ctx-size" => {
                config.ctx_size = Some(parse_next("--ctx-size", "a positive integer", &mut iter)?)
            }
            "-n" | "--predict" | "--n-predict" => {
                config.predict = Some(parse_next(
                    "--predict",
                    "an integer token count",
                    &mut iter,
                )?)
            }
            "--temp" => {
                config.temp = Some(parse_next("--temp", "a floating point number", &mut iter)?)
            }
            "--top-p" => {
                config.top_p = Some(parse_next("--top-p", "a floating point number", &mut iter)?)
            }
            "-t" | "--threads" => {
                config.threads = Some(parse_next("--threads", "a positive integer", &mut iter)?)
            }
            "-ngl" | "--gpu-layers" => {
                config.gpu_layers = Some(parse_next("--gpu-layers", "an integer", &mut iter)?)
            }
            flag if flag.starts_with('-') => return Err(CliError::UnknownFlag(flag.to_owned())),
            value => config.extra_args.push(OsString::from(value)),
        }
    }

    Ok(config)
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

    if !config.dry_run && !config.allow_missing_model && !config.model.is_file() {
        return Err(CliError::MissingModel(config.model.clone()));
    }

    Ok(())
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

fn env_f32(key: &str) -> Option<f32> {
    env::var(key).ok()?.parse().ok()
}

fn env_bool(key: &str) -> bool {
    matches!(
        env::var(key).as_deref(),
        Ok("1" | "true" | "TRUE" | "yes" | "YES")
    )
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
        "Rust + llama-cli + Gemma 4 E4B GGUF Q4_K_M example\n\n\
Usage:\n  cargo run -- [options] [-- llama-cli-extra-args...]\n\n\
Options:\n  --llama-cli <path>        Path to llama-cli (or LLAMA_CLI)\n  -m, --model <path>        Path to Gemma 4 E4B Q4_K_M GGUF (or GEMMA4_GGUF)\n  -p, --prompt <text>       Prompt text (or PROMPT)\n  --system-prompt <text>    System prompt (or SYSTEM_PROMPT)\n  --no-system-prompt        Do not send a system prompt\n  -c, --ctx-size <tokens>   Context size\n  -n, --predict <tokens>    Tokens to generate (default: 256)\n  --temp <value>            Temperature (default: 0.7)\n  --top-p <value>           Top-p (default: 0.95)\n  -t, --threads <count>     CPU thread count\n  -ngl, --gpu-layers <n>    Layers to offload to GPU\n  --dry-run                 Print llama-cli command without running it\n  --allow-missing-model     Skip model-file validation\n  -h, --help                Show this help\n\n\
Example:\n  cargo run -- --model models/gemma4-e4b-it-Q4_K_M.gguf --prompt \"你好\" -- -fa on"
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
    fn validates_non_empty_prompt() {
        let config = AppConfig {
            prompt: "   ".to_owned(),
            dry_run: true,
            ..Default::default()
        };

        assert_eq!(validate(&config), Err(CliError::EmptyPrompt));
    }
}
