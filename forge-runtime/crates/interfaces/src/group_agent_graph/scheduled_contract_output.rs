use std::io::{self, Write};

use forge_runtime_application::{
    AdmitGroupAgentScheduledNodeContractDisposition, GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractInspection,
    GroupAgentScheduledNodeContractRecord,
};
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeContractCliOutput {
    #[serde(rename = "group_agent_scheduled_node_contract_candidate_admitted")]
    Admitted {
        v: u16,
        disposition: AdmitGroupAgentScheduledNodeContractDisposition,
        inspection: ScheduledContractInspectionView,
    },
    #[serde(rename = "group_agent_scheduled_node_contract_candidate")]
    Contract {
        v: u16,
        inspection: ScheduledContractInspectionView,
    },
    #[serde(rename = "group_agent_scheduled_node_predecessor_receipt")]
    PredecessorReceipt {
        v: u16,
        provider_request_id: String,
        receipt_sha256: String,
        receipt_included: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        receipt: Option<String>,
    },
    #[serde(rename = "group_agent_scheduled_node_successor_candidate")]
    Successor {
        v: u16,
        inspection: ScheduledContractInspectionView,
    },
    #[serde(rename = "group_agent_scheduled_node_successor_candidates")]
    Successors {
        v: u16,
        metadata_only: bool,
        passive_successor_candidate_only: bool,
        current_run_lifecycle_included: bool,
        lifecycle_contract_admitted: bool,
        provider_request_present: bool,
        execution_authority_released: bool,
        dispatch_authority_released: bool,
        progress_observed: bool,
        successor_advance_authorized: bool,
        credential_read: bool,
        provider_used: bool,
        network_accessed: bool,
        workspace_accessed: bool,
        tools_used: bool,
        result_or_receipt_produced: bool,
        writeback_performed: bool,
        contracts: Vec<ScheduledContractMetadataView>,
    },
    #[serde(rename = "group_agent_scheduled_node_wave_admit")]
    Wave {
        v: u16,
        wave: Vec<super::wave_command::WaveAdmitNodeOutput>,
        rejected: Vec<super::wave_command::WaveAdmitNodeOutput>,
    },
    #[serde(rename = "group_agent_scheduled_node_contract_candidates")]
    Contracts {
        v: u16,
        metadata_only: bool,
        passive_initial_candidate_only: bool,
        candidate_creation_state_only: bool,
        current_run_lifecycle_included: bool,
        current_provider_request_sidecar_included: bool,
        lifecycle_contract_admitted: bool,
        provider_request_present: bool,
        provider_request_present_at_candidate_creation: bool,
        execution_authority_released: bool,
        dispatch_authority_released: bool,
        progress_observed: bool,
        successor_advance_authorized: bool,
        credential_read: bool,
        provider_used: bool,
        network_accessed: bool,
        workspace_accessed: bool,
        tools_used: bool,
        result_or_receipt_produced: bool,
        writeback_performed: bool,
        contracts: Vec<ScheduledContractMetadataView>,
    },
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScheduledContractInspectionView {
    v: u16,
    passive_initial_candidate_only: bool,
    candidate_creation_state_only: bool,
    source_graph_validated: bool,
    control_snapshot_validated: bool,
    stored_schedule_validated: bool,
    current_run_lifecycle_included: bool,
    current_provider_request_sidecar_included: bool,
    lifecycle_contract_admitted: bool,
    provider_request_present: bool,
    provider_request_present_at_candidate_creation: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    progress_observed: bool,
    successor_advance_authorized: bool,
    predecessor_receipts_present: bool,
    predecessor_content_included: bool,
    credential_read: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    result_or_receipt_produced: bool,
    writeback_performed: bool,
    explicit_contract_file_read: bool,
    contract_included: bool,
    record: ScheduledContractMetadataView,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract: Option<GroupAgentScheduledNodeContractCandidate>,
}

#[derive(Serialize)]
pub struct ScheduledContractMetadataView {
    v: u16,
    contract_id: String,
    graph_run_id: String,
    schedule_id: String,
    contract_sha256: String,
    contract_bytes: usize,
    execution_ordinal: usize,
    attempt: u16,
    created_at_ms: u64,
}

impl GroupAgentScheduledNodeContractCliOutput {
    pub fn wave(output: super::wave_command::WaveAdmitOutput) -> Self {
        Self::Wave {
            v: 1,
            wave: output.wave,
            rejected: output.rejected,
        }
    }


    pub fn admitted(
        disposition: AdmitGroupAgentScheduledNodeContractDisposition,
        inspection: GroupAgentScheduledNodeContractInspection,
        explicit_contract_file_read: bool,
    ) -> Self {
        Self::Admitted {
            v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
            disposition,
            inspection: ScheduledContractInspectionView::new(
                inspection,
                false,
                explicit_contract_file_read,
            ),
        }
    }

    pub fn contract(
        inspection: GroupAgentScheduledNodeContractInspection,
        include_contract: bool,
    ) -> Self {
        Self::Contract {
            v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
            inspection: ScheduledContractInspectionView::new(inspection, include_contract, false),
        }
    }

    pub fn predecessor_receipt(
        provider_request_id: String,
        receipt_json: String,
        receipt_sha256: String,
    ) -> Self {
        Self::PredecessorReceipt {
            v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
            provider_request_id,
            receipt_sha256,
            receipt_included: true,
            receipt: Some(receipt_json),
        }
    }

    pub fn successor(
        inspection: GroupAgentScheduledNodeContractInspection,
        include_contract: bool,
    ) -> Self {
        Self::Successor {
            v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
            inspection: ScheduledContractInspectionView::new_successor(
                inspection,
                include_contract,
                false,
            ),
        }
    }

    pub fn successor_list(records: Vec<GroupAgentScheduledNodeContractRecord>) -> Self {
        Self::Successors {
            v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
            metadata_only: true,
            passive_successor_candidate_only: true,
            current_run_lifecycle_included: false,
            lifecycle_contract_admitted: false,
            provider_request_present: false,
            execution_authority_released: false,
            dispatch_authority_released: false,
            progress_observed: false,
            successor_advance_authorized: false,
            credential_read: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            result_or_receipt_produced: false,
            writeback_performed: false,
            contracts: records
                .into_iter()
                .map(ScheduledContractMetadataView::from)
                .collect(),
        }
    }

    pub fn list(records: Vec<GroupAgentScheduledNodeContractRecord>) -> Self {
        Self::Contracts {
            v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
            metadata_only: true,
            passive_initial_candidate_only: true,
            candidate_creation_state_only: true,
            current_run_lifecycle_included: false,
            current_provider_request_sidecar_included: false,
            lifecycle_contract_admitted: false,
            provider_request_present: false,
            provider_request_present_at_candidate_creation: false,
            execution_authority_released: false,
            dispatch_authority_released: false,
            progress_observed: false,
            successor_advance_authorized: false,
            credential_read: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            result_or_receipt_produced: false,
            writeback_performed: false,
            contracts: records
                .into_iter()
                .map(ScheduledContractMetadataView::from)
                .collect(),
        }
    }
}

impl ScheduledContractInspectionView {
    fn new_successor(
        inspection: GroupAgentScheduledNodeContractInspection,
        include_contract: bool,
        explicit_contract_file_read: bool,
    ) -> Self {
        let mut view = Self::new(inspection, include_contract, explicit_contract_file_read);
        view.passive_initial_candidate_only = false;
        view.predecessor_receipts_present = true;
        view
    }

    fn new(
        inspection: GroupAgentScheduledNodeContractInspection,
        include_contract: bool,
        explicit_contract_file_read: bool,
    ) -> Self {
        Self {
            v: inspection.v,
            passive_initial_candidate_only: true,
            candidate_creation_state_only: true,
            source_graph_validated: true,
            control_snapshot_validated: true,
            stored_schedule_validated: true,
            current_run_lifecycle_included: false,
            current_provider_request_sidecar_included: false,
            lifecycle_contract_admitted: false,
            provider_request_present: false,
            provider_request_present_at_candidate_creation: false,
            execution_authority_released: false,
            dispatch_authority_released: false,
            progress_observed: false,
            successor_advance_authorized: false,
            predecessor_receipts_present: false,
            predecessor_content_included: false,
            credential_read: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            result_or_receipt_produced: false,
            writeback_performed: false,
            explicit_contract_file_read,
            contract_included: include_contract,
            record: ScheduledContractMetadataView::from(inspection.record),
            contract: include_contract.then_some(inspection.candidate),
        }
    }
}

impl From<GroupAgentScheduledNodeContractRecord> for ScheduledContractMetadataView {
    fn from(record: GroupAgentScheduledNodeContractRecord) -> Self {
        Self {
            v: record.v,
            contract_id: record.contract_id,
            graph_run_id: record.graph_run_id,
            schedule_id: record.schedule_id,
            contract_sha256: record.contract_sha256,
            contract_bytes: record.contract_bytes,
            execution_ordinal: record.execution_ordinal,
            attempt: record.attempt,
            created_at_ms: record.created_at_ms,
        }
    }
}

pub fn write_output(
    output: &GroupAgentScheduledNodeContractCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    match output {
        GroupAgentScheduledNodeContractCliOutput::Wave { wave, rejected, .. } => {
            super::wave_command::write_wave(writer, wave, rejected)
        }
        GroupAgentScheduledNodeContractCliOutput::Admitted {
            disposition,
            inspection,
            ..
        } => {
            writeln!(
                writer,
                "admitted passive scheduled-node contract candidate — {}",
                disposition_label(*disposition)
            )?;
            write_inspection(inspection, writer)
        }
        GroupAgentScheduledNodeContractCliOutput::Contract { inspection, .. }
        | GroupAgentScheduledNodeContractCliOutput::Successor { inspection, .. } => {
            write_inspection(inspection, writer)
        }
        GroupAgentScheduledNodeContractCliOutput::Contracts { contracts, .. }
        | GroupAgentScheduledNodeContractCliOutput::Successors { contracts, .. } => {
            write_list(contracts, writer)
        }
        GroupAgentScheduledNodeContractCliOutput::PredecessorReceipt {
            provider_request_id,
            receipt_sha256,
            receipt_included,
            receipt,
            ..
        } => write_predecessor_receipt(
            provider_request_id,
            receipt_sha256,
            *receipt_included,
            receipt.as_deref(),
            writer,
        ),
    }
}

fn write_predecessor_receipt(
    provider_request_id: &str,
    receipt_sha256: &str,
    receipt_included: bool,
    receipt: Option<&str>,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "predecessor receipt · provider_request={} · sha256={}",
        terminal_text(provider_request_id),
        terminal_text(receipt_sha256)
    )?;
    if receipt_included {
        if let Some(receipt) = receipt {
            writeln!(writer, "receipt: {}", terminal_text(receipt))?;
        }
    } else {
        writeln!(
            writer,
            "receipt hidden; use --include-receipt to reveal exact evidence"
        )?;
    }
    write_successor_boundaries(writer)
}

