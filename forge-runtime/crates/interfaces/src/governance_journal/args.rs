use std::{collections::VecDeque, path::PathBuf};

use crate::runtime_domain::{GovernanceRecordKind, MAX_GOVERNANCE_RECORD_LIST_LIMIT};

use super::{Command, next_value, require_empty, usage};

#[derive(Debug, Eq, PartialEq)]
pub enum GovernanceCommand {
    Journal(GovernanceJournalCommand),
}

#[derive(Debug, Eq, PartialEq)]
pub enum GovernanceJournalCommand {
    Append {
        file: PathBuf,
    },
    Show {
        record_id: String,
        include_record: bool,
    },
    List {
        record_kind: Option<GovernanceRecordKind>,
        aggregate_id: Option<String>,
        limit: usize,
        include_record: bool,
    },
    Head {
        record_kind: GovernanceRecordKind,
        aggregate_id: String,
    },
}

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("journal") => Ok(Command::Governance(GovernanceCommand::Journal(
            parse_journal(tokens)?,
        ))),
        Some(value) => Err(format!(
            "unknown governance command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("governance command is required\n\n{}", usage())),
    }
}

fn parse_journal(tokens: &mut VecDeque<String>) -> Result<GovernanceJournalCommand, String> {
    match tokens.pop_front().as_deref() {
        Some("append") => parse_append(tokens),
        Some("show") => parse_show(tokens),
        Some("list") => parse_list(tokens),
        Some("head") => parse_head(tokens),
        Some(value) => Err(format!(
            "unknown governance journal command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "governance journal command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_append(tokens: &mut VecDeque<String>) -> Result<GovernanceJournalCommand, String> {
    if tokens.pop_front().as_deref() != Some("--file") {
        return Err(format!(
            "governance journal append requires --file PATH|-\n\n{}",
            usage()
        ));
    }
    let file = PathBuf::from(next_value(tokens, "--file")?);
    require_empty(tokens)?;
    Ok(GovernanceJournalCommand::Append { file })
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<GovernanceJournalCommand, String> {
    let record_id = next_value(tokens, "governance journal show")?;
    let include_record = take_flag(tokens, "--include-record");
    require_empty(tokens)?;
    Ok(GovernanceJournalCommand::Show {
        record_id,
        include_record,
    })
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<GovernanceJournalCommand, String> {
    let mut record_kind = None;
    let mut aggregate_id = None;
    let mut limit = 50;
    let mut limit_seen = false;
    let mut include_record = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--kind" if record_kind.is_none() => {
                record_kind = Some(parse_kind(&next_value(tokens, "--kind")?)?);
            }
            "--aggregate-id" if aggregate_id.is_none() => {
                aggregate_id = Some(next_value(tokens, "--aggregate-id")?);
            }
            "--limit" if !limit_seen => {
                limit = parse_limit(&next_value(tokens, "--limit")?)?;
                limit_seen = true;
            }
            "--limit" => return Err(format!("duplicate --limit\n\n{}", usage())),
            "--include-record" if !include_record => include_record = true,
            _ => return Err(format!("unexpected argument '{option}'\n\n{}", usage())),
        }
    }
    Ok(GovernanceJournalCommand::List {
        record_kind,
        aggregate_id,
        limit,
        include_record,
    })
}

fn parse_head(tokens: &mut VecDeque<String>) -> Result<GovernanceJournalCommand, String> {
    let record_kind = parse_kind(&next_value(tokens, "governance journal head KIND")?)?;
    let aggregate_id = next_value(tokens, "governance journal head AGGREGATE_ID")?;
    require_empty(tokens)?;
    Ok(GovernanceJournalCommand::Head {
        record_kind,
        aggregate_id,
    })
}

fn parse_kind(value: &str) -> Result<GovernanceRecordKind, String> {
    match value {
        "EvidenceRecord" => Ok(GovernanceRecordKind::EvidenceRecord),
        "KnowledgeClaim" => Ok(GovernanceRecordKind::KnowledgeClaim),
        _ => Err(format!(
            "invalid governance record kind '{value}'; expected EvidenceRecord or KnowledgeClaim"
        )),
    }
}

fn parse_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| format!("invalid --limit '{value}'"))?;
    if !(1..=MAX_GOVERNANCE_RECORD_LIST_LIMIT).contains(&limit) {
        return Err(format!(
            "--limit must be between 1 and {MAX_GOVERNANCE_RECORD_LIST_LIMIT}"
        ));
    }
    Ok(limit)
}

fn take_flag(tokens: &mut VecDeque<String>, flag: &str) -> bool {
    if tokens.front().is_some_and(|value| value == flag) {
        tokens.pop_front();
        return true;
    }
    false
}
