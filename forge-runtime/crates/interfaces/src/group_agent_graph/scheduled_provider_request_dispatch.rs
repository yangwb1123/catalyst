use std::{error::Error, path::PathBuf, sync::Arc};

use forge_runtime_application::{
    ExecuteGroupAgentScheduledNodeDispatchInput, ExecuteGroupAgentScheduledNodeDispatchResult,
    GroupAgentScheduledNodeDispatchExecutionService,
    GroupAgentScheduledNodeDispatchReadinessService,
};
use forge_runtime_infrastructure::{
    PinnedScheduledCoreTerminalBridge, RegisteredGroupAgentNodeProviderFactory, SqliteHubStore,
};

use crate::{
    args::{Args, GroupGraphRunScheduledContractProviderRequestCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    runtime_domain::{Cancellation, GroupAgentScheduledNodeLifecycleStore},
    state_path::hub_database_path,
};

use super::{
    GroupAgentScheduledNodeProviderRequestCommandCliOutput, read_authorization, read_pricing,
    scheduled_provider_request_output::GroupAgentScheduledNodeDispatchExecutionCliOutput,
};

pub(super) struct DispatchInputs {
    pub(super) authorization_json: Option<String>,
    pub(super) pricing_json: Option<String>,
}

impl DispatchInputs {
    pub(super) fn authorization(&self) -> &str {
        self.authorization_json
            .as_deref()
            .expect("scheduled authorization was read before execution")
    }

    pub(super) fn pricing(&self) -> &str {
        self.pricing_json
            .as_deref()
            .expect("scheduled pricing was read before execution")
    }
}

pub(super) fn inspect_existing_execution(
    args: &Args,
    command: &GroupGraphRunScheduledContractProviderRequestCommand,
) -> Result<Option<GroupAgentScheduledNodeProviderRequestCommandCliOutput>, Box<dyn Error>> {
    let GroupGraphRunScheduledContractProviderRequestCommand::Execute {
        provider_request_id,
        include_result,
        ..
    } = command
    else {
        return Ok(None);
    };
    let database = hub_database_path(args.state_dir.as_deref())?;
    if !database.try_exists()? {
        return Ok(None);
    }
    let store = SqliteHubStore::open_existing_current_read_only(database)?;
    let inspection = match store.inspect_group_agent_scheduled_node_lifecycle(provider_request_id) {
        Ok(value) => value,
        Err(crate::runtime_domain::HubStoreError::NotFound { .. }) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let output = GroupAgentScheduledNodeDispatchExecutionCliOutput::from_result(
        ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(inspection),
        *include_result,
    );
    Ok(Some(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(Box::new(output)),
    ))
}

pub(super) fn read_inputs(
    command: &GroupGraphRunScheduledContractProviderRequestCommand,
) -> Result<DispatchInputs, Box<dyn Error>> {
    match command {
        GroupGraphRunScheduledContractProviderRequestCommand::Execute {
            authorization_source,
            pricing_source,
            ..
        } => Ok(DispatchInputs {
            authorization_json: Some(read_authorization(authorization_source)?),
            pricing_json: Some(read_pricing(pricing_source)?),
        }),
        _ => Ok(DispatchInputs {
            authorization_json: None,
            pricing_json: None,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_dispatch(
    args: &Args,
    provider_request_id: &str,
    inputs: &DispatchInputs,
    core_bin: &str,
    core_bin_sha256: &str,
    confirm_off_machine: bool,
    confirm_predecessor_content: bool,
    include_result: bool,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    validate_execute_preflight(
        args,
        provider_request_id,
        inputs.authorization(),
        inputs.pricing(),
        confirm_off_machine,
    )?;
    let bridge = Arc::new(PinnedScheduledCoreTerminalBridge::new(
        PathBuf::from(core_bin),
        core_bin_sha256.into(),
    )?);
    let service = execution_service(args, bridge)?;
    let result = Box::pin(
        service.execute(&ExecuteGroupAgentScheduledNodeDispatchInput {
            provider_request_id: provider_request_id.into(),
            authorization_json: inputs
                .authorization_json
                .clone()
                .expect("scheduled authorization was read before execution"),
            pricing_json: inputs
                .pricing_json
                .clone()
                .expect("scheduled pricing was read before execution"),
            confirm_off_machine,
            confirm_predecessor_content,
            cancellation: Cancellation::default(),
        }),
    )
    .await?;
    Ok(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(Box::new(
            GroupAgentScheduledNodeDispatchExecutionCliOutput::from_result(result, include_result),
        )),
    )
}

fn execution_service(
    args: &Args,
    bridge: Arc<PinnedScheduledCoreTerminalBridge>,
) -> Result<GroupAgentScheduledNodeDispatchExecutionService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let providers = Arc::new(RegisteredGroupAgentNodeProviderFactory::new());
    Ok(GroupAgentScheduledNodeDispatchExecutionService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        Arc::new(OpenAiRequestCodec),
        store,
        providers,
        Arc::new(crate::group_agent_graph::dispatch_execution_adapters::EnvironmentOpenAiCredentialSource),
        bridge,
        Arc::new(crate::group_agent_graph::dispatch_execution_adapters::SystemDispatchMetadataSource),
    ))
}

fn validate_execute_preflight(
    args: &Args,
    provider_request_id: &str,
    authorization_json: &str,
    pricing_json: &str,
    confirm_off_machine: bool,
) -> Result<(), Box<dyn Error>> {
    if !confirm_off_machine {
        return Err(forge_runtime_application::GroupAgentScheduledNodeDispatchExecutionServiceError::ConsentRequired.into());
    }
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    GroupAgentScheduledNodeDispatchReadinessService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        Arc::new(OpenAiRequestCodec),
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
    )
    .verify(provider_request_id, authorization_json, pricing_json)?;
    Ok(())
}
