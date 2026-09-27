use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

use super::{RemoteCommand, RemoteConversationScope, parse_instance_id};

pub(super) fn parse_sessions(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_session_list(tokens),
        Some("show") => parse_session_show(tokens),
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

fn parse_session_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote sessions show")?;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--instance" | "--instance-id" if instance_id.is_none() => {
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote sessions show option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote sessions show --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::SessionsShow {
        conversation_id,
        instance_id,
        instance_view,
    }))
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
    let mut instance_id = None;
    let mut instance_view = None;
    let mut all_pages = false;
    let mut all_pages_seen = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--after" if after_id.is_none() => after_id = Some(next_value(tokens, "--after")?),
            "--scope" if scope.is_none() => {
                scope = Some(parse_scope(&next_value(tokens, "--scope")?)?);
            }
            "--instance" | "--instance-id" if instance_id.is_none() => {
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
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
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote sessions list --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::SessionsList {
        after_id,
        scope,
        instance_id,
        instance_view,
        all_pages,
    }))
}

fn parse_session_create(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut title = None;
    let mut scope = None;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--title" if title.is_none() => title = Some(next_value(tokens, "--title")?),
            "--scope" if scope.is_none() => {
                scope = Some(parse_scope(&next_value(tokens, "--scope")?)?);
            }
            "--instance" | "--instance-id" if instance_id.is_none() => {
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
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
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote sessions create --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::SessionsCreate {
        title: title.unwrap_or_else(|| "New conversation".to_owned()),
        scope: scope.unwrap_or(RemoteConversationScope::Global),
        instance_id,
        instance_view,
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
