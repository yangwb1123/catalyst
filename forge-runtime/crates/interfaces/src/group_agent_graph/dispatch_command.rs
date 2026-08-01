use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    sync::Arc,
};

use forge_runtime_application::{
    GroupAgentNodeDispatchReleaseControlService, GroupAgentNodeDispatchRequestService,
    MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES, PrepareGroupAgentNodeDispatchRequestInput,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupGraphRunDispatchCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::{
    dispatch_authorization_output::{self, GroupAgentNodeDispatchAuthorizationCliOutput},
    dispatch_output::{self, GroupAgentNodeDispatchRequestCliOutput},
};

pub enum GroupAgentGraphRunDispatchCommandCliOutput {
    Request(Box<GroupAgentNodeDispatchRequestCliOutput>),
    ReleaseControl(String),
    Authorization(Box<GroupAgentNodeDispatchAuthorizationCliOutput>),
}

pub fn execute(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let authorization_json = match command {
        GroupGraphRunDispatchCommand::AuthorizationVerify {
            authorization_source,
            ..
        } => Some(read_authorization(authorization_source)?),
        _ => None,
    };
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
            let authorization_json = authorization_json
                .as_deref()
                .expect("authorization was read before service construction");
            verify_authorization(args, graph_run_id, authorization_json)
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
    }
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

fn read_authorization(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_authorization_bounded(io::stdin().lock())?
    } else {
        read_authorization_bounded(File::open(source)?)?
    };
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Node Dispatch Authorization must be UTF-8").into())
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

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
#[path = "dispatch_command_tests.rs"]
mod tests;
