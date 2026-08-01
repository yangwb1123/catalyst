use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    sync::Arc,
};

use forge_runtime_application::{
    AdmitGroupAgentNodeExecutionContractInput, GroupAgentNodeExecutionContract,
    GroupAgentNodeExecutionContractService, MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupGraphRunContractCommand, GroupGraphRunControlCommand},
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::contract_output::GroupAgentNodeExecutionContractCliOutput;

pub enum GroupAgentGraphControlContractCliOutput {
    ControlSnapshot(String),
    Contract(Box<GroupAgentNodeExecutionContractCliOutput>),
}

pub fn execute_control(
    args: &Args,
    command: &GroupGraphRunControlCommand,
) -> Result<GroupAgentGraphControlContractCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunControlCommand::Export { graph_run_id } => {
            let exported = service(args)?.export_control(graph_run_id)?;
            Ok(GroupAgentGraphControlContractCliOutput::ControlSnapshot(
                exported.snapshot_json,
            ))
        }
    }
}

pub fn execute_contract(
    args: &Args,
    command: &GroupGraphRunContractCommand,
) -> Result<GroupAgentGraphControlContractCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunContractCommand::Admit {
            graph_run_id,
            contract_source,
        } => admit(args, graph_run_id, contract_source),
        GroupGraphRunContractCommand::Show {
            contract_id,
            include_contract,
        } => {
            let inspection = service(args)?.inspect(contract_id)?;
            Ok(contract_output(
                GroupAgentNodeExecutionContractCliOutput::contract(inspection, *include_contract),
            ))
        }
        GroupGraphRunContractCommand::List {
            graph_run_id,
            limit,
        } => {
            let records = service(args)?.list(graph_run_id.as_deref(), *limit)?;
            Ok(contract_output(
                GroupAgentNodeExecutionContractCliOutput::list(records),
            ))
        }
    }
}

fn admit(
    args: &Args,
    graph_run_id: &str,
    contract_source: &str,
) -> Result<GroupAgentGraphControlContractCliOutput, Box<dyn Error>> {
    let contract_json = read_contract(contract_source)?;
    let service = service(args)?;
    let result = service.admit(&AdmitGroupAgentNodeExecutionContractInput {
        graph_run_id: graph_run_id.into(),
        contract_json,
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-node-contract")),
        admitted_at_ms: unix_time_millis(),
    })?;
    Ok(contract_output(
        GroupAgentNodeExecutionContractCliOutput::admitted(
            result.disposition,
            result.inspection,
            contract_source != "-",
        ),
    ))
}

fn service(args: &Args) -> Result<GroupAgentNodeExecutionContractService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentNodeExecutionContractService::new(
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
    let text = String::from_utf8(bytes)
        .map_err(|_| invalid_input("Node Execution Contract must be UTF-8"))?;
    let contract: GroupAgentNodeExecutionContract = serde_json::from_str(&text)
        .map_err(|_| invalid_input("invalid Node Execution Contract JSON"))?;
    contract
        .validate()
        .map_err(|_| invalid_input("invalid Node Execution Contract"))?;
    let canonical = contract
        .canonical_json()
        .map_err(|_| invalid_input("invalid Node Execution Contract"))?;
    if canonical.as_bytes() != text.as_bytes() {
        return Err(invalid_input("Node Execution Contract is not canonical").into());
    }
    Ok(text)
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let limit = MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES
        .checked_add(1)
        .expect("Node Execution Contract bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("contract bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES {
        return Err(invalid_input(
            "Node Execution Contract exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

fn contract_output(
    output: GroupAgentNodeExecutionContractCliOutput,
) -> GroupAgentGraphControlContractCliOutput {
    GroupAgentGraphControlContractCliOutput::Contract(Box::new(output))
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
