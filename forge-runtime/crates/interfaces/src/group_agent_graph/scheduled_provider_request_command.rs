use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    sync::Arc,
};

use forge_runtime_application::{
    GroupAgentScheduledNodeDispatchReadinessService,
    GroupAgentScheduledNodeDispatchReleaseControlService,
    GroupAgentScheduledNodeProviderRequestService,
    PrepareGroupAgentScheduledNodeProviderRequestInput,
};
use forge_runtime_infrastructure::{RegisteredGroupAgentNodeProviderFactory, SqliteHubStore};

use crate::{
    args::{Args, GroupGraphRunScheduledContractProviderRequestCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    runtime_domain::{
        GroupAgentNodePricingSnapshot, GroupAgentScheduledNodeDispatchAuthorization,
        MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES,
        MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES,
    },
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::{
    scheduled_provider_request_output::{
        self, GroupAgentScheduledNodeDispatchExecutionCliOutput,
        GroupAgentScheduledNodeProviderRequestCliOutput,
    },
    scheduled_release_output::{self, GroupAgentScheduledNodeDispatchAuthorizationCliOutput},
    scheduled_release_readiness_output::{
        self, GroupAgentScheduledNodeDispatchReadinessCliOutput, ScheduledReadinessMetadataView,
    },
};

#[path = "scheduled_provider_request_dispatch.rs"]
mod dispatch;
use dispatch::{execute_dispatch, inspect_existing_execution, read_inputs};

pub enum GroupAgentScheduledNodeProviderRequestCommandCliOutput {
    Request(Box<GroupAgentScheduledNodeProviderRequestCliOutput>),
    ReleaseControl(String),
    Authorization(Box<GroupAgentScheduledNodeDispatchAuthorizationCliOutput>),
    Readiness(Box<GroupAgentScheduledNodeDispatchReadinessCliOutput>),
    Execution(Box<GroupAgentScheduledNodeDispatchExecutionCliOutput>),
}

pub async fn execute(
    args: &Args,
    command: &GroupGraphRunScheduledContractProviderRequestCommand,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    if let Some(existing) = inspect_existing_execution(args, command)? {
        return Ok(existing);
    }
    let inputs = read_inputs(command)?;
    if let GroupGraphRunScheduledContractProviderRequestCommand::Execute {
        provider_request_id,
        core_bin,
        core_bin_sha256,
        confirm_off_machine,
        confirm_predecessor_content,
        include_result,
        ..
    } = command
    {
        return Box::pin(execute_dispatch(
            args,
            provider_request_id,
            &inputs,
            core_bin,
            core_bin_sha256,
            *confirm_off_machine,
            *confirm_predecessor_content,
            *include_result,
        ))
        .await;
    }
    run_sync_command(args, command)
}

/// Runs the non-dispatch (effect-free) commands, which never touch a provider,
/// credential, lane, or the network.
fn run_sync_command(
    args: &Args,
    command: &GroupGraphRunScheduledContractProviderRequestCommand,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunScheduledContractProviderRequestCommand::Prepare {
            scheduled_contract_id,
        } => prepare(args, scheduled_contract_id),
        GroupGraphRunScheduledContractProviderRequestCommand::Show {
            provider_request_id,
            include_request,
        } => show(args, provider_request_id, *include_request),
        GroupGraphRunScheduledContractProviderRequestCommand::List {
            graph_run_id,
            limit,
        } => list(args, graph_run_id.as_deref(), *limit),
        GroupGraphRunScheduledContractProviderRequestCommand::ReleaseControlExport {
            provider_request_id,
        } => export_release_control(args, provider_request_id),
        GroupGraphRunScheduledContractProviderRequestCommand::AuthorizationVerify {
            provider_request_id,
            authorization_source,
        } => verify_authorization(args, provider_request_id, authorization_source),
        GroupGraphRunScheduledContractProviderRequestCommand::ReadinessVerify {
            provider_request_id,
            authorization_source,
            pricing_source,
        } => verify_readiness(
            args,
            provider_request_id,
            authorization_source,
            pricing_source,
        ),
        GroupGraphRunScheduledContractProviderRequestCommand::Execute { .. } => {
            unreachable!("dispatch execute is routed before run_sync_command")
        }
    }
}

fn show(
    args: &Args,
    provider_request_id: &str,
    include_request: bool,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    GroupAgentScheduledNodeProviderRequestService::preflight_inspect(provider_request_id)?;
    let output = GroupAgentScheduledNodeProviderRequestCliOutput::request(
        read_service(args)?.inspect(provider_request_id)?,
        include_request,
    )?;
    Ok(request_output(output))
}

fn list(
    args: &Args,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    GroupAgentScheduledNodeProviderRequestService::preflight_list(graph_run_id, limit)?;
    let inspections = read_service(args)?.list(graph_run_id, limit)?;
    Ok(request_output(
        GroupAgentScheduledNodeProviderRequestCliOutput::list(inspections),
    ))
}

fn prepare(
    args: &Args,
    scheduled_contract_id: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
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
    let output = GroupAgentScheduledNodeProviderRequestCliOutput::prepared(
        result.disposition,
        result.inspection,
    )?;
    Ok(request_output(output))
}

fn export_release_control(
    args: &Args,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    GroupAgentScheduledNodeProviderRequestService::preflight_inspect(provider_request_id)?;
    let exported = release_service(args)?.export(provider_request_id)?;
    Ok(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::ReleaseControl(
            exported.canonical_json,
        ),
    )
}

fn verify_authorization(
    args: &Args,
    provider_request_id: &str,
    source: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    GroupAgentScheduledNodeProviderRequestService::preflight_inspect(provider_request_id)?;
    let authorization_json = read_authorization(source)?;
    GroupAgentScheduledNodeDispatchAuthorization::decode_exact(&authorization_json)?;
    let verified = release_service(args)?.verify(provider_request_id, &authorization_json)?;
    Ok(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Authorization(Box::new(
            GroupAgentScheduledNodeDispatchAuthorizationCliOutput::verified(verified),
        )),
    )
}

fn verify_readiness(
    args: &Args,
    provider_request_id: &str,
    authorization_source: &str,
    pricing_source: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    GroupAgentScheduledNodeProviderRequestService::preflight_inspect(provider_request_id)?;
    let authorization_json = read_authorization(authorization_source)?;
    let pricing_json = read_pricing(pricing_source)?;
    GroupAgentScheduledNodeDispatchAuthorization::decode_exact(&authorization_json)?;
    GroupAgentNodePricingSnapshot::decode_exact(&pricing_json)?;
    let verified =
        readiness_service(args)?.verify(provider_request_id, &authorization_json, &pricing_json)?;
    let authorization = verified.authorization;
    Ok(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Readiness(Box::new(
            GroupAgentScheduledNodeDispatchReadinessCliOutput::verified(
                verified.v,
                ScheduledReadinessMetadataView {
                    authorization_id: authorization.authorization_id,
                    graph_run_id: authorization.graph_run_id,
                    schedule_id: authorization.schedule_id,
                    scheduled_contract_id: authorization.scheduled_contract_id,
                    scheduled_provider_request_id: authorization.scheduled_provider_request_id,
                    execution_ordinal: authorization.execution_ordinal,
                    node_id: authorization.node_id,
                    attempt: authorization.attempt,
                },
            ),
        )),
    )
}

pub fn write_output(
    output: &GroupAgentScheduledNodeProviderRequestCommandCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    match output {
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Request(output) => {
            scheduled_provider_request_output::write_output(output, json, writer)
        }
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::ReleaseControl(control) => {
            writer.write_all(control.as_bytes())
        }
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Authorization(output) => {
            scheduled_release_output::write_output(output, json, writer)
        }
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Readiness(output) => {
            scheduled_release_readiness_output::write_output(output, json, writer)
        }
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(output) => {
            scheduled_provider_request_output::write_dispatch_execution_output(output, json, writer)
        }
    }
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

fn release_service(
    args: &Args,
) -> Result<GroupAgentScheduledNodeDispatchReleaseControlService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    Ok(GroupAgentScheduledNodeDispatchReleaseControlService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
    ))
}

