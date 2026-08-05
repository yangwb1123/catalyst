use std::collections::VecDeque;

use crate::args::group_graph_args::{
    duplicate, parse_limit, required_id, run_command, unknown, with_usage,
};
use crate::args::{Command, GroupGraphRunCommand, GroupGraphRunScheduleCommand, next_value};

pub(crate) fn parse_schedule(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("admit") => parse_admit(tokens, idempotency_key),
        Some("show") => parse_show(tokens),
        Some("list") => parse_list(tokens),
        Some(value) => Err(unknown("group graph run schedule", value)),
        None => Err(with_usage("group graph run schedule command is required")),
    }
}

fn parse_admit(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, "group graph run schedule admit", "GRAPH_RUN_ID")?;
    let mut schedule_source = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--schedule" if schedule_source.is_none() => {
                schedule_source = Some(next_value(tokens, "--schedule")?);
            }
            "--schedule" => return Err(duplicate("--schedule")),
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown("group graph run schedule admit", &option)),
        }
    }
    let schedule_source = schedule_source
        .ok_or_else(|| with_usage("group graph run schedule admit requires --schedule FILE|-"))?;
    Ok(run_command(GroupGraphRunCommand::Schedule(
        GroupGraphRunScheduleCommand::Admit {
            graph_run_id,
            schedule_source,
        },
    )))
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let schedule_id = required_id(tokens, "group graph run schedule show", "SCHEDULE_ID")?;
    let include_schedule = match tokens.pop_front().as_deref() {
        Some("--include-schedule") => true,
        Some(option) => return Err(unknown("group graph run schedule show", option)),
        None => false,
    };
    crate::args::require_empty(tokens)?;
    Ok(run_command(GroupGraphRunCommand::Schedule(
        GroupGraphRunScheduleCommand::Show {
            schedule_id,
            include_schedule,
        },
    )))
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    crate::args::require_empty(tokens)?;
    Ok(run_command(GroupGraphRunCommand::Schedule(
        GroupGraphRunScheduleCommand::List {
            graph_run_id,
            limit,
        },
    )))
}
use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    sync::Arc,
};

use forge_runtime_application::{
    AdmitGroupAgentGraphExecutionScheduleInput, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleService, MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::Args,
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::schedule_output::GroupAgentGraphExecutionScheduleCliOutput;

pub fn execute(
    args: &Args,
    command: &GroupGraphRunScheduleCommand,
) -> Result<GroupAgentGraphExecutionScheduleCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunScheduleCommand::Admit {
            graph_run_id,
            schedule_source,
        } => admit(args, graph_run_id, schedule_source),
        GroupGraphRunScheduleCommand::Show {
            schedule_id,
            include_schedule,
        } => Ok(GroupAgentGraphExecutionScheduleCliOutput::schedule(
            service(args)?.inspect(schedule_id)?,
            *include_schedule,
        )),
        GroupGraphRunScheduleCommand::List {
            graph_run_id,
            limit,
        } => Ok(GroupAgentGraphExecutionScheduleCliOutput::list(
            service(args)?.list(graph_run_id.as_deref(), *limit)?,
        )),
    }
}

fn admit(
    args: &Args,
    graph_run_id: &str,
    schedule_source: &str,
) -> Result<GroupAgentGraphExecutionScheduleCliOutput, Box<dyn Error>> {
    let schedule_json = read_schedule(schedule_source)?;
    let result = service(args)?.admit(&AdmitGroupAgentGraphExecutionScheduleInput {
        graph_run_id: graph_run_id.into(),
        schedule_json,
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-graph-execution-schedule")),
        admitted_at_ms: unix_time_millis(),
    })?;
    Ok(GroupAgentGraphExecutionScheduleCliOutput::admitted(
        result.disposition,
        result.inspection,
        schedule_source != "-",
    ))
}

fn service(args: &Args) -> Result<GroupAgentGraphExecutionScheduleService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentGraphExecutionScheduleService::new(
        store.clone(),
        store.clone(),
        store,
    ))
}

fn read_schedule(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(io::stdin().lock())?
    } else {
        read_bounded(File::open(source)?)?
    };
    GroupAgentGraphExecutionSchedule::decode_exact_bytes(&bytes)
        .map_err(|_| invalid_input("invalid or noncanonical Graph Execution Schedule"))?;
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Graph Execution Schedule must be UTF-8").into())
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let limit = MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES
        .checked_add(1)
        .expect("schedule bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("schedule bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES {
        return Err(invalid_input(
            "Graph Execution Schedule exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use crate::args::{
        Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand,
        GroupGraphRunScheduleCommand, parse_tokens,
    };

    fn parse(args: &[&str]) -> Result<crate::args::Args, String> {
        parse_tokens(args.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn parses_schedule_admit_with_local_key() {
        let args = parse(&[
            "group",
            "graph",
            "run",
            "schedule",
            "admit",
            "run-1",
            "--schedule",
            "schedule.json",
            "--idempotency-key",
            "schedule-key",
        ])
        .expect("schedule admission parses");
        assert_eq!(args.idempotency_key.as_deref(), Some("schedule-key"));
        assert!(matches!(
            args.command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::Schedule(GroupGraphRunScheduleCommand::Admit {
                    graph_run_id,
                    schedule_source,
                })
            ))) if graph_run_id == "run-1" && schedule_source == "schedule.json"
        ));
    }

    #[test]
    fn parses_redacted_show_and_filtered_list() {
        assert!(matches!(
            parse(&["group", "graph", "run", "schedule", "show", "schedule-1"])
                .expect("show parses")
                .command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::Schedule(GroupGraphRunScheduleCommand::Show {
                    schedule_id,
                    include_schedule: false,
                })
            ))) if schedule_id == "schedule-1"
        ));
        assert!(matches!(
            parse(&[
                "group", "graph", "run", "schedule", "list", "run-1", "--limit", "7"
            ])
            .expect("list parses")
            .command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::Schedule(GroupGraphRunScheduleCommand::List {
                    graph_run_id: Some(graph_run_id),
                    limit: 7,
                })
            ))) if graph_run_id == "run-1"
        ));
    }

    #[test]
    fn rejects_missing_artifact_duplicate_flags_and_read_only_key() {
        assert!(
            parse(&["group", "graph", "run", "schedule", "admit", "run-1"])
                .expect_err("missing schedule rejects")
                .contains("requires --schedule")
        );
        assert!(
            parse(&[
                "group",
                "graph",
                "run",
                "schedule",
                "show",
                "schedule-1",
                "--include-schedule",
                "--include-schedule"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "--idempotency-key",
                "key",
                "group",
                "graph",
                "run",
                "schedule",
                "show",
                "schedule-1"
            ])
            .expect_err("read-only command rejects mutation key")
            .contains("only valid for mutating commands")
        );
    }
}
