use std::{
    collections::VecDeque,
    path::{Component, Path, PathBuf},
};

use super::{
    Command, GlobalOptions, next_value, parse_optional_id_and_limit, require_empty, usage,
};

const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4_096;
const MAX_OUTPUT_TOKENS: u32 = 32_768;
const MAX_ALLOWED_READS: usize = 32;
const MAX_ALLOWED_READ_PATH_BYTES: usize = 1_024;

#[derive(Debug, Eq, PartialEq)]
pub enum RunCommand {
    Start {
        conversation_id: String,
        prompt_id: String,
        read_path: String,
        allowed_read_paths: Vec<String>,
        live: bool,
        model: Option<String>,
        max_output_tokens: u32,
    },
    List {
        conversation_id: Option<String>,
        limit: usize,
    },
    Show {
        run_id: String,
    },
}

#[derive(Default)]
struct StartOptions {
    read_path: Option<String>,
    allowed_read_paths: Vec<String>,
    live: bool,
    model: Option<String>,
    max_output_tokens: Option<u32>,
}

pub(super) fn parse(
    tokens: &mut VecDeque<String>,
    global: &mut GlobalOptions,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("start") => parse_start(tokens, global),
        Some("list") => parse_list(tokens),
        Some("show") => parse_show(tokens),
        Some(value) => Err(with_usage(&format!("unknown run command '{value}'"))),
        None => Err(with_usage("run command is required")),
    }
}

fn parse_start(
    tokens: &mut VecDeque<String>,
    global: &mut GlobalOptions,
) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "run start conversation")?;
    let prompt_id = next_value(tokens, "run start prompt")?;
    let mut options = StartOptions {
        read_path: global.read_path.take(),
        ..StartOptions::default()
    };
    parse_start_options(tokens, &mut options)?;
    validate_start_options(&options)?;
    Ok(Command::Run(RunCommand::Start {
        conversation_id,
        prompt_id,
        read_path: options.read_path.unwrap_or_else(|| "README.md".into()),
        allowed_read_paths: options.allowed_read_paths,
        live: options.live,
        model: options.model,
        max_output_tokens: options
            .max_output_tokens
            .unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS),
    }))
}

fn parse_start_options(
    tokens: &mut VecDeque<String>,
    options: &mut StartOptions,
) -> Result<(), String> {
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--read" => set_once(
                &mut options.read_path,
                next_value(tokens, "--read")?,
                "--read",
            )?,
            "--allow-read" => {
                if options.allowed_read_paths.len() == MAX_ALLOWED_READS {
                    return Err(with_usage(&format!(
                        "--allow-read may be specified at most {MAX_ALLOWED_READS} times"
                    )));
                }
                options
                    .allowed_read_paths
                    .push(next_value(tokens, "--allow-read")?);
            }
            "--live" if !options.live => options.live = true,
            "--live" => return Err(with_usage("--live was specified more than once")),
            "--model" => set_once(
                &mut options.model,
                next_value(tokens, "--model")?,
                "--model",
            )?,
            "--max-output-tokens" => {
                let value = next_value(tokens, "--max-output-tokens")?;
                let parsed = value
                    .parse()
                    .map_err(|_| with_usage(&format!("invalid --max-output-tokens '{value}'")))?;
                set_once(
                    &mut options.max_output_tokens,
                    parsed,
                    "--max-output-tokens",
                )?;
            }
            _ => return Err(with_usage(&format!("unknown run start option '{option}'"))),
        }
    }
    Ok(())
}

fn validate_start_options(options: &StartOptions) -> Result<(), String> {
    if options.live && options.read_path.is_some() {
        return Err(with_usage("--read cannot be combined with --live"));
    }
    if !options.live && !options.allowed_read_paths.is_empty() {
        return Err(with_usage("--allow-read is only valid with --live"));
    }
    if !options.live && (options.model.is_some() || options.max_output_tokens.is_some()) {
        return Err(with_usage("--model and --max-output-tokens require --live"));
    }
    if let Some(tokens) = options.max_output_tokens
        && !(1..=MAX_OUTPUT_TOKENS).contains(&tokens)
    {
        return Err(with_usage(&format!(
            "--max-output-tokens must be between 1 and {MAX_OUTPUT_TOKENS}"
        )));
    }
    for (index, path) in options.allowed_read_paths.iter().enumerate() {
        validate_allowed_read_path(path)?;
        if options.allowed_read_paths[..index].contains(path) {
            return Err(with_usage(&format!(
                "--allow-read path '{path}' was specified more than once"
            )));
        }
    }
    Ok(())
}

fn validate_allowed_read_path(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(with_usage(
            "--allow-read requires a non-empty relative file",
        ));
    }
    if value.len() > MAX_ALLOWED_READ_PATH_BYTES {
        return Err(with_usage(&format!(
            "--allow-read paths may contain at most {MAX_ALLOWED_READ_PATH_BYTES} bytes"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(with_usage(
            "--allow-read paths must not contain control characters",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(with_usage("--allow-read requires a relative file"));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return Err(with_usage(
                "--allow-read requires a clean relative file without '.' or '..'",
            ));
        };
        normalized.push(segment);
    }
    if normalized.as_os_str() != path.as_os_str() {
        return Err(with_usage(
            "--allow-read requires a clean relative file without redundant separators",
        ));
    }
    Ok(())
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let (conversation_id, limit) = parse_optional_id_and_limit(tokens)?;
    Ok(Command::Run(RunCommand::List {
        conversation_id,
        limit,
    }))
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let run_id = next_value(tokens, "run show")?;
    require_empty(tokens)?;
    Ok(Command::Run(RunCommand::Show { run_id }))
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        return Err(with_usage(&format!(
            "{option} was specified more than once"
        )));
    }
    Ok(())
}

fn with_usage(message: &str) -> String {
    format!("{message}\n\n{}", usage())
}
