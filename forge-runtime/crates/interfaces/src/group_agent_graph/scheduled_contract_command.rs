use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    sync::Arc,
};

use forge_runtime_application::{
    AdmitGroupAgentScheduledNodeContractInput, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractService, MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupGraphRunScheduledContractCommand},
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
    }
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
