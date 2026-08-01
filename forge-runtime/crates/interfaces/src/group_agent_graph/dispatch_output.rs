use std::io::{self, Write};

use forge_runtime_application::{
    GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION, GroupAgentNodeDispatchRequestInspection,
    GroupAgentNodeDispatchRequestRecord, PrepareGroupAgentNodeDispatchRequestDisposition,
};
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupAgentNodeDispatchRequestCliOutput {
    #[serde(rename = "group_agent_node_dispatch_request_prepared")]
    Prepared {
        v: u16,
        disposition: PrepareGroupAgentNodeDispatchRequestDisposition,
        inspection: DispatchRequestInspectionView,
    },
    #[serde(rename = "group_agent_node_dispatch_request")]
    DispatchRequest {
        v: u16,
        inspection: DispatchRequestInspectionView,
    },
    #[serde(rename = "group_agent_node_dispatch_requests")]
    DispatchRequests {
        v: u16,
        metadata_only: bool,
        source_contract_and_request_validated: bool,
        returned_requests_present: bool,
        request_preparation_validated: bool,
        dispatch_authority_released: bool,
        fresh_off_machine_consent_obtained: bool,
        credential_read: bool,
        execution_performed: bool,
        model_used: bool,
        provider_used: bool,
        network_accessed: bool,
        workspace_accessed: bool,
        tools_used: bool,
        result_produced: bool,
        conversation_or_prompt_written: bool,
        memory_written: bool,
        writeback_performed: bool,
        pricing_snapshot_identity_validated: bool,
        pricing_policy_enforced: bool,
        request_included: bool,
        requests: Vec<DispatchRequestMetadataView>,
    },
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct DispatchRequestInspectionView {
    v: u16,
    request_prepared: bool,
    source_graph_validated: bool,
    contract_and_journal_validated: bool,
    request_body_validated: bool,
    dispatch_authority_released: bool,
    fresh_off_machine_consent_obtained: bool,
    credential_read: bool,
    execution_performed: bool,
    model_selected: bool,
    model_used: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    result_produced: bool,
    conversation_or_prompt_written: bool,
    memory_written: bool,
    writeback_performed: bool,
    pricing_snapshot_identity_pinned: bool,
    pricing_policy_enforced: bool,
    request_included: bool,
    record: DispatchRequestMetadataView,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_request_body: Option<String>,
}

#[derive(Serialize)]
pub struct DispatchRequestMetadataView {
    v: u16,
    dispatch_request_id: String,
    graph_run_id: String,
    contract_id: String,
    attempt: u16,
    provider_request_bytes: usize,
    codec_protocol_version: u16,
    created_at_ms: u64,
}

impl GroupAgentNodeDispatchRequestCliOutput {
    pub fn prepared(
        disposition: PrepareGroupAgentNodeDispatchRequestDisposition,
        inspection: GroupAgentNodeDispatchRequestInspection,
    ) -> Result<Self, io::Error> {
        let v = inspection.v;
        Ok(Self::Prepared {
            v,
            disposition,
            inspection: DispatchRequestInspectionView::new(inspection, false)?,
        })
    }

    pub fn request(
        inspection: GroupAgentNodeDispatchRequestInspection,
        include_request: bool,
    ) -> Result<Self, io::Error> {
        let v = inspection.v;
        Ok(Self::DispatchRequest {
            v,
            inspection: DispatchRequestInspectionView::new(inspection, include_request)?,
        })
    }

    pub fn list(records: Vec<GroupAgentNodeDispatchRequestRecord>) -> Self {
        let returned_requests_present = !records.is_empty();
        Self::DispatchRequests {
            v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
            metadata_only: true,
            source_contract_and_request_validated: false,
            returned_requests_present,
            request_preparation_validated: false,
            dispatch_authority_released: false,
            fresh_off_machine_consent_obtained: false,
            credential_read: false,
            execution_performed: false,
            model_used: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            result_produced: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            pricing_snapshot_identity_validated: false,
            pricing_policy_enforced: false,
            request_included: false,
            requests: records
                .into_iter()
                .map(DispatchRequestMetadataView::from)
                .collect(),
        }
    }
}

impl DispatchRequestInspectionView {
    fn new(
        inspection: GroupAgentNodeDispatchRequestInspection,
        include_request: bool,
    ) -> Result<Self, io::Error> {
        let provider_request_body = include_request
            .then(|| String::from_utf8(inspection.provider_request_body))
            .transpose()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validated provider request body is not UTF-8",
                )
            })?;
        Ok(Self {
            v: inspection.v,
            request_prepared: true,
            source_graph_validated: true,
            contract_and_journal_validated: true,
            request_body_validated: true,
            dispatch_authority_released: false,
            fresh_off_machine_consent_obtained: false,
            credential_read: false,
            execution_performed: false,
            model_selected: true,
            model_used: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            result_produced: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            pricing_snapshot_identity_pinned: true,
            pricing_policy_enforced: false,
            request_included: include_request,
            record: DispatchRequestMetadataView::from(inspection.record),
            provider_request_body,
        })
    }
}

