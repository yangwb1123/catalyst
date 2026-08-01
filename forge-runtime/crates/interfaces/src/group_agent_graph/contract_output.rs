use std::io::{self, Write};

use forge_runtime_application::{
    AdmitGroupAgentNodeExecutionContractDisposition, GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
    GroupAgentGraphRunRecord, GroupAgentNodeExecutionContract,
    GroupAgentNodeExecutionContractInspection, GroupAgentNodeExecutionContractRecord,
};
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupAgentNodeExecutionContractCliOutput {
    #[serde(rename = "group_agent_node_execution_contract_admitted")]
    Admitted {
        v: u16,
        disposition: AdmitGroupAgentNodeExecutionContractDisposition,
        inspection: ContractInspectionView,
    },
    #[serde(rename = "group_agent_node_execution_contract")]
    Contract {
        v: u16,
        inspection: ContractInspectionView,
    },
    #[serde(rename = "group_agent_node_execution_contracts")]
    Contracts {
        v: u16,
        metadata_only: bool,
        returned_contracts_present: bool,
        dispatch_authority_released: bool,
        execution_performed: bool,
        manager_execution_performed: bool,
        node_execution_performed: bool,
        credential_read: bool,
        model_used: bool,
        provider_used: bool,
        network_accessed: bool,
        workspace_accessed: bool,
        tools_used: bool,
        task_results_produced: bool,
        conversation_or_prompt_written: bool,
        memory_written: bool,
        writeback_performed: bool,
        contract_included: bool,
        contracts: Vec<ContractMetadataView>,
    },
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ContractInspectionView {
    v: u16,
    contract_admitted: bool,
    source_graph_validated: bool,
    control_snapshot_validated: bool,
    contract_and_journal_validated: bool,
    execution_contract_present: bool,
    dispatch_authority_released: bool,
    provider_configuration_present: bool,
    model_selected: bool,
    credential_read: bool,
    model_used: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    task_results_produced: bool,
    conversation_or_prompt_written: bool,
    memory_written: bool,
    writeback_performed: bool,
    explicit_contract_file_read: bool,
    contract_included: bool,
    control_snapshot_included: bool,
    record: ContractMetadataView,
    graph_run: GroupAgentGraphRunRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract: Option<GroupAgentNodeExecutionContract>,
}

#[derive(Serialize)]
pub struct ContractMetadataView {
    v: u16,
    contract_id: String,
    graph_run_id: String,
    attempt: u16,
    contract_bytes: usize,
    created_at_ms: u64,
}

impl GroupAgentNodeExecutionContractCliOutput {
    pub fn admitted(
        disposition: AdmitGroupAgentNodeExecutionContractDisposition,
        inspection: GroupAgentNodeExecutionContractInspection,
        explicit_contract_file_read: bool,
    ) -> Self {
        Self::Admitted {
            v: inspection.v,
            disposition,
            inspection: ContractInspectionView::new(inspection, false, explicit_contract_file_read),
        }
    }

    pub fn contract(
        inspection: GroupAgentNodeExecutionContractInspection,
        include_contract: bool,
    ) -> Self {
        Self::Contract {
            v: inspection.v,
            inspection: ContractInspectionView::new(inspection, include_contract, false),
        }
    }

    pub fn list(records: Vec<GroupAgentNodeExecutionContractRecord>) -> Self {
        let returned_contracts_present = !records.is_empty();
        let contracts = records
            .into_iter()
            .map(ContractMetadataView::from)
            .collect();
        Self::Contracts {
            v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
            metadata_only: true,
            returned_contracts_present,
            dispatch_authority_released: false,
            execution_performed: false,
            manager_execution_performed: false,
            node_execution_performed: false,
            credential_read: false,
            model_used: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            task_results_produced: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            contract_included: false,
            contracts,
        }
    }
}

impl ContractInspectionView {
    fn new(
        inspection: GroupAgentNodeExecutionContractInspection,
        include_contract: bool,
        explicit_contract_file_read: bool,
    ) -> Self {
        Self {
            v: inspection.v,
            contract_admitted: true,
            source_graph_validated: true,
            control_snapshot_validated: true,
            contract_and_journal_validated: true,
            execution_contract_present: true,
            dispatch_authority_released: false,
            provider_configuration_present: true,
            model_selected: true,
            credential_read: false,
            model_used: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            task_results_produced: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            explicit_contract_file_read,
            contract_included: include_contract,
            control_snapshot_included: false,
            record: ContractMetadataView::from(inspection.record),
            graph_run: inspection.graph_run.run,
            contract: include_contract.then_some(inspection.contract),
        }
    }
}

impl From<GroupAgentNodeExecutionContractRecord> for ContractMetadataView {
    fn from(record: GroupAgentNodeExecutionContractRecord) -> Self {
        Self {
            v: record.v,
            contract_id: record.contract_id,
            graph_run_id: record.graph_run_id,
            attempt: record.attempt,
            contract_bytes: record.contract_bytes,
            created_at_ms: record.created_at_ms,
        }
    }
}

pub fn write_output(
    output: &GroupAgentNodeExecutionContractCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    match output {
        GroupAgentNodeExecutionContractCliOutput::Admitted {
            disposition,
            inspection,
            ..
        } => {
            writeln!(
                writer,
                "admitted Node Execution Contract — {}",
                disposition_label(*disposition)
            )?;
            write_inspection(inspection, writer)
        }
        GroupAgentNodeExecutionContractCliOutput::Contract { inspection, .. } => {
            write_inspection(inspection, writer)
        }
        GroupAgentNodeExecutionContractCliOutput::Contracts { contracts, .. } => {
            write_list(contracts, writer)
        }
    }
}

fn write_inspection(
    inspection: &ContractInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "contract {} · graph_run={} · attempt={} · status=awaiting_core_dispatch",
        terminal_text(&inspection.record.contract_id),
        terminal_text(&inspection.record.graph_run_id),
        inspection.record.attempt
    )?;
    if let Some(contract) = &inspection.contract {
        let json = contract
            .canonical_json()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        writeln!(writer, "contract: {}", terminal_text(&json))?;
    } else {
        writeln!(
            writer,
            "contract hidden; use --include-contract to reveal private plaintext"
        )?;
    }
    write_boundaries(writer)
}

fn write_list(
    contracts: &[ContractMetadataView],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Node Execution Contracts: {} (metadata only; use show for validation)",
        contracts.len()
    )?;
    for contract in contracts {
        writeln!(
            writer,
            "{}\tgraph_run={}\tattempt={}\tcreated={}",
            terminal_text(&contract.contract_id),
            terminal_text(&contract.graph_run_id),
            contract.attempt,
            contract.created_at_ms
        )?;
    }
    write_boundaries(writer)
}

fn write_boundaries(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "execution contract present; dispatch authority not released"
    )?;
    writeln!(
        writer,
        "provider/model configuration selected but unused; no credential read"
    )?;
    writeln!(
        writer,
        "manager/node Agents not executed; no provider/network/workspace/tools or result"
    )?;
    writeln!(
        writer,
        "no Conversation/Prompt/memory/writeback operation occurred"
    )
}

fn disposition_label(disposition: AdmitGroupAgentNodeExecutionContractDisposition) -> &'static str {
    match disposition {
        AdmitGroupAgentNodeExecutionContractDisposition::Created => "created",
        AdmitGroupAgentNodeExecutionContractDisposition::Replayed => "replayed",
    }
}
