use std::{error::Error, sync::Arc};

use forge_runtime_application::{
    GroupAgentNodeDispatchRequestService, PrepareGroupAgentNodeDispatchRequestInput,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupGraphRunDispatchCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::dispatch_output::GroupAgentNodeDispatchRequestCliOutput;

pub fn execute(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
) -> Result<GroupAgentNodeDispatchRequestCliOutput, Box<dyn Error>> {
    let service = service(args)?;
    match command {
        GroupGraphRunDispatchCommand::Prepare { graph_run_id } => {
            let result = service.prepare(&PrepareGroupAgentNodeDispatchRequestInput {
                graph_run_id: graph_run_id.clone(),
                idempotency_key: args
                    .idempotency_key
                    .clone()
                    .unwrap_or_else(|| idempotency_key("group-agent-node-dispatch-request")),
                prepared_at_ms: unix_time_millis(),
            })?;
            Ok(GroupAgentNodeDispatchRequestCliOutput::prepared(
                result.disposition,
                result.inspection,
            )?)
        }
        GroupGraphRunDispatchCommand::Show {
            dispatch_request_id,
            include_request,
        } => Ok(GroupAgentNodeDispatchRequestCliOutput::request(
            service.inspect(dispatch_request_id)?,
            *include_request,
        )?),
        GroupGraphRunDispatchCommand::List {
            graph_run_id,
            limit,
        } => Ok(GroupAgentNodeDispatchRequestCliOutput::list(
            service.list(graph_run_id.as_deref(), *limit)?,
        )),
    }
}

fn service(args: &Args) -> Result<GroupAgentNodeDispatchRequestService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentNodeDispatchRequestService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
    ))
}
