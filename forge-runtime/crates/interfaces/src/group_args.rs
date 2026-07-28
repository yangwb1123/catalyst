use std::{collections::VecDeque, path::PathBuf};

use forge_runtime_application::{
    DEFAULT_GROUP_CONTEXT_CONTENT_BYTES, MAX_GROUP_CONTEXT_CONTENT_BYTES,
};

use super::{Command, GroupCommand, next_value, require_empty, required_text, usage};

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("create") => Ok(Command::Group(GroupCommand::Create {
            name: required_text(tokens, "group name")?,
        })),
        Some("add") => parse_add(tokens),
        Some("context") => parse_context(tokens),
        Some("list") => {
            require_empty(tokens)?;
            Ok(Command::Group(GroupCommand::List))
        }
        Some(value) => Err(format!("unknown group command '{value}'\n\n{}", usage())),
        None => Err(format!("group command is required\n\n{}", usage())),
    }
}

fn parse_context(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let group_id = required_group_id(tokens)?;
    let mut include_content = false;
    let mut max_bytes = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--include-content" if !include_content => include_content = true,
            "--include-content" => return Err(duplicate("--include-content")),
            "--max-bytes" if max_bytes.is_none() => {
                max_bytes = Some(parse_context_bytes(tokens)?);
            }
            "--max-bytes" => return Err(duplicate("--max-bytes")),
            _ => {
                return Err(format!(
                    "unknown group context option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    Ok(Command::Group(GroupCommand::Context {
        group_id,
        include_content,
        max_bytes: max_bytes.unwrap_or(DEFAULT_GROUP_CONTEXT_CONTENT_BYTES),
    }))
}

fn required_group_id(tokens: &mut VecDeque<String>) -> Result<String, String> {
    match tokens.front() {
        Some(value) if !value.starts_with('-') => next_value(tokens, "group context"),
        _ => Err(format!("group context requires GROUP_ID\n\n{}", usage())),
    }
}

fn parse_context_bytes(tokens: &mut VecDeque<String>) -> Result<usize, String> {
    let value = next_value(tokens, "--max-bytes")?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid --max-bytes '{value}'"))?;
    if (1..=MAX_GROUP_CONTEXT_CONTENT_BYTES).contains(&parsed) {
        return Ok(parsed);
    }
    Err(format!(
        "--max-bytes must be between 1 and {MAX_GROUP_CONTEXT_CONTENT_BYTES}"
    ))
}

fn parse_add(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let group_id = next_value(tokens, "group add")?;
    let project = PathBuf::from(next_value(tokens, "group add project")?);
    let mut role = "member".to_owned();
    if tokens.front().is_some_and(|value| value == "--role") {
        tokens.pop_front();
        role = next_value(tokens, "--role")?;
    }
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Add {
        group_id,
        project,
        role,
    }))
}

fn duplicate(option: &str) -> String {
    format!("{option} was specified more than once\n\n{}", usage())
}
