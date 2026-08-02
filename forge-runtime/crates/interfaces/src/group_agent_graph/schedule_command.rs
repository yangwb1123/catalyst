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
    args::{Args, GroupGraphRunScheduleCommand},
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
