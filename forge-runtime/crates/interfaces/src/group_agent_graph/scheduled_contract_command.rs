use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    sync::Arc,
};

use forge_runtime_application::{
    AdmitGroupAgentScheduledNodeContractInput, AdmitGroupAgentScheduledNodeSuccessorInput,
    GroupAgentNodeExecutionContractService, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractService, GroupAgentScheduledNodeSuccessorService,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
};
pub(crate) use forge_runtime_application::{
    AdmitGroupAgentScheduledNodeSuccessorInput as WaveAdmitInput,
    GroupAgentScheduledNodeSuccessorService as WaveSuccessorService,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{
        Args, GroupGraphRunScheduledContractCommand, GroupGraphRunScheduledContractSuccessorCommand,
        WaveAdmitExecutionOptions,
    },
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::scheduled_contract_output::GroupAgentScheduledNodeContractCliOutput;

pub fn execute(
    args: &Args,
    command: &GroupGraphRunScheduledContractCommand,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunScheduledContractCommand::Admit {
            graph_run_id,
            contract_source,
        } => admit(args, graph_run_id, contract_source),
        GroupGraphRunScheduledContractCommand::Show {
            contract_id,
            include_contract,
        } => {
            GroupAgentScheduledNodeContractService::preflight_inspect(contract_id)?;
            Ok(GroupAgentScheduledNodeContractCliOutput::contract(
                service(args)?.inspect(contract_id)?,
                *include_contract,
            ))
        }
        GroupGraphRunScheduledContractCommand::List {
            graph_run_id,
            limit,
        } => {
            GroupAgentScheduledNodeContractService::preflight_list(
                graph_run_id.as_deref(),
                *limit,
            )?;
            Ok(GroupAgentScheduledNodeContractCliOutput::list(
                service(args)?.list(graph_run_id.as_deref(), *limit)?,
            ))
        }
        GroupGraphRunScheduledContractCommand::ProviderRequest(_) => {
            unreachable!("provider-request commands are routed by run_command")
        }
        GroupGraphRunScheduledContractCommand::PredecessorReceiptExport {
            provider_request_id,
        } => export_predecessor_receipt(args, provider_request_id),
        GroupGraphRunScheduledContractCommand::Successor(command) => {
            execute_successor(args, command)
        }
        GroupGraphRunScheduledContractCommand::WaveAdmit {
            graph_run_id,
            predecessor_receipt_sources,
            schedule_sha256,
            go_core,
            idempotency_key,
            execution,
        } => execute_wave(args, graph_run_id, predecessor_receipt_sources, schedule_sha256, go_core.as_deref(), idempotency_key.as_deref(), execution),
    }
}

fn export_predecessor_receipt(
    args: &Args,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    let exported = successor_service(args)?.export_predecessor_receipt(provider_request_id)?;
    Ok(
        GroupAgentScheduledNodeContractCliOutput::predecessor_receipt(
            exported.provider_request_id,
            exported.receipt_json,
            exported.receipt_sha256,
        ),
    )
}

fn execute_successor(
    args: &Args,
    command: &GroupGraphRunScheduledContractSuccessorCommand,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunScheduledContractSuccessorCommand::Admit {
            graph_run_id,
            contract_source,
            predecessor_receipt_sources,
            predecessor_content_source,
        } => admit_successor(
            args,
            graph_run_id,
            contract_source,
            predecessor_receipt_sources,
            predecessor_content_source.as_deref(),
        ),
        GroupGraphRunScheduledContractSuccessorCommand::Show {
            contract_id,
            include_contract,
        } => {
            GroupAgentScheduledNodeSuccessorService::preflight_inspect(contract_id)?;
            Ok(GroupAgentScheduledNodeContractCliOutput::successor(
                successor_service(args)?.inspect(contract_id)?,
                *include_contract,
            ))
        }
        GroupGraphRunScheduledContractSuccessorCommand::List {
            graph_run_id,
            limit,
        } => {
            GroupAgentScheduledNodeSuccessorService::preflight_list(
                graph_run_id.as_deref(),
                *limit,
            )?;
            Ok(GroupAgentScheduledNodeContractCliOutput::successor_list(
                successor_service(args)?.list(graph_run_id.as_deref(), *limit)?,
            ))
        }
    }
}

