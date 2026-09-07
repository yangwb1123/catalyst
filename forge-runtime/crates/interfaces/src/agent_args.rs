use std::collections::VecDeque;

pub const DEFAULT_MAX_TURNS: u32 = 24;
pub const DEFAULT_MAX_TOOL_CALLS: u32 = 64;
pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4_096;
pub const MAX_TURNS: u32 = 64;
pub const MAX_TOOL_CALLS: u32 = 256;
pub const MAX_OUTPUT_TOKENS: u32 = 32_768;

pub(crate) const MAX_PROMPT_BYTES: usize = 256 * 1024;
const MAX_SESSION_BYTES: usize = 128;
const MAX_MODEL_BYTES: usize = 128;

#[derive(Debug, Eq, PartialEq)]
pub struct AgentArgs {
    pub command: AgentCommand,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AgentCommand {
    Run {
        prompt: String,
        session: Option<String>,
        model: Option<String>,
        dev: bool,
        max_turns: u32,
        max_tool_calls: u32,
        max_output_tokens: u32,
    },
}

#[derive(Default)]
struct Options {
    session: Option<String>,
    model: Option<String>,
    dev: bool,
    max_turns: Option<u32>,
    max_tool_calls: Option<u32>,
    max_output_tokens: Option<u32>,
}

pub fn parse(tokens: &mut VecDeque<String>) -> Result<AgentArgs, String> {
    let mut options = Options::default();
    parse_options(tokens, &mut options)?;
    let prompt = parse_prompt(tokens)?;
    Ok(AgentArgs {
        command: AgentCommand::Run {
            prompt,
            session: options.session,
            model: options.model,
            dev: options.dev,
            max_turns: options.max_turns.unwrap_or(DEFAULT_MAX_TURNS),
            max_tool_calls: options.max_tool_calls.unwrap_or(DEFAULT_MAX_TOOL_CALLS),
            max_output_tokens: options
                .max_output_tokens
                .unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS),
        },
    })
}

fn parse_options(tokens: &mut VecDeque<String>, options: &mut Options) -> Result<(), String> {
    loop {
        let Some(option) = tokens.front().map(String::as_str) else {
            return Ok(());
        };
        if option == "--" {
            tokens.pop_front();
            return Ok(());
        }
        if option == "-" {
            return Ok(());
        }
        if !option.starts_with('-') {
            return Ok(());
        }
        let option = tokens.pop_front().expect("front token exists");
        parse_option(&option, tokens, options)?;
    }
}

fn parse_option(
    option: &str,
    tokens: &mut VecDeque<String>,
    options: &mut Options,
) -> Result<(), String> {
    match option {
        "--session" => {
            let value = option_value(tokens, option)?;
            validate_label(&value, option, MAX_SESSION_BYTES)?;
            set_once(&mut options.session, value, option)
        }
        "--model" => {
            let value = option_value(tokens, option)?;
            validate_label(&value, option, MAX_MODEL_BYTES)?;
            set_once(&mut options.model, value, option)
        }
        "--dev" if !options.dev => {
            options.dev = true;
            Ok(())
        }
        "--dev" => Err(duplicate(option)),
        "--max-turns" => parse_number_option(tokens, &mut options.max_turns, option, MAX_TURNS),
        "--max-tool-calls" => {
            parse_number_option(tokens, &mut options.max_tool_calls, option, MAX_TOOL_CALLS)
        }
        "--max-output-tokens" => parse_number_option(
            tokens,
            &mut options.max_output_tokens,
            option,
            MAX_OUTPUT_TOKENS,
        ),
        _ => Err(format!("unknown agent option '{option}'")),
    }
}

fn parse_number_option(
    tokens: &mut VecDeque<String>,
    target: &mut Option<u32>,
    option: &str,
    maximum: u32,
) -> Result<(), String> {
    let value = option_value(tokens, option)?;
    let number = value
        .parse::<u32>()
        .map_err(|_| format!("invalid {option} '{value}'"))?;
    if !(1..=maximum).contains(&number) {
        return Err(format!("{option} must be between 1 and {maximum}"));
    }
    set_once(target, number, option)
}

fn option_value(tokens: &mut VecDeque<String>, option: &str) -> Result<String, String> {
    let value = tokens
        .pop_front()
        .ok_or_else(|| format!("{option} requires a value"))?;
    if value == "--" || value.starts_with('-') {
        return Err(format!("{option} requires a value"));
    }
    Ok(value)
}

fn validate_label(value: &str, option: &str, maximum: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{option} requires a non-empty value"));
    }
    if value.len() > maximum {
        return Err(format!("{option} may contain at most {maximum} bytes"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{option} must not contain control characters"));
    }
    Ok(())
}

fn parse_prompt(tokens: &mut VecDeque<String>) -> Result<String, String> {
    if tokens.is_empty() {
        return Err("agent prompt is required".into());
    }
    if tokens.front().is_some_and(|token| token == "-") && tokens.len() != 1 {
        return Err("agent stdin prompt marker '-' must be the only prompt token".into());
    }
    let prompt = tokens.drain(..).collect::<Vec<_>>().join(" ");
    validate_prompt(&prompt)?;
    Ok(prompt)
}

pub(crate) fn validate_prompt(prompt: &str) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("agent prompt must not be empty".into());
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(format!(
            "agent prompt may contain at most {MAX_PROMPT_BYTES} bytes"
        ));
    }
    Ok(())
}