fn write_inspection(
    inspection: &ScheduledContractInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "candidate {} · graph_run={} · ordinal={} · attempt={}",
        terminal_text(&inspection.record.contract_id),
        terminal_text(&inspection.record.graph_run_id),
        inspection.record.execution_ordinal,
        inspection.record.attempt
    )?;
    if let Some(contract) = &inspection.contract {
        let json = contract
            .canonical_json()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        writeln!(writer, "contract candidate: {}", terminal_text(&json))?;
    } else {
        writeln!(
            writer,
            "contract hidden; use --include-contract to reveal private plaintext"
        )?;
    }
    write_boundaries(writer)
}

fn write_list(
    contracts: &[ScheduledContractMetadataView],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Scheduled-node contract candidates: {} (metadata only; use show for validation)",
        contracts.len()
    )?;
    for contract in contracts {
        writeln!(
            writer,
            "{}\tgraph_run={}\tordinal={}\tattempt={}\tcreated={}",
            terminal_text(&contract.contract_id),
            terminal_text(&contract.graph_run_id),
            contract.execution_ordinal,
            contract.attempt,
            contract.created_at_ms
        )?;
    }
    write_boundaries(writer)
}

fn write_successor_boundaries(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "passive successor candidate evidence; not a Run lifecycle contract"
    )?;
    writeln!(writer, "current Run lifecycle is not reported")?;
    writeln!(
        writer,
        "receipt is predecessor evidence only; no successor authority is granted"
    )
}

fn write_boundaries(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "passive initial-node candidate only; not a Run lifecycle contract"
    )?;
    writeln!(writer, "current Run lifecycle is not reported")?;
    writeln!(
        writer,
        "candidate creation flags report no provider request or authority at creation"
    )?;
    writeln!(
        writer,
        "current provider-request sidecars, Run progress, receipts, and successors are not reported"
    )?;
    writeln!(
        writer,
        "no credential/provider/network/workspace/tool/result/receipt/writeback effect"
    )
}

fn disposition_label(value: AdmitGroupAgentScheduledNodeContractDisposition) -> &'static str {
    match value {
        AdmitGroupAgentScheduledNodeContractDisposition::Created => "created",
        AdmitGroupAgentScheduledNodeContractDisposition::Replayed => "replayed",
    }
}
