use std::collections::VecDeque;

use crate::runtime_domain::{
    MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT, MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS,
};

use super::{Command, GroupCommand, GroupSynthesisCommand, next_value, require_empty, usage};

const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4_096;

pub(super) fn parse(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("prepare") => parse_prepare(tokens, idempotency_key),
        Some("send") => parse_send(tokens),
        Some("show") => parse_show(tokens),
        Some("list") => parse_list(tokens),
        Some(value) => Err(with_usage(&format!(
            "unknown group synthesis command '{value}'"
        ))),
        None => Err(with_usage("group synthesis command is required")),
    }
}

fn parse_prepare(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let panel_id = required_id(tokens, "group synthesis prepare", "PANEL_ID")?;
    let mut model = None;
    let mut max_output_tokens = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--model" => set_once(&mut model, next_value(tokens, "--model")?, "--model")?,
            "--max-output-tokens" => {
                let value = parse_output_tokens(tokens)?;
                set_once(&mut max_output_tokens, value, "--max-output-tokens")?;
            }
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown("group synthesis prepare", &option)),
        }
    }
    Ok(Command::Group(GroupCommand::Synthesis(
        GroupSynthesisCommand::Prepare {
            panel_id,
            model,
            max_output_tokens: max_output_tokens.unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS),
        },
    )))
}

fn parse_send(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let synthesis_id = required_id(tokens, "group synthesis send", "SYNTHESIS_ID")?;
    let (confirm_off_machine, include_result) = parse_visibility(tokens, true)?;
    Ok(Command::Group(GroupCommand::Synthesis(
        GroupSynthesisCommand::Send {
            synthesis_id,
            confirm_off_machine,
            include_result,
        },
    )))
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let synthesis_id = required_id(tokens, "group synthesis show", "SYNTHESIS_ID")?;
    let (_, include_result) = parse_visibility(tokens, false)?;
    Ok(Command::Group(GroupCommand::Synthesis(
        GroupSynthesisCommand::Show {
            synthesis_id,
            include_result,
        },
    )))
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let panel_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Synthesis(
        GroupSynthesisCommand::List { panel_id, limit },
    )))
}

fn parse_visibility(
    tokens: &mut VecDeque<String>,
    allow_consent: bool,
) -> Result<(bool, bool), String> {
    let mut confirm = false;
    let mut include = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--confirm-off-machine" if allow_consent && !confirm => confirm = true,
            "--confirm-off-machine" if allow_consent => {
                return Err(duplicate("--confirm-off-machine"));
            }
            "--include-result" if !include => include = true,
            "--include-result" => return Err(duplicate("--include-result")),
            _ => return Err(unknown("group synthesis output", &option)),
        }
    }
    Ok((confirm, include))
}

fn parse_output_tokens(tokens: &mut VecDeque<String>) -> Result<u32, String> {
    let value = next_value(tokens, "--max-output-tokens")?;
    let parsed = value
        .parse::<u32>()
        .map_err(|_| with_usage(&format!("invalid --max-output-tokens '{value}'")))?;
    if (1..=MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(with_usage(&format!(
            "--max-output-tokens must be between 1 and {MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS}"
        )))
    }
}

fn parse_limit(tokens: &mut VecDeque<String>) -> Result<usize, String> {
    let value = next_value(tokens, "--limit")?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| with_usage(&format!("invalid --limit '{value}'")))?;
    if (1..=MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(with_usage(&format!(
            "--limit must be between 1 and {MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT}"
        )))
    }
}

fn required_id(
    tokens: &mut VecDeque<String>,
    operation: &str,
    field: &str,
) -> Result<String, String> {
    match tokens.front() {
        Some(value) if !value.starts_with('-') => next_value(tokens, operation),
        _ => Err(with_usage(&format!("{operation} requires {field}"))),
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> Result<(), String> {
    if slot.replace(value).is_none() {
        Ok(())
    } else {
        Err(duplicate(option))
    }
}

fn duplicate(option: &str) -> String {
    with_usage(&format!("{option} was specified more than once"))
}

fn unknown(operation: &str, option: &str) -> String {
    with_usage(&format!("unknown {operation} option '{option}'"))
}

fn with_usage(message: &str) -> String {
    format!("{message}\n\n{}", usage())
}
