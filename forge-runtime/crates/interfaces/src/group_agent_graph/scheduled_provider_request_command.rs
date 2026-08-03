use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    sync::Arc,
};

use forge_runtime_application::{
    GroupAgentScheduledNodeDispatchReleaseControlService,
    GroupAgentScheduledNodeProviderRequestService,
    PrepareGroupAgentScheduledNodeProviderRequestInput,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupGraphRunScheduledContractProviderRequestCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    runtime_domain::{
        GroupAgentScheduledNodeDispatchAuthorization,
        MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES,
    },
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::{
    scheduled_provider_request_output::{self, GroupAgentScheduledNodeProviderRequestCliOutput},
    scheduled_release_output::{self, GroupAgentScheduledNodeDispatchAuthorizationCliOutput},
};

pub enum GroupAgentScheduledNodeProviderRequestCommandCliOutput {
    Request(Box<GroupAgentScheduledNodeProviderRequestCliOutput>),
    ReleaseControl(String),
    Authorization(Box<GroupAgentScheduledNodeDispatchAuthorizationCliOutput>),
}

pub fn execute(
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
        } => {
            GroupAgentScheduledNodeProviderRequestService::preflight_inspect(provider_request_id)?;
            let output = GroupAgentScheduledNodeProviderRequestCliOutput::request(
                read_service(args)?.inspect(provider_request_id)?,
                *include_request,
            )?;
            Ok(request_output(output))
        }
        GroupGraphRunScheduledContractProviderRequestCommand::List {
            graph_run_id,
            limit,
        } => {
            GroupAgentScheduledNodeProviderRequestService::preflight_list(
                graph_run_id.as_deref(),
                *limit,
            )?;
            Ok(request_output(
                GroupAgentScheduledNodeProviderRequestCliOutput::list(
                    read_service(args)?.list(graph_run_id.as_deref(), *limit)?,
                ),
            ))
        }
        GroupGraphRunScheduledContractProviderRequestCommand::ReleaseControlExport {
            provider_request_id,
        } => export_release_control(args, provider_request_id),
        GroupGraphRunScheduledContractProviderRequestCommand::AuthorizationVerify {
            provider_request_id,
            authorization_source,
        } => verify_authorization(args, provider_request_id, authorization_source),
    }
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

fn request_output(
    output: GroupAgentScheduledNodeProviderRequestCliOutput,
) -> GroupAgentScheduledNodeProviderRequestCommandCliOutput {
    GroupAgentScheduledNodeProviderRequestCommandCliOutput::Request(Box::new(output))
}

fn read_authorization(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(io::stdin().lock())?
    } else {
        read_bounded(File::open(source)?)?
    };
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Scheduled Node Dispatch Authorization must be UTF-8").into())
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let maximum = MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES;
    let limit = maximum
        .checked_add(1)
        .expect("authorization bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("authorization bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid_input(
            "Scheduled Node Dispatch Authorization exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
