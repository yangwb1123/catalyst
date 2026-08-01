use std::collections::{HashSet, VecDeque};

use crate::runtime_domain::{
    MAX_GROUP_ANALYSIS_PANEL_ANALYSES, MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT,
    MIN_GROUP_ANALYSIS_PANEL_ANALYSES,
};

use super::{Command, GroupCommand, GroupPanelCommand, next_value, require_empty, usage};

pub(super) fn parse(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("prepare") => parse_prepare(tokens, idempotency_key),
        Some("show") => parse_show(tokens),
        Some("list") => parse_list(tokens),
        Some(value) => Err(with_usage(&format!(
            "unknown group panel command '{value}'"
        ))),
        None => Err(with_usage("group panel command is required")),
    }
}

fn parse_prepare(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let group_run_id = required_id(tokens, "group panel prepare", "GROUP_RUN_ID")?;
    let mut analysis_ids = Vec::new();
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--analysis" => analysis_ids.push(next_value(tokens, "--analysis")?),
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown("group panel prepare", &option)),
        }
    }
    validate_analysis_ids(&analysis_ids)?;
    Ok(Command::Group(GroupCommand::Panel(
        GroupPanelCommand::Prepare {
            group_run_id,
            analysis_ids,
        },
    )))
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let panel_id = required_id(tokens, "group panel show", "PANEL_ID")?;
    let include_results = match tokens.pop_front().as_deref() {
        Some("--include-results") => true,
        Some(option) => return Err(unknown("group panel show", option)),
        None => false,
    };
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Panel(
        GroupPanelCommand::Show {
            panel_id,
            include_results,
        },
    )))
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let group_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Panel(
        GroupPanelCommand::List {
            group_run_id,
            limit,
        },
    )))
}

fn validate_analysis_ids(ids: &[String]) -> Result<(), String> {
    if !(MIN_GROUP_ANALYSIS_PANEL_ANALYSES..=MAX_GROUP_ANALYSIS_PANEL_ANALYSES).contains(&ids.len())
    {
        return Err(with_usage(&format!(
            "group panel prepare requires between {MIN_GROUP_ANALYSIS_PANEL_ANALYSES} and \
             {MAX_GROUP_ANALYSIS_PANEL_ANALYSES} --analysis values"
        )));
    }
    let unique = ids.iter().collect::<HashSet<_>>();
    if unique.len() == ids.len() {
        Ok(())
    } else {
        Err(with_usage(
            "group panel prepare does not allow duplicate analysis IDs",
        ))
    }
}

fn parse_limit(tokens: &mut VecDeque<String>) -> Result<usize, String> {
    let value = next_value(tokens, "--limit")?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| with_usage(&format!("invalid --limit '{value}'")))?;
    if (1..=MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(with_usage(&format!(
            "--limit must be between 1 and {MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT}"
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

fn duplicate(option: &str) -> String {
    with_usage(&format!("{option} was specified more than once"))
}

fn unknown(operation: &str, option: &str) -> String {
    with_usage(&format!("unknown {operation} option '{option}'"))
}

fn with_usage(message: &str) -> String {
    format!("{message}\n\n{}", usage())
}
