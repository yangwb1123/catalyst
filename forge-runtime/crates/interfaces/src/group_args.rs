use std::{collections::VecDeque, path::PathBuf};

use forge_runtime_application::{
    DEFAULT_GROUP_CONTEXT_CONTENT_BYTES, MAX_GROUP_CONTEXT_CONTENT_BYTES,
    MAX_GROUP_EXECUTION_LIST_LIMIT, MAX_GROUP_RUN_LIST_LIMIT,
};

use super::{
    Command, GroupCommand, GroupExecutionCommand, GroupRunCommand, group_analysis_args, next_value,
    require_empty, required_text, usage,
};

pub(super) fn parse(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("create") => Ok(Command::Group(GroupCommand::Create {
            name: required_text(tokens, "group name")?,
        })),
        Some("add") => parse_add(tokens),
        Some("analysis") => group_analysis_args::parse(tokens, idempotency_key),
        Some("context") => parse_context(tokens),
        Some("execution") => parse_execution(tokens, idempotency_key),
        Some("run") => parse_run(tokens, idempotency_key),
        Some("list") => {
            require_empty(tokens)?;
            Ok(Command::Group(GroupCommand::List))
        }
        Some(value) => Err(format!("unknown group command '{value}'\n\n{}", usage())),
        None => Err(format!("group command is required\n\n{}", usage())),
    }
}

fn parse_execution(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("start") => parse_execution_start(tokens, idempotency_key),
        Some("show") => parse_execution_show(tokens),
        Some("list") => parse_execution_list(tokens),
        Some(value) => Err(format!(
            "unknown group execution command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "group execution command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_execution_start(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let group_run_id = required_id(tokens, "group execution start", "GROUP_RUN_ID")?;
    if tokens
        .front()
        .is_some_and(|value| value == "--idempotency-key")
    {
        tokens.pop_front();
        if idempotency_key.is_some() {
            return Err(duplicate("--idempotency-key"));
        }
        *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
    }
    require_empty(tokens)?;
    if idempotency_key.is_none() {
        return Err(format!(
            "group execution start requires an explicit --idempotency-key for durable recovery\n\n{}",
            usage()
        ));
    }
    Ok(Command::Group(GroupCommand::Execution(
        GroupExecutionCommand::Start { group_run_id },
    )))
}

fn parse_execution_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let execution_id = required_id(tokens, "group execution show", "EXECUTION_ID")?;
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Execution(
        GroupExecutionCommand::Show { execution_id },
    )))
}

fn parse_execution_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let group_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_list_limit(tokens, MAX_GROUP_EXECUTION_LIST_LIMIT)?;
    }
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Execution(
        GroupExecutionCommand::List {
            group_run_id,
            limit,
        },
    )))
}

fn parse_context(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let group_id = required_id(tokens, "group context", "GROUP_ID")?;
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

fn parse_run(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("prepare") => parse_run_prepare(tokens, idempotency_key),
        Some("show") => parse_run_show(tokens),
        Some("list") => parse_run_list(tokens),
        Some(value) => Err(format!(
            "unknown group run command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("group run command is required\n\n{}", usage())),
    }
}

fn parse_run_prepare(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let group_id = required_id(tokens, "group run prepare", "GROUP_ID")?;
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
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown_option("group run prepare", &option)),
        }
    }
    Ok(Command::Group(GroupCommand::Run(
        GroupRunCommand::Prepare {
            group_id,
            include_content,
            max_bytes: max_bytes.unwrap_or(DEFAULT_GROUP_CONTEXT_CONTENT_BYTES),
        },
    )))
}

fn parse_run_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let run_id = required_id(tokens, "group run show", "RUN_ID")?;
    let include_content = match tokens.pop_front().as_deref() {
        Some("--include-content") => true,
        Some(option) => return Err(unknown_option("group run show", option)),
        None => false,
    };
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Run(GroupRunCommand::Show {
        run_id,
        include_content,
    })))
}

fn parse_run_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let group_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_list_limit(tokens, MAX_GROUP_RUN_LIST_LIMIT)?;
    }
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Run(GroupRunCommand::List {
        group_id,
        limit,
    })))
}

fn required_id(
    tokens: &mut VecDeque<String>,
    operation: &str,
    field: &str,
) -> Result<String, String> {
    match tokens.front() {
        Some(value) if !value.starts_with('-') => next_value(tokens, operation),
        _ => Err(format!("{operation} requires {field}\n\n{}", usage())),
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

fn parse_list_limit(tokens: &mut VecDeque<String>, max: usize) -> Result<usize, String> {
    let value = next_value(tokens, "--limit")?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid --limit '{value}'"))?;
    if (1..=max).contains(&parsed) {
        return Ok(parsed);
    }
    Err(format!("--limit must be between 1 and {max}"))
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

fn unknown_option(operation: &str, option: &str) -> String {
    format!("unknown {operation} option '{option}'\n\n{}", usage())
}
