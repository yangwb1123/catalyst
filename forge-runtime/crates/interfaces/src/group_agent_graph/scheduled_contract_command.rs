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
    MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES,
};
pub(crate) use forge_runtime_application::{
    AdmitGroupAgentScheduledNodeSuccessorInput as WaveAdmitInput,
    GroupAgentScheduledNodeSuccessorService as WaveSuccessorService,
};
use forge_runtime_domain::{
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodePredecessorOutcome,
    GroupAgentScheduledNodePredecessorReceipt, GroupAgentScheduledNodeTerminalReceipt,
    MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{
        Args, GroupGraphRunScheduledContractCommand,
        GroupGraphRunScheduledContractSuccessorCommand, WaveAdmitExecutionOptions,
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
        } => inspect_contract(args, contract_id, *include_contract),
        GroupGraphRunScheduledContractCommand::List {
            graph_run_id,
            limit,
        } => list_contracts(args, graph_run_id.as_deref(), *limit),
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
        } => execute_wave(
            args,
            graph_run_id,
            predecessor_receipt_sources,
            schedule_sha256,
            go_core.as_deref(),
            idempotency_key.as_deref(),
            execution,
        ),
    }
}

fn inspect_contract(
    args: &Args,
    contract_id: &str,
    include_contract: bool,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    GroupAgentScheduledNodeContractService::preflight_inspect(contract_id)?;
    Ok(GroupAgentScheduledNodeContractCliOutput::contract(
        service(args)?.inspect(contract_id)?,
        include_contract,
    ))
}

fn list_contracts(
    args: &Args,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    GroupAgentScheduledNodeContractService::preflight_list(graph_run_id, limit)?;
    Ok(GroupAgentScheduledNodeContractCliOutput::list(
        service(args)?.list(graph_run_id, limit)?,
    ))
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
    let candidate =
        GroupAgentScheduledNodeContractCandidate::decode_exact_bytes(contract_json.as_bytes())
            .map_err(|_| invalid_input("invalid successor contract candidate"))?;
    let supplied_receipts = predecessor_receipt_sources
        .iter()
        .map(|source| read_predecessor_receipt(source))
        .collect::<Result<Vec<_>, _>>()?;
    validate_supplied_receipts(
        &candidate.graph_run_id,
        &candidate.request.predecessor_terminal_receipts,
        &supplied_receipts,
    )?;
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
        read_bounded(
            std::io::stdin().lock(),
            MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES,
            "predecessor content",
        )?
    } else {
        read_bounded(
            std::fs::File::open(source)?,
            MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES,
            "predecessor content",
        )?
    };
    String::from_utf8(bytes).map_err(|_| invalid_input("predecessor content must be UTF-8").into())
}

/// Reads and strictly decodes one exact bounded Core terminal receipt.
fn read_predecessor_receipt(
    source: &str,
) -> Result<GroupAgentScheduledNodeTerminalReceipt, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(
            io::stdin().lock(),
            MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES,
            "predecessor receipt",
        )?
    } else {
        read_bounded(
            File::open(source)?,
            MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES,
            "predecessor receipt",
        )?
    };
    GroupAgentScheduledNodeTerminalReceipt::decode_exact(&bytes)
        .map_err(|_| invalid_input("invalid or noncanonical predecessor receipt").into())
}

fn validate_supplied_receipts(
    graph_run_id: &str,
    expected: &[GroupAgentScheduledNodePredecessorReceipt],
    supplied: &[GroupAgentScheduledNodeTerminalReceipt],
) -> Result<(), io::Error> {
    let matches = expected.len() == supplied.len()
        && expected.iter().all(|compact| {
            supplied
                .iter()
                .filter(|full| supplied_receipt_matches(graph_run_id, compact, full))
                .count()
                == 1
        });
    matches
        .then_some(())
        .ok_or_else(|| invalid_input("supplied predecessor receipts disagree with the candidate"))
}

fn supplied_receipt_matches(
    graph_run_id: &str,
    compact: &GroupAgentScheduledNodePredecessorReceipt,
    full: &GroupAgentScheduledNodeTerminalReceipt,
) -> bool {
    compact.predecessor_node_id == full.node_id
        && compact.predecessor_attempt == full.attempt
        && compact.terminal_event_seq == 0
        && compact.terminal_event_sha256.is_empty()
        && compact.terminal_receipt_id == full.receipt_id
        && compact.terminal_receipt_sha256 == full.receipt_sha256
        && compact.node_outcome == GroupAgentScheduledNodePredecessorOutcome::Completed
        && full.node_outcome == GroupAgentNodeTerminalOutcome::Completed
        && compact.provider_request_id == full.provider_request_id
        && compact.dispatch_id == full.dispatch_id
        && full.graph_run_id == graph_run_id
}

pub(crate) fn successor_service(
    args: &Args,
) -> Result<GroupAgentScheduledNodeSuccessorService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(
        GroupAgentScheduledNodeSuccessorService::new_with_any_lifecycles(
            store.clone(),
            store.clone(),
            store.clone(),
            store.clone(),
            store,
        ),
    )
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
pub(crate) fn export_control(args: &Args, graph_run_id: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service = GroupAgentNodeExecutionContractService::new(store.clone(), store.clone(), store);
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
        read_bounded(
            io::stdin().lock(),
            MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
            "scheduled-node contract candidate",
        )?
    } else {
        read_bounded(
            File::open(source)?,
            MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
            "scheduled-node contract candidate",
        )?
    };
    GroupAgentScheduledNodeContractCandidate::decode_exact_bytes(&bytes)
        .map_err(|_| invalid_input("invalid or noncanonical scheduled-node contract candidate"))?;
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("scheduled-node contract candidate must be UTF-8").into())
}

fn read_bounded(reader: impl Read, maximum: usize, label: &str) -> Result<Vec<u8>, io::Error> {
    let limit = maximum
        .checked_add(1)
        .expect("public input bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("public input bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid_input(&format!("{label} exceeds its byte limit")));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
#[path = "tests/scheduled_contract_command.rs"]
mod tests;
