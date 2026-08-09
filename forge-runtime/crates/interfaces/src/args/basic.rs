use std::collections::VecDeque;

use super::{Command, next_value, parse_optional_id_and_limit, require_empty, usage};

#[derive(Debug, Eq, PartialEq)]
pub enum SessionCommand {
    List,
    New { title: String },
}

#[derive(Debug, Eq, PartialEq)]
pub enum PromptCommand {
    Add {
        conversation_id: String,
        text: String,
    },
    List {
        conversation_id: Option<String>,
        limit: usize,
    },
}

pub(super) fn parse_session(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => {
            require_empty(tokens)?;
            Ok(Command::Session(SessionCommand::List))
        }
        Some("new") => Ok(Command::Session(SessionCommand::New {
            title: parse_optional_title(tokens)?,
        })),
        Some(value) => Err(format!("unknown session command '{value}'\n\n{}", usage())),
        None => Err(format!("session command is required\n\n{}", usage())),
    }
}

pub(super) fn parse_prompt(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("add") => {
            let conversation_id = next_value(tokens, "prompt add")?;
            let text = required_text(tokens, "prompt text")?;
            Ok(Command::Prompt(PromptCommand::Add {
                conversation_id,
                text,
            }))
        }
        Some("list") => parse_prompt_list(tokens),
        Some(value) => Err(format!("unknown prompt command '{value}'\n\n{}", usage())),
        None => Err(format!("prompt command is required\n\n{}", usage())),
    }
}

fn parse_optional_title(tokens: &mut VecDeque<String>) -> Result<String, String> {
    if tokens.front().is_some_and(|value| value == "--title") {
        tokens.pop_front();
        let title = next_value(tokens, "--title")?;
        require_empty(tokens)?;
        return Ok(title);
    }
    if tokens.is_empty() {
        return Ok("New conversation".into());
    }
    Ok(drain_text(tokens))
}

fn parse_prompt_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let (conversation_id, limit) = parse_optional_id_and_limit(tokens)?;
    Ok(Command::Prompt(PromptCommand::List {
        conversation_id,
        limit,
    }))
}

fn required_text(tokens: &mut VecDeque<String>, field: &str) -> Result<String, String> {
    if tokens.is_empty() {
        return Err(format!("{field} is required\n\n{}", usage()));
    }
    Ok(drain_text(tokens))
}

fn drain_text(tokens: &mut VecDeque<String>) -> String {
    tokens.drain(..).collect::<Vec<_>>().join(" ")
}
