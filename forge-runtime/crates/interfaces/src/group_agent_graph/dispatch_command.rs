use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
    sync::Arc,
};

use crate::runtime_domain::Cancellation;
use forge_runtime_application::{
    ExecuteGroupAgentNodeDispatchInput, GroupAgentNodeDispatchExecutionService,
    GroupAgentNodeDispatchExecutionServiceError, GroupAgentNodeDispatchReadinessService,
    GroupAgentNodeDispatchReleaseControlService, GroupAgentNodeDispatchRequestService,
    MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES, PrepareGroupAgentNodeDispatchRequestInput,
    validate_group_agent_node_dispatch_topology,
};
use forge_runtime_infrastructure::{
    PinnedCoreTerminalBridge, RegisteredGroupAgentNodeProviderFactory, SqliteHubStore,
};

use crate::{
    args::{Args, GroupGraphRunDispatchCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::{
    dispatch_authorization_output::{self, GroupAgentNodeDispatchAuthorizationCliOutput},
    dispatch_execution_adapters::{PreparedDispatchDependencies, SystemDispatchMetadataSource},
    dispatch_execution_output::{self, GroupAgentNodeDispatchExecutionCliOutput},
    dispatch_output::{self, GroupAgentNodeDispatchRequestCliOutput},
    dispatch_readiness_output::{self, GroupAgentNodeDispatchReadinessCliOutput},
};

pub enum GroupAgentGraphRunDispatchCommandCliOutput {
    Request(Box<GroupAgentNodeDispatchRequestCliOutput>),
    ReleaseControl(String),
    Authorization(Box<GroupAgentNodeDispatchAuthorizationCliOutput>),
    Readiness(Box<GroupAgentNodeDispatchReadinessCliOutput>),
    Execution(Box<GroupAgentNodeDispatchExecutionCliOutput>),
}

struct DispatchInputs {
    authorization_json: Option<String>,
    pricing_json: Option<String>,
}

impl DispatchInputs {
    fn authorization(&self) -> &str {
        self.authorization_json
            .as_deref()
            .expect("authorization was read before service construction")
    }

    fn pricing(&self) -> &str {
        self.pricing_json
            .as_deref()
            .expect("pricing was read before service construction")
    }
}

pub async fn execute(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    if let Some(existing) = inspect_existing_execution(args, command)? {
        return Ok(existing);
    }
    let inputs = read_inputs(command)?;
    execute_with_inputs(args, command, &inputs).await
}

fn inspect_existing_execution(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
) -> Result<Option<GroupAgentGraphRunDispatchCommandCliOutput>, Box<dyn Error>> {
    let GroupGraphRunDispatchCommand::Execute {
        graph_run_id,
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
    let store = SqliteHubStore::open_existing_dispatch_inspection_read_only(database)?;
    let Some(inspection) = store.inspect_existing_group_agent_node_lifecycle(graph_run_id)? else {
        return Ok(None);
    };
    let output = GroupAgentNodeDispatchExecutionCliOutput::from_result(
        forge_runtime_application::ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(inspection),
        *include_result,
    );
    Ok(Some(GroupAgentGraphRunDispatchCommandCliOutput::Execution(
        Box::new(output),
    )))
}

fn read_inputs(command: &GroupGraphRunDispatchCommand) -> Result<DispatchInputs, Box<dyn Error>> {
    let authorization_json = match command {
        GroupGraphRunDispatchCommand::AuthorizationVerify {
            authorization_source,
            ..
        }
        | GroupGraphRunDispatchCommand::ReadinessVerify {
            authorization_source,
            ..
        }
        | GroupGraphRunDispatchCommand::Execute {
            authorization_source,
            ..
        } => Some(read_authorization(authorization_source)?),
        _ => None,
    };
    let pricing_json = match command {
        GroupGraphRunDispatchCommand::ReadinessVerify { pricing_source, .. }
        | GroupGraphRunDispatchCommand::Execute { pricing_source, .. } => {
            Some(read_pricing(pricing_source)?)
        }
        _ => None,
    };
    Ok(DispatchInputs {
        authorization_json,
        pricing_json,
    })
}

async fn execute_with_inputs(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
    inputs: &DispatchInputs,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunDispatchCommand::Execute {
            graph_run_id,
            core_bin,
            core_bin_sha256,
            confirm_off_machine,
            include_result,
            ..
        } => {
            execute_dispatch(
                args,
                graph_run_id,
                inputs,
                core_bin,
                core_bin_sha256,
                *confirm_off_machine,
                *include_result,
            )
            .await
        }
        _ => execute_effect_free(args, command, inputs),
    }
}

fn execute_effect_free(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
    inputs: &DispatchInputs,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunDispatchCommand::Prepare { graph_run_id } => {
            execute_prepare(args, graph_run_id)
        }
        GroupGraphRunDispatchCommand::Show {
            dispatch_request_id,
            include_request,
        } => Ok(request_output(
            GroupAgentNodeDispatchRequestCliOutput::request(
                request_service(args)?.inspect(dispatch_request_id)?,
                *include_request,
            )?,
        )),
        GroupGraphRunDispatchCommand::List {
            graph_run_id,
            limit,
        } => Ok(request_output(
            GroupAgentNodeDispatchRequestCliOutput::list(
                request_service(args)?.list(graph_run_id.as_deref(), *limit)?,
            ),
        )),
        GroupGraphRunDispatchCommand::ReleaseControlExport { graph_run_id } => {
            export_release_control(args, graph_run_id)
        }
        GroupGraphRunDispatchCommand::AuthorizationVerify { graph_run_id, .. } => {
            verify_authorization(args, graph_run_id, inputs.authorization())
        }
        GroupGraphRunDispatchCommand::ReadinessVerify { graph_run_id, .. } => {
            verify_readiness(args, graph_run_id, inputs.authorization(), inputs.pricing())
        }
        GroupGraphRunDispatchCommand::Execute { .. } => {
            unreachable!("handled before local routing")
        }
    }
}

fn execute_prepare(
    args: &Args,
    graph_run_id: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let result = request_service(args)?.prepare(&PrepareGroupAgentNodeDispatchRequestInput {
        graph_run_id: graph_run_id.into(),
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-node-dispatch-request")),
        prepared_at_ms: unix_time_millis(),
    })?;
    Ok(request_output(
        GroupAgentNodeDispatchRequestCliOutput::prepared(result.disposition, result.inspection)?,
    ))
}

fn export_release_control(
    args: &Args,
    graph_run_id: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let exported = release_service(args)?.export(graph_run_id)?;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::ReleaseControl(
        exported.canonical_json,
    ))
}

