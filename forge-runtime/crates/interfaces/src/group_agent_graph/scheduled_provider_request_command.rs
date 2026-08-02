use std::{error::Error, sync::Arc};

use forge_runtime_application::{
    GroupAgentScheduledNodeProviderRequestService,
    PrepareGroupAgentScheduledNodeProviderRequestInput,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupGraphRunScheduledContractProviderRequestCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::scheduled_provider_request_output::GroupAgentScheduledNodeProviderRequestCliOutput;

pub fn execute(
    args: &Args,
    command: &GroupGraphRunScheduledContractProviderRequestCommand,
) -> Result<GroupAgentScheduledNodeProviderRequestCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunScheduledContractProviderRequestCommand::Prepare {
            scheduled_contract_id,
        } => prepare(args, scheduled_contract_id),
        GroupGraphRunScheduledContractProviderRequestCommand::Show {
            provider_request_id,
            include_request,
        } => {
            GroupAgentScheduledNodeProviderRequestService::preflight_inspect(provider_request_id)?;
            GroupAgentScheduledNodeProviderRequestCliOutput::request(
                read_service(args)?.inspect(provider_request_id)?,
                *include_request,
            )
            .map_err(Into::into)
        }
        GroupGraphRunScheduledContractProviderRequestCommand::List {
            graph_run_id,
            limit,
        } => {
            GroupAgentScheduledNodeProviderRequestService::preflight_list(
                graph_run_id.as_deref(),
                *limit,
            )?;
            Ok(GroupAgentScheduledNodeProviderRequestCliOutput::list(
                read_service(args)?.list(graph_run_id.as_deref(), *limit)?,
            ))
        }
    }
}

fn prepare(
    args: &Args,
    scheduled_contract_id: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestCliOutput, Box<dyn Error>> {
    let input = PrepareGroupAgentScheduledNodeProviderRequestInput {
        scheduled_contract_id: scheduled_contract_id.into(),
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-scheduled-node-provider-request")),
        prepared_at_ms: unix_time_millis(),
    };
    GroupAgentScheduledNodeProviderRequestService::preflight_prepare(&input)?;
    let result = prepare_service(args)?.prepare(&input)?;
    GroupAgentScheduledNodeProviderRequestCliOutput::prepared(result.disposition, result.inspection)
        .map_err(Into::into)
}

fn prepare_service(
    args: &Args,
) -> Result<GroupAgentScheduledNodeProviderRequestService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(service(store))
}

fn read_service(
    args: &Args,
) -> Result<GroupAgentScheduledNodeProviderRequestService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    Ok(service(store))
}

fn service(store: Arc<SqliteHubStore>) -> GroupAgentScheduledNodeProviderRequestService {
    GroupAgentScheduledNodeProviderRequestService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
    )
}
