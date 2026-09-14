use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

#[derive(Debug, Eq, PartialEq)]
pub enum RemoteCommand {
    Login,
    Tui,
    SessionsList {
        after_id: Option<String>,
        scope: Option<RemoteConversationScope>,
        all_pages: bool,
    },
    SessionsCreate {
        title: String,
        scope: RemoteConversationScope,
    },
    SessionsImport {
        conversation_id: String,
        confirm: Option<String>,
    },
    RunsList {
        conversation_id: String,
        limit: usize,
        before_created_at_ms: Option<u64>,
        before_run_id: Option<String>,
    },
    RunTimeline {
        conversation_id: String,
        run_id: String,
        after_sequence: u64,
        limit: usize,
    },
    PromptsList {
        conversation_id: String,
        before_created_at_ms: Option<u64>,
        before_prompt_id: Option<String>,
    },
    PromptsAdd {
        conversation_id: String,
        expected_version: u64,
        content: String,
    },
    ChangesList {
        after_cursor: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteConversationScope {
    Global,
    Project(String),
    Group(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptPageCursor {
    pub created_at_ms: u64,
    pub prompt_id: String,
}

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("login") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::Login))
        }
        Some("tui") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::Tui))
        }
        Some("sessions") => parse_sessions(tokens),
        Some("runs") => parse_runs(tokens),
        Some("prompts") => parse_prompts(tokens),
        Some("changes") => parse_changes(tokens),
        Some(value) => Err(format!("unknown remote command '{value}'\n\n{}", usage())),
        None => Err(format!("remote command is required\n\n{}", usage())),
    }
}

fn parse_runs(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_run_list(tokens),
        Some("timeline") => parse_run_timeline(tokens),
        Some(value) => Err(format!(
            "unknown remote runs command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("remote runs command is required\n\n{}", usage())),
    }
}

fn parse_run_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote runs list")?;
    let mut limit = 25;
    let mut limit_seen = false;
    let mut before_created_at_ms = None;
    let mut before_run_id = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--limit" if !limit_seen => {
                limit = parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 25, "--limit")?;
                limit_seen = true;
            }
            "--before-created-at-ms" if before_created_at_ms.is_none() => {
                before_created_at_ms = Some(parse_sqlite_u64(
                    &next_value(tokens, "--before-created-at-ms")?,
                    "--before-created-at-ms",
                )?);
            }
            "--before-run-id" if before_run_id.is_none() => {
                before_run_id = Some(next_value(tokens, "--before-run-id")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote runs list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if before_created_at_ms.is_some() != before_run_id.is_some() {
        return Err(format!(
            "remote runs list cursor requires both --before-created-at-ms and --before-run-id\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::RunsList {
        conversation_id,
        limit,
        before_created_at_ms,
        before_run_id,
    }))
}

fn parse_run_timeline(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote runs timeline")?;
    let run_id = next_value(tokens, "remote runs timeline")?;
    let mut after_sequence = 0;
    let mut limit = 128;
    let mut after_sequence_seen = false;
    let mut limit_seen = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--after-sequence" if !after_sequence_seen => {
                after_sequence =
                    parse_sqlite_u64(&next_value(tokens, "--after-sequence")?, "--after-sequence")?;
                after_sequence_seen = true;
            }
            "--limit" if !limit_seen => {
                limit = parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 128, "--limit")?;
                limit_seen = true;
            }
            _ => {
                return Err(format!(
                    "invalid remote runs timeline option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    Ok(Command::Remote(RemoteCommand::RunTimeline {
        conversation_id,
        run_id,
        after_sequence,
        limit,
    }))
}

fn parse_sqlite_u64(value: &str, option: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|parsed| i64::try_from(*parsed).is_ok())
        .ok_or_else(|| format!("invalid {option} '{value}'\n\n{}", usage()))
}

fn parse_bounded_u64(value: &str, min: u64, max: u64, option: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|parsed| (*parsed >= min) && (*parsed <= max))
        .ok_or_else(|| format!("{option} must be between {min} and {max}\n\n{}", usage()))
}

fn parse_bounded_usize(value: &str, min: u64, max: u64, option: &str) -> Result<usize, String> {
    usize::try_from(parse_bounded_u64(value, min, max, option)?).map_err(|_| {
        format!(
            "{option} exceeds this platform's integer range\n\n{}",
            usage()
        )
    })
}