fn verify_authorization(
    args: &Args,
    graph_run_id: &str,
    authorization_json: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let verified = release_service(args)?.verify(graph_run_id, authorization_json)?;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::Authorization(
        Box::new(GroupAgentNodeDispatchAuthorizationCliOutput::verified(
            verified,
        )),
    ))
}

pub fn write_output(
    output: &GroupAgentGraphRunDispatchCommandCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    match output {
        GroupAgentGraphRunDispatchCommandCliOutput::Request(output) => {
            dispatch_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunDispatchCommandCliOutput::ReleaseControl(canonical_json) => {
            writer.write_all(canonical_json.as_bytes())
        }
        GroupAgentGraphRunDispatchCommandCliOutput::Authorization(output) => {
            dispatch_authorization_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunDispatchCommandCliOutput::Readiness(output) => {
            dispatch_readiness_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunDispatchCommandCliOutput::Execution(output) => {
            dispatch_execution_output::write_output(output, json, writer)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_dispatch(
    args: &Args,
    graph_run_id: &str,
    inputs: &DispatchInputs,
    core_bin: &str,
    core_bin_sha256: &str,
    confirm_off_machine: bool,
    include_result: bool,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    validate_execute_preflight(
        args,
        graph_run_id,
        inputs.authorization(),
        inputs.pricing(),
        confirm_off_machine,
    )?;
    let bridge = Arc::new(PinnedCoreTerminalBridge::new(
        PathBuf::from(core_bin),
        core_bin_sha256.into(),
    )?);
    let dependencies =
        PreparedDispatchDependencies::prepare(inputs.authorization(), inputs.pricing())?;
    let service = execution_service(args, bridge, dependencies)?;
    let result = service
        .execute(&ExecuteGroupAgentNodeDispatchInput {
            graph_run_id: graph_run_id.into(),
            authorization_json: inputs
                .authorization_json
                .clone()
                .expect("execute authorization was read before bridge preflight"),
            pricing_json: inputs
                .pricing_json
                .clone()
                .expect("execute pricing was read before bridge preflight"),
            confirm_off_machine,
            cancellation: Cancellation::default(),
        })
        .await?;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::Execution(
        Box::new(GroupAgentNodeDispatchExecutionCliOutput::from_result(
            result,
            include_result,
        )),
    ))
}

fn execution_service(
    args: &Args,
    bridge: Arc<PinnedCoreTerminalBridge>,
    dependencies: PreparedDispatchDependencies,
) -> Result<GroupAgentNodeDispatchExecutionService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentNodeDispatchExecutionService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
        dependencies.providers,
        dependencies.credentials,
        bridge,
        Arc::new(SystemDispatchMetadataSource),
    ))
}

fn validate_execute_preflight(
    args: &Args,
    graph_run_id: &str,
    authorization_json: &str,
    pricing_json: &str,
    confirm_off_machine: bool,
) -> Result<(), Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_dispatch_preflight_read_only(
        database,
    )?);
    let service = GroupAgentNodeDispatchReleaseControlService::new(
        store.clone(),
        store.clone(),
        Arc::new(OpenAiRequestCodec),
    );
    let exported = service.export(graph_run_id)?;
    validate_group_agent_node_dispatch_topology(&exported.release_control)?;
    if !confirm_off_machine {
        return Err(GroupAgentNodeDispatchExecutionServiceError::ConsentRequired.into());
    }
    GroupAgentNodeDispatchReadinessService::new(
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
    )
    .verify(graph_run_id, authorization_json, pricing_json)?;
    Ok(())
}