impl From<GroupAgentNodeDispatchRequestRecord> for DispatchRequestMetadataView {
    fn from(record: GroupAgentNodeDispatchRequestRecord) -> Self {
        Self {
            v: record.v,
            dispatch_request_id: record.dispatch_request_id,
            graph_run_id: record.graph_run_id,
            contract_id: record.contract_id,
            attempt: record.attempt,
            provider_request_bytes: record.provider_request_bytes,
            codec_protocol_version: record.codec_protocol_version,
            created_at_ms: record.created_at_ms,
        }
    }
}

pub fn write_output(
    output: &GroupAgentNodeDispatchRequestCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    match output {
        GroupAgentNodeDispatchRequestCliOutput::Prepared {
            disposition,
            inspection,
            ..
        } => {
            writeln!(
                writer,
                "prepared Node Dispatch Request — {}",
                disposition_label(*disposition)
            )?;
            write_inspection(inspection, writer)
        }
        GroupAgentNodeDispatchRequestCliOutput::DispatchRequest { inspection, .. } => {
            write_inspection(inspection, writer)
        }
        GroupAgentNodeDispatchRequestCliOutput::DispatchRequests { requests, .. } => {
            write_list(requests, writer)
        }
    }
}

fn write_inspection(
    inspection: &DispatchRequestInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let record = &inspection.record;
    writeln!(
        writer,
        "dispatch_request {} · graph_run={} · contract={} · attempt={} · bytes={} · status=awaiting_dispatch_authorization",
        terminal_text(&record.dispatch_request_id),
        terminal_text(&record.graph_run_id),
        terminal_text(&record.contract_id),
        record.attempt,
        record.provider_request_bytes,
    )?;
    if let Some(request) = &inspection.provider_request_body {
        writeln!(writer, "provider request: {}", terminal_text(request))?;
    } else {
        writeln!(
            writer,
            "provider request hidden; use --include-request to reveal private exact bytes"
        )?;
    }
    write_boundaries(writer)
}

fn write_list(
    requests: &[DispatchRequestMetadataView],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Node Dispatch Requests: {} (metadata only; use show for source and request validation)",
        requests.len()
    )?;
    for request in requests {
        writeln!(
            writer,
            "{}\tgraph_run={}\tcontract={}\tattempt={}\tbytes={}\tcreated={}",
            terminal_text(&request.dispatch_request_id),
            terminal_text(&request.graph_run_id),
            terminal_text(&request.contract_id),
            request.attempt,
            request.provider_request_bytes,
            request.created_at_ms,
        )?;
    }
    write_list_boundaries(requests.is_empty(), writer)
}

fn write_list_boundaries(empty: bool, writer: &mut impl Write) -> Result<(), io::Error> {
    if empty {
        writeln!(
            writer,
            "no request metadata returned; preparation was not inferred"
        )?;
    } else {
        writeln!(
            writer,
            "metadata reports stored request rows; exact source, body, and pricing were not validated"
        )?;
    }
    writeln!(
        writer,
        "list released no dispatch authority and obtained no consent"
    )?;
    writeln!(
        writer,
        "list read no credential and used no provider/model/network"
    )?;
    writeln!(
        writer,
        "list performed no workspace/tool/result or writeback effect"
    )?;
    writeln!(writer, "list wrote no Conversation/Prompt/memory state")
}

fn write_boundaries(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "exact provider request prepared locally; dispatch authority not released"
    )?;
    writeln!(
        writer,
        "fresh off-machine consent absent; credential not read; provider/model not used"
    )?;
    writeln!(
        writer,
        "pricing identity pinned; pricing policy not enforced"
    )?;
    writeln!(
        writer,
        "no execution/provider/network/workspace/tools/result or writeback effect occurred"
    )?;
    writeln!(
        writer,
        "no Conversation/Prompt/memory write operation occurred"
    )
}

fn disposition_label(disposition: PrepareGroupAgentNodeDispatchRequestDisposition) -> &'static str {
    match disposition {
        PrepareGroupAgentNodeDispatchRequestDisposition::Created => "created",
        PrepareGroupAgentNodeDispatchRequestDisposition::Replayed => "replayed",
    }
}

#[cfg(test)]
#[path = "dispatch_output_tests.rs"]
mod tests;