fn parse_changes(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => {
            let after_cursor = if tokens
                .front()
                .is_some_and(|value| value == "--after-cursor")
            {
                tokens.pop_front();
                let raw = next_value(tokens, "--after-cursor")?;
                Some(
                    raw.parse::<u64>()
                        .ok()
                        .filter(|value| i64::try_from(*value).is_ok())
                        .ok_or_else(|| format!("invalid --after-cursor '{raw}'\n\n{}", usage()))?,
                )
            } else {
                None
            };
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::ChangesList { after_cursor }))
        }
        Some(value) => Err(format!(
            "unknown remote changes command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("remote changes command is required\n\n{}", usage())),
    }
}

fn parse_sessions(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_session_list(tokens),
        Some("create") => parse_session_create(tokens),
        Some("import") => parse_session_import(tokens),
        Some(value) => Err(format!(
            "unknown remote sessions command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote sessions command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_session_import(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote sessions import")?;
    let mut confirm = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--confirm" if confirm.is_none() => {
                let digest = next_value(tokens, "--confirm")?;
                if digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(format!(
                        "--confirm must be a 64-character lowercase SHA-256 digest\n\n{}",
                        usage()
                    ));
                }
                confirm = Some(digest);
            }
            _ => {
                return Err(format!(
                    "invalid remote sessions import option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    Ok(Command::Remote(RemoteCommand::SessionsImport {
        conversation_id,
        confirm,
    }))
}

fn parse_session_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut after_id = None;
    let mut scope = None;
    let mut all_pages = false;
    let mut all_pages_seen = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--after" if after_id.is_none() => after_id = Some(next_value(tokens, "--after")?),
            "--scope" if scope.is_none() => {
                scope = Some(parse_scope(&next_value(tokens, "--scope")?)?);
            }
            "--all" if !all_pages_seen => {
                all_pages = true;
                all_pages_seen = true;
            }
            _ => {
                return Err(format!(
                    "invalid remote sessions list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    Ok(Command::Remote(RemoteCommand::SessionsList {
        after_id,
        scope,
        all_pages,
    }))
}

fn parse_session_create(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut title = None;
    let mut scope = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--title" if title.is_none() => title = Some(next_value(tokens, "--title")?),
            "--scope" if scope.is_none() => {
                scope = Some(parse_scope(&next_value(tokens, "--scope")?)?);
            }
            _ => {
                return Err(format!(
                    "invalid remote sessions create option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    Ok(Command::Remote(RemoteCommand::SessionsCreate {
        title: title.unwrap_or_else(|| "New conversation".to_owned()),
        scope: scope.unwrap_or(RemoteConversationScope::Global),
    }))
}

pub(crate) fn parse_scope(value: &str) -> Result<RemoteConversationScope, String> {
    if value == "global" {
        return Ok(RemoteConversationScope::Global);
    }
    let (kind, id) = value.split_once(':').ok_or_else(|| {
        format!(
            "scope must be global, project:ID, or group:ID\n\n{}",
            usage()
        )
    })?;
    if id.trim().is_empty()
        || id.len() > 128
        || id.chars().any(char::is_control)
        || id.contains('/')
    {
        return Err(format!(
            "remote conversation scope id is invalid\n\n{}",
            usage()
        ));
    }
    match kind {
        "project" => Ok(RemoteConversationScope::Project(id.to_owned())),
        "group" => Ok(RemoteConversationScope::Group(id.to_owned())),
        _ => Err(format!(
            "scope must be global, project:ID, or group:ID\n\n{}",
            usage()
        )),
    }
}

fn parse_prompts(tokens: &mut VecDeque<String>) -> Result<Command, String> {
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
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--before-created-at-ms" if before_created_at_ms.is_none() => {
                before_created_at_ms = Some(parse_sqlite_u64(
                    &next_value(tokens, "--before-created-at-ms")?,
                    "--before-created-at-ms",
                )?);
            }
            "--before-prompt-id" if before_prompt_id.is_none() => {
                before_prompt_id = Some(next_value(tokens, "--before-prompt-id")?);
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
    Ok(Command::Remote(RemoteCommand::PromptsList {
        conversation_id,
        before_created_at_ms,
        before_prompt_id,
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
        .filter(|value| i64::try_from(*value).is_ok())
        .ok_or_else(|| format!("invalid --expected-version '{version}'\n\n{}", usage()))?;
    if tokens.is_empty() {
        return Err(format!("prompt content is required\n\n{}", usage()));
    }
    let content = tokens.drain(..).collect::<Vec<_>>().join(" ");
    Ok(Command::Remote(RemoteCommand::PromptsAdd {
        conversation_id,
        expected_version,
        content,
    }))
}

#[cfg(test)]
#[path = "remote_args_tests.rs"]
mod tests;