fn set_once<T>(target: &mut Option<T>, value: T, option: &str) -> Result<(), String> {
    if target.is_some() {
        return Err(duplicate(option));
    }
    *target = Some(value);
    Ok(())
}

fn duplicate(option: &str) -> String {
    format!("{option} was specified more than once")
}

#[cfg(test)]
mod tests {
    use super::{
        AgentArgs, AgentCommand, DEFAULT_MAX_OUTPUT_TOKENS, DEFAULT_MAX_TOOL_CALLS,
        DEFAULT_MAX_TURNS, MAX_MODEL_BYTES, MAX_OUTPUT_TOKENS, MAX_PROMPT_BYTES, MAX_SESSION_BYTES,
        MAX_TOOL_CALLS, MAX_TURNS, parse,
    };
    use std::collections::VecDeque;

    #[test]
    fn prompt_uses_defaults() {
        assert_eq!(
            parse_args(["explain", "this"]),
            Ok(run("explain this", None, None, false, 24, 64, 4_096))
        );
        assert_eq!(DEFAULT_MAX_TURNS, 24);
        assert_eq!(DEFAULT_MAX_TOOL_CALLS, 64);
        assert_eq!(DEFAULT_MAX_OUTPUT_TOKENS, 4_096);
    }

    #[test]
    fn options_precede_prompt_and_delimiter_allows_leading_dash() {
        assert_eq!(
            parse_args([
                "--session",
                "session-1",
                "--model",
                "gpt-test",
                "--dev",
                "--max-turns",
                "64",
                "--max-tool-calls",
                "256",
                "--max-output-tokens",
                "32768",
                "--",
                "--fix",
                "tests",
            ]),
            Ok(run(
                "--fix tests",
                Some("session-1"),
                Some("gpt-test"),
                true,
                64,
                256,
                32_768,
            ))
        );
    }

    #[test]
    fn tokens_after_prompt_are_prompt_text() {
        assert_eq!(
            parse_args(["fix", "--model", "literally"]),
            Ok(run(
                "fix --model literally",
                None,
                None,
                false,
                24,
                64,
                4_096,
            ))
        );
    }

    #[test]
    fn a_single_dash_selects_stdin_and_cannot_be_mixed_with_argv_text() {
        assert_eq!(
            parse_args(["-"]),
            Ok(run("-", None, None, false, 24, 64, 4_096))
        );
        assert_error(["-", "extra"], "only prompt token");
    }

    #[test]
    fn missing_prompt_and_unknown_or_duplicate_options_fail() {
        assert_error([], "agent prompt is required");
        assert_error(["--"], "agent prompt is required");
        assert_error(["--wat", "prompt"], "unknown agent option");
        assert_error(["--dev", "--dev", "prompt"], "more than once");
        assert_error(
            ["--model", "one", "--model", "two", "prompt"],
            "more than once",
        );
        assert_error(["--session", "--dev", "prompt"], "requires a value");
    }

    #[test]
    fn numeric_bounds_are_closed() {
        for (option, maximum) in [
            ("--max-turns", MAX_TURNS),
            ("--max-tool-calls", MAX_TOOL_CALLS),
            ("--max-output-tokens", MAX_OUTPUT_TOKENS),
        ] {
            assert!(parse_args([option, "1", "prompt"]).is_ok());
            assert!(parse_args([option, &maximum.to_string(), "prompt"]).is_ok());
            assert_error([option, "0", "prompt"], "must be between");
            assert_error(
                [option, &(u64::from(maximum) + 1).to_string(), "prompt"],
                "must be between",
            );
            assert_error([option, "nope", "prompt"], "invalid");
        }
    }

    #[test]
    fn text_bounds_are_enforced() {
        assert!(parse_args(["--session", &"s".repeat(MAX_SESSION_BYTES), "p"]).is_ok());
        assert!(parse_args(["--model", &"m".repeat(MAX_MODEL_BYTES), "p"]).is_ok());
        assert!(parse_args([&"p".repeat(MAX_PROMPT_BYTES)]).is_ok());
        assert_error(
            ["--session", &"s".repeat(MAX_SESSION_BYTES + 1), "p"],
            "at most",
        );
        assert_error(
            ["--model", &"m".repeat(MAX_MODEL_BYTES + 1), "p"],
            "at most",
        );
        assert_error([&"p".repeat(MAX_PROMPT_BYTES + 1)], "at most");
        assert_error(["--model", "bad\nmodel", "p"], "control characters");
        assert_error(["   "], "must not be empty");
    }

    fn parse_args<const N: usize>(values: [&str; N]) -> Result<AgentArgs, String> {
        let mut tokens = values
            .into_iter()
            .map(str::to_owned)
            .collect::<VecDeque<_>>();
        parse(&mut tokens)
    }

    fn assert_error<const N: usize>(values: [&str; N], expected: &str) {
        let error = parse_args(values).expect_err("arguments should be rejected");
        assert!(error.contains(expected), "unexpected error: {error}");
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        prompt: &str,
        session: Option<&str>,
        model: Option<&str>,
        dev: bool,
        max_turns: u32,
        max_tool_calls: u32,
        max_output_tokens: u32,
    ) -> AgentArgs {
        AgentArgs {
            command: AgentCommand::Run {
                prompt: prompt.into(),
                session: session.map(str::to_owned),
                model: model.map(str::to_owned),
                dev,
                max_turns,
                max_tool_calls,
                max_output_tokens,
            },
        }
    }
}