fn request_output(
    output: GroupAgentNodeDispatchRequestCliOutput,
) -> GroupAgentGraphRunDispatchCommandCliOutput {
    GroupAgentGraphRunDispatchCommandCliOutput::Request(Box::new(output))
}

fn request_service(args: &Args) -> Result<GroupAgentNodeDispatchRequestService, Box<dyn Error>> {
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

fn release_service(
    args: &Args,
) -> Result<GroupAgentNodeDispatchReleaseControlService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    Ok(GroupAgentNodeDispatchReleaseControlService::new(
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
    ))
}

fn verify_readiness(
    args: &Args,
    graph_run_id: &str,
    authorization_json: &str,
    pricing_json: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let verified =
        readiness_service(args)?.verify(graph_run_id, authorization_json, pricing_json)?;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::Readiness(
        Box::new(GroupAgentNodeDispatchReadinessCliOutput::verified(verified)),
    ))
}

fn readiness_service(
    args: &Args,
) -> Result<GroupAgentNodeDispatchReadinessService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    Ok(GroupAgentNodeDispatchReadinessService::new(
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
    ))
}

fn read_authorization(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_authorization_bounded(io::stdin().lock())?
    } else {
        read_authorization_bounded(File::open(source)?)?
    };
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Node Dispatch Authorization must be UTF-8").into())
}

fn read_pricing(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_pricing_bounded(io::stdin().lock())?
    } else {
        read_pricing_bounded(File::open(source)?)?
    };
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Node pricing snapshot must be UTF-8").into())
}

fn read_authorization_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let limit = MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES
        .checked_add(1)
        .expect("Node Dispatch Authorization bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("Node Dispatch Authorization bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES {
        return Err(invalid_input(
            "Node Dispatch Authorization exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

fn read_pricing_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    read_bounded_artifact(
        reader,
        crate::runtime_domain::MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES,
        "Node pricing snapshot exceeds its byte limit",
    )
}

fn read_bounded_artifact(
    reader: impl Read,
    maximum: usize,
    message: &str,
) -> Result<Vec<u8>, io::Error> {
    let limit = maximum.checked_add(1).expect("artifact bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("artifact bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid_input(message));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
#[path = "dispatch_command_tests.rs"]
mod tests;
