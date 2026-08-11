use std::{collections::VecDeque, path::PathBuf};

use crate::runtime_domain::{
    GovernanceRecordKind, MAX_GOVERNANCE_RECORD_LIST_LIMIT, MAX_GOVERNANCE_SEMANTIC_LIST_LIMIT,
};

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
    View {
        record_kind: GovernanceRecordKind,
        aggregate_id: String,
        as_of_unix_ms: i64,
    },
    Conflicts {
        as_of_unix_ms: i64,
        limit: usize,
    },
    ValidationJobs {
        as_of_unix_ms: i64,
        due_only: bool,
        limit: usize,
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
        Some("view") => parse_view(tokens),
        Some("conflicts") => parse_conflicts(tokens),
        Some("validation-jobs") => parse_validation_jobs(tokens),
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

fn parse_view(tokens: &mut VecDeque<String>) -> Result<GovernanceJournalCommand, String> {
    let record_kind = parse_kind(&next_value(tokens, "governance journal view KIND")?)?;
    let aggregate_id = next_value(tokens, "governance journal view AGGREGATE_ID")?;
    let as_of_unix_ms = parse_required_as_of(tokens)?;
    Ok(GovernanceJournalCommand::View {
        record_kind,
        aggregate_id,
        as_of_unix_ms,
    })
}

fn parse_conflicts(tokens: &mut VecDeque<String>) -> Result<GovernanceJournalCommand, String> {
    let mut as_of_unix_ms = None;
    let mut limit = 50;
    let mut limit_seen = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--as-of-unix-ms" if as_of_unix_ms.is_none() => {
                as_of_unix_ms = Some(parse_time(&next_value(tokens, "--as-of-unix-ms")?)?);
            }
            "--as-of-unix-ms" => {
                return Err(format!("duplicate --as-of-unix-ms\n\n{}", usage()));
            }
            "--limit" if !limit_seen => {
                limit = parse_semantic_limit(&next_value(tokens, "--limit")?)?;
                limit_seen = true;
            }
            "--limit" => return Err(format!("duplicate --limit\n\n{}", usage())),
            _ => return Err(format!("unexpected argument '{option}'\n\n{}", usage())),
        }
    }
    let as_of_unix_ms = require_as_of(as_of_unix_ms)?;
    Ok(GovernanceJournalCommand::Conflicts {
        as_of_unix_ms,
        limit,
    })
}

fn parse_validation_jobs(
    tokens: &mut VecDeque<String>,
) -> Result<GovernanceJournalCommand, String> {
    let mut as_of_unix_ms = None;
    let mut due_only = false;
    let mut limit = 50;
    let mut limit_seen = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--as-of-unix-ms" if as_of_unix_ms.is_none() => {
                as_of_unix_ms = Some(parse_time(&next_value(tokens, "--as-of-unix-ms")?)?);
            }
            "--as-of-unix-ms" => {
                return Err(format!("duplicate --as-of-unix-ms\n\n{}", usage()));
            }
            "--due-only" if !due_only => due_only = true,
            "--due-only" => return Err(format!("duplicate --due-only\n\n{}", usage())),
            "--limit" if !limit_seen => {
                limit = parse_semantic_limit(&next_value(tokens, "--limit")?)?;
                limit_seen = true;
            }
            "--limit" => return Err(format!("duplicate --limit\n\n{}", usage())),
            _ => return Err(format!("unexpected argument '{option}'\n\n{}", usage())),
        }
    }
    let as_of_unix_ms = require_as_of(as_of_unix_ms)?;
    Ok(GovernanceJournalCommand::ValidationJobs {
        as_of_unix_ms,
        due_only,
        limit,
    })
}

fn parse_required_as_of(tokens: &mut VecDeque<String>) -> Result<i64, String> {
    if tokens.pop_front().as_deref() != Some("--as-of-unix-ms") {
        return Err(format!(
            "semantic view commands require --as-of-unix-ms N\n\n{}",
            usage()
        ));
    }
    let value = parse_time(&next_value(tokens, "--as-of-unix-ms")?)?;
    require_empty(tokens)?;
    Ok(value)
}

fn require_as_of(value: Option<i64>) -> Result<i64, String> {
    value.ok_or_else(|| {
        format!(
            "semantic view commands require --as-of-unix-ms N\n\n{}",
            usage()
        )
    })
}

fn parse_time(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
        .ok_or_else(|| format!("invalid --as-of-unix-ms '{value}'"))
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

fn parse_semantic_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| format!("invalid --limit '{value}'"))?;
    if !(1..=MAX_GOVERNANCE_SEMANTIC_LIST_LIMIT).contains(&limit) {
        return Err(format!(
            "--limit must be between 1 and {MAX_GOVERNANCE_SEMANTIC_LIST_LIMIT}"
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
