use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

use super::{RemoteCommand, parse_safe_integer_u64};

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_prompt_list(tokens),
        Some("add") => parse_prompt_add(tokens),
        Some(value) => Err(format!(
            "unknown remote prompts command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("remote prompts command is required\n\n{}", usage())),
    }
}

fn parse_prompt_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote prompts list")?;
    let mut before_created_at_ms = None;
    let mut before_prompt_id = None;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--before-created-at-ms" if before_created_at_ms.is_none() => {
                before_created_at_ms = Some(parse_safe_integer_u64(
                    &next_value(tokens, "--before-created-at-ms")?,
                    "--before-created-at-ms",
                )?);
            }
            "--before-prompt-id" if before_prompt_id.is_none() => {
                before_prompt_id = Some(next_value(tokens, "--before-prompt-id")?);
            }
            "--instance" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote prompts list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if !valid_prompt_cursor_fields(before_created_at_ms, before_prompt_id.as_deref()) {
        return Err(format!(
            "remote prompts list cursor requires a valid timestamp and Prompt ID pair\n\n{}",
            usage()
        ));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote prompts list --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::PromptsList {
        conversation_id,
        before_created_at_ms,
        before_prompt_id,
        instance_id,
        instance_view,
    }))
}

fn valid_prompt_cursor_fields(created_at_ms: Option<u64>, prompt_id: Option<&str>) -> bool {
    if created_at_ms.is_some() != prompt_id.is_some() {
        return false;
    }
    prompt_id.is_none_or(|id| {
        !id.trim().is_empty()
            && id.len() <= 128
            && !id.chars().any(char::is_control)
            && !id.contains('/')
    })
}

fn parse_prompt_add(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote prompts add")?;
    if tokens
        .front()
        .is_none_or(|value| value != "--expected-version")
    {
        return Err(format!(
            "remote prompt add requires --expected-version from the session list\n\n{}",
            usage()
        ));
    }
    tokens.pop_front();
    let version = next_value(tokens, "--expected-version")?;
    let expected_version = version
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0 && *value <= 9_007_199_254_740_991)
        .ok_or_else(|| format!("invalid --expected-version '{version}'\n\n{}", usage()))?;

    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.front().map(String::as_str) {
        match option {
            "--instance" => {
                if instance_id.is_some() {
                    return Err(format!(
                        "invalid remote prompts add option '--instance'\n\n{}",
                        usage()
                    ));
                }
                tokens.pop_front();
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" => {
                if instance_view.is_some() {
                    return Err(format!(
                        "invalid remote prompts add option '--instance-view'\n\n{}",
                        usage()
                    ));
                }
                tokens.pop_front();
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => break,
        }
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote prompts add --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    if tokens.is_empty() {
        return Err(format!("prompt content is required\n\n{}", usage()));
    }
    let content_tokens = tokens.drain(..).collect::<Vec<_>>();
    if content_tokens.first().is_some_and(|token| token == "-") && content_tokens.len() != 1 {
        return Err(format!(
            "remote stdin prompt marker '-' must be the only prompt token\n\n{}",
            usage()
        ));
    }
    let content = content_tokens.join(" ");
    Ok(Command::Remote(RemoteCommand::PromptsAdd {
        conversation_id,
        expected_version,
        content,
        instance_id,
        instance_view,
    }))
}