fn readiness_service(
    args: &Args,
) -> Result<GroupAgentScheduledNodeDispatchReadinessService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    Ok(GroupAgentScheduledNodeDispatchReadinessService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
    ))
}

fn request_output(
    output: GroupAgentScheduledNodeProviderRequestCliOutput,
) -> GroupAgentScheduledNodeProviderRequestCommandCliOutput {
    GroupAgentScheduledNodeProviderRequestCommandCliOutput::Request(Box::new(output))
}

fn read_authorization(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(
            io::stdin().lock(),
            MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES,
            "Scheduled Node Dispatch Authorization",
        )?
    } else {
        read_bounded(
            File::open(source)?,
            MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES,
            "Scheduled Node Dispatch Authorization",
        )?
    };
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Scheduled Node Dispatch Authorization must be UTF-8").into())
}

fn read_pricing(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(
            io::stdin().lock(),
            MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES,
            "Node pricing snapshot",
        )?
    } else {
        read_bounded(
            File::open(source)?,
            MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES,
            "Node pricing snapshot",
        )?
    };
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Node pricing snapshot must be UTF-8").into())
}

fn read_bounded(reader: impl Read, maximum: usize, artifact: &str) -> Result<Vec<u8>, io::Error> {
    let limit = maximum.checked_add(1).expect("artifact bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("artifact bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid_input(&format!("{artifact} exceeds its byte limit")));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