fn admit_successor(
    args: &Args,
    graph_run_id: &str,
    contract_source: &str,
    predecessor_receipt_sources: &[String],
    predecessor_content_source: Option<&str>,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    let contract_json = read_contract(contract_source)?;
    for source in predecessor_receipt_sources {
        read_predecessor_receipt(source)?;
    }
    let predecessor_content = match predecessor_content_source {
        Some(source) => Some(read_predecessor_content(source)?),
        None => None,
    };
    let input = AdmitGroupAgentScheduledNodeSuccessorInput {
        graph_run_id: graph_run_id.into(),
        contract_json,
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-scheduled-node-successor")),
        admitted_at_ms: unix_time_millis(),
        predecessor_content,
    };
    GroupAgentScheduledNodeSuccessorService::preflight_admit(&input)?;
    let result = successor_service(args)?.admit(&input)?;
    Ok(GroupAgentScheduledNodeContractCliOutput::admitted(
        result.disposition,
        result.inspection,
        contract_source != "-",
    ))
}

/// Reads one bounded exact UTF-8 predecessor content file (1 MiB cap).
fn read_predecessor_content(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(std::io::stdin().lock())?
    } else {
        read_bounded(std::fs::File::open(source)?)?
    };
    if bytes.len() > 1024 * 1024 {
        return Err(invalid_input("predecessor content exceeds its byte limit").into());
    }
    String::from_utf8(bytes).map_err(|_| invalid_input("predecessor content must be UTF-8").into())
}

/// Reads one bounded predecessor receipt file; the candidate already binds
/// its digest, so this is a size/canonical preflight only.
fn read_predecessor_receipt(source: &str) -> Result<(), Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(io::stdin().lock())?
    } else {
        read_bounded(File::open(source)?)?
    };
    if bytes.len() > 64 * 1024 {
        return Err(invalid_input("predecessor receipt exceeds its byte limit").into());
    }
    Ok(())
}

pub(crate) fn successor_service(
    args: &Args,
) -> Result<GroupAgentScheduledNodeSuccessorService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentScheduledNodeSuccessorService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store,
    ))
}

fn admit(
    args: &Args,
    graph_run_id: &str,
    contract_source: &str,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    let contract_json = read_contract(contract_source)?;
    let input = AdmitGroupAgentScheduledNodeContractInput {
        graph_run_id: graph_run_id.into(),
        contract_json,
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-scheduled-node-contract")),
        admitted_at_ms: unix_time_millis(),
    };
    GroupAgentScheduledNodeContractService::preflight_admit(&input)?;
    let result = service(args)?.admit(&input)?;
    Ok(GroupAgentScheduledNodeContractCliOutput::admitted(
        result.disposition,
        result.inspection,
        contract_source != "-",
    ))
}

fn execute_wave(
    args: &Args,
    graph_run_id: &str,
    predecessor_receipt_sources: &[String],
    schedule_sha256: &str,
    go_core: Option<&str>,
    idempotency_key: Option<&str>,
    execution: &WaveAdmitExecutionOptions,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    super::wave_command::execute_wave_admit(
        args,
        graph_run_id,
        predecessor_receipt_sources,
        schedule_sha256,
        go_core,
        idempotency_key,
        execution,
    )
}

/// `export_control` loads the graph run control snapshot (the artifact the
/// `control export` command prints); shared with the wave-admit adapter.
pub(crate) fn export_control(
    args: &Args,
    graph_run_id: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service = GroupAgentNodeExecutionContractService::new(
        store.clone(),
        store.clone(),
        store,
    );
    let exported = service.export_control(graph_run_id)?;
    Ok(exported.snapshot_json.into_bytes())
}

fn service(args: &Args) -> Result<GroupAgentScheduledNodeContractService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentScheduledNodeContractService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store,
    ))
}

fn read_contract(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(io::stdin().lock())?
    } else {
        read_bounded(File::open(source)?)?
    };
    GroupAgentScheduledNodeContractCandidate::decode_exact_bytes(&bytes)
        .map_err(|_| invalid_input("invalid or noncanonical scheduled-node contract candidate"))?;
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("scheduled-node contract candidate must be UTF-8").into())
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let limit = MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES
        .checked_add(1)
        .expect("contract candidate bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("contract candidate bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES {
        return Err(invalid_input(
            "scheduled-node contract candidate exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
