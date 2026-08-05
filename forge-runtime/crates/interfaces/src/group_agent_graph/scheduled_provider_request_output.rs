use std::io::{self, Write};

use forge_runtime_application::{
    GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
    GroupAgentScheduledNodeProviderRequestInspection, GroupAgentScheduledNodeProviderRequestRecord,
    PrepareGroupAgentScheduledNodeProviderRequestDisposition,
};
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeProviderRequestCliOutput {
    #[serde(rename = "group_agent_scheduled_node_provider_request_prepared")]
    Prepared {
        v: u16,
        disposition: PrepareGroupAgentScheduledNodeProviderRequestDisposition,
        inspection: ScheduledProviderRequestInspectionView,
    },
    #[serde(rename = "group_agent_scheduled_node_provider_request")]
    ProviderRequest {
        v: u16,
        inspection: ScheduledProviderRequestInspectionView,
    },
    #[serde(rename = "group_agent_scheduled_node_provider_requests")]
    ProviderRequests {
        v: u16,
        metadata_only: bool,
        source_and_request_validated: bool,
        current_run_state_included: bool,
        returned_requests_present: bool,
        provider_request_sidecar_rows_returned: bool,
        provider_request_preparation_validated: bool,
        fresh_off_machine_consent_obtained: bool,
        credential_read: bool,
        provider_constructed: bool,
        provider_used: bool,
        network_accessed: bool,
        workspace_accessed: bool,
        tools_used: bool,
        result_or_receipt_produced: bool,
        conversation_or_prompt_written: bool,
        memory_written: bool,
        writeback_performed: bool,
        request_included: bool,
        requests: Vec<ScheduledProviderRequestMetadataView>,
    },
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScheduledProviderRequestInspectionView {
    v: u16,
    passive_scheduled_provider_request: bool,
    source_graph_validated: bool,
    control_snapshot_validated: bool,
    stored_schedule_validated: bool,
    scheduled_contract_validated: bool,
    request_body_validated: bool,
    candidate_provider_request_present: bool,
    provider_request_sidecar_present: bool,
    current_run_dispatch_request_present: bool,
    current_run_lifecycle_included: bool,
    provider_request_sent: bool,
    lifecycle_contract_admitted: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    project_lane_claimed: bool,
    progress_observed: bool,
    successor_advance_authorized: bool,
    fresh_off_machine_consent_obtained: bool,
    credential_read: bool,
    provider_constructed: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    result_or_receipt_produced: bool,
    conversation_or_prompt_written: bool,
    memory_written: bool,
    writeback_performed: bool,
    pricing_snapshot_identity_pinned: bool,
    pricing_policy_enforced: bool,
    request_included: bool,
    record: ScheduledProviderRequestMetadataView,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_request_body: Option<String>,
}

#[derive(Serialize)]
pub struct ScheduledProviderRequestMetadataView {
    v: u16,
    provider_request_id: String,
    graph_run_id: String,
    scheduled_contract_id: String,
    execution_ordinal: usize,
    attempt: u16,
    provider_request_bytes: usize,
    codec_protocol_version: u16,
    created_at_ms: u64,
}

impl GroupAgentScheduledNodeProviderRequestCliOutput {
    pub fn prepared(
        disposition: PrepareGroupAgentScheduledNodeProviderRequestDisposition,
        inspection: GroupAgentScheduledNodeProviderRequestInspection,
    ) -> Result<Self, io::Error> {
        let v = inspection.v;
        Ok(Self::Prepared {
            v,
            disposition,
            inspection: ScheduledProviderRequestInspectionView::new(inspection, false)?,
        })
    }

    pub fn request(
        inspection: GroupAgentScheduledNodeProviderRequestInspection,
        include_request: bool,
    ) -> Result<Self, io::Error> {
        let v = inspection.v;
        Ok(Self::ProviderRequest {
            v,
            inspection: ScheduledProviderRequestInspectionView::new(inspection, include_request)?,
        })
    }

    pub fn list(records: Vec<GroupAgentScheduledNodeProviderRequestRecord>) -> Self {
        let returned_requests_present = !records.is_empty();
        Self::ProviderRequests {
            v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
            metadata_only: true,
            source_and_request_validated: false,
            current_run_state_included: false,
            returned_requests_present,
            provider_request_sidecar_rows_returned: returned_requests_present,
            provider_request_preparation_validated: false,
            fresh_off_machine_consent_obtained: false,
            credential_read: false,
            provider_constructed: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            result_or_receipt_produced: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            request_included: false,
            requests: records
                .into_iter()
                .map(ScheduledProviderRequestMetadataView::from)
                .collect(),
        }
    }
}

impl ScheduledProviderRequestInspectionView {
    fn new(
        inspection: GroupAgentScheduledNodeProviderRequestInspection,
        include_request: bool,
    ) -> Result<Self, io::Error> {
        let provider_request_body = include_request
            .then(|| String::from_utf8(inspection.provider_request_body))
            .transpose()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validated scheduled provider request body is not UTF-8",
                )
            })?;
        Ok(Self {
            v: inspection.v,
            passive_scheduled_provider_request: true,
            source_graph_validated: true,
            control_snapshot_validated: true,
            stored_schedule_validated: true,
            scheduled_contract_validated: true,
            request_body_validated: true,
            candidate_provider_request_present: false,
            provider_request_sidecar_present: true,
            current_run_dispatch_request_present: false,
            current_run_lifecycle_included: false,
            provider_request_sent: false,
            lifecycle_contract_admitted: false,
            execution_authority_released: false,
            dispatch_authority_released: false,
            project_lane_claimed: false,
            progress_observed: false,
            successor_advance_authorized: false,
            fresh_off_machine_consent_obtained: false,
            credential_read: false,
            provider_constructed: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            result_or_receipt_produced: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            pricing_snapshot_identity_pinned: true,
            pricing_policy_enforced: false,
            request_included: include_request,
            record: ScheduledProviderRequestMetadataView::from(inspection.record),
            provider_request_body,
        })
    }
}

impl From<GroupAgentScheduledNodeProviderRequestRecord> for ScheduledProviderRequestMetadataView {
    fn from(record: GroupAgentScheduledNodeProviderRequestRecord) -> Self {
        Self {
            v: record.v,
            provider_request_id: record.provider_request_id,
            graph_run_id: record.graph_run_id,
            scheduled_contract_id: record.scheduled_contract_id,
            execution_ordinal: record.execution_ordinal,
            attempt: record.attempt,
            provider_request_bytes: record.provider_request_bytes,
            codec_protocol_version: record.codec_protocol_version,
            created_at_ms: record.created_at_ms,
        }
    }
}

pub fn write_output(
    output: &GroupAgentScheduledNodeProviderRequestCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    match output {
        GroupAgentScheduledNodeProviderRequestCliOutput::Prepared {
            disposition,
            inspection,
            ..
        } => {
            writeln!(
                writer,
                "prepared passive scheduled-node provider request — {}",
                disposition_label(*disposition)
            )?;
            write_inspection(inspection, writer)
        }
        GroupAgentScheduledNodeProviderRequestCliOutput::ProviderRequest { inspection, .. } => {
            write_inspection(inspection, writer)
        }
        GroupAgentScheduledNodeProviderRequestCliOutput::ProviderRequests { requests, .. } => {
            write_list(requests, writer)
        }
    }
}

fn write_inspection(
    inspection: &ScheduledProviderRequestInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let record = &inspection.record;
    writeln!(
        writer,
        "provider_request {} · graph_run={} · scheduled_contract={} · ordinal={} · attempt={} · bytes={}",
        terminal_text(&record.provider_request_id),
        terminal_text(&record.graph_run_id),
        terminal_text(&record.scheduled_contract_id),
        record.execution_ordinal,
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
    write_boundaries(writer, true)
}

fn write_list(
    requests: &[ScheduledProviderRequestMetadataView],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Scheduled-node provider requests: {} (metadata only; use show for validation)",
        requests.len()
    )?;
    for request in requests {
        writeln!(
            writer,
            "{}\tgraph_run={}\tcontract={}\tordinal={}\tattempt={}\tbytes={}\tcreated={}",
            terminal_text(&request.provider_request_id),
            terminal_text(&request.graph_run_id),
            terminal_text(&request.scheduled_contract_id),
            request.execution_ordinal,
            request.attempt,
            request.provider_request_bytes,
            request.created_at_ms,
        )?;
    }
    if requests.is_empty() {
        writeln!(
            writer,
            "no request metadata returned; preparation was not inferred"
        )?;
    } else {
        writeln!(
            writer,
            "metadata reports sidecar rows; current sources and hidden bytes were not validated"
        )?;
    }
    write_boundaries(writer, false)
}

fn write_boundaries(
    writer: &mut impl Write,
    current_state_validated: bool,
) -> Result<(), io::Error> {
    if current_state_validated {
        writeln!(
            writer,
            "passive exact-byte sidecar only; current Run lifecycle and dispatch request are unchanged"
        )?;
    } else {
        writeln!(
            writer,
            "passive sidecar metadata only; current Run lifecycle and dispatch state were not inspected"
        )?;
    }
    writeln!(
        writer,
        "no consent, credential, provider, network, lane claim, execution, or dispatch authority"
    )?;
    writeln!(
        writer,
        "no workspace, tool, progress, result, receipt, successor, or writeback effect"
    )
}

fn disposition_label(
    disposition: PrepareGroupAgentScheduledNodeProviderRequestDisposition,
) -> &'static str {
    match disposition {
        PrepareGroupAgentScheduledNodeProviderRequestDisposition::Created => "created",
        PrepareGroupAgentScheduledNodeProviderRequestDisposition::Replayed => "replayed",
    }
}

use forge_runtime_application::ExecuteGroupAgentScheduledNodeDispatchResult;

use crate::runtime_domain::{
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodeLifecycleInspection, GroupAgentScheduledNodeLifecycleStatus,
};

/// Public CLI projection for the scheduled effectful lifecycle.
///
/// The projection deliberately contains no request, authorization, pricing,
/// credential, or Core-control bytes. The optional result is only emitted when
/// the caller explicitly asks for it.
#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeDispatchExecutionCliOutput {
    pub v: u16,
    pub r#type: &'static str,
    pub status: GroupAgentScheduledNodeLifecycleStatus,
    pub provider_request_id: String,
    pub graph_run_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub artifact_kind: Option<crate::runtime_domain::GroupAgentScheduledNodeTerminalArtifactKind>,
    pub classification: Option<GroupAgentNodeTerminalClassification>,
    pub outcome: Option<GroupAgentNodeTerminalOutcome>,
    pub provider_poll_started: bool,
    pub terminal_seen: bool,
    pub stream_eof_seen: bool,
    pub lane_active: bool,
    pub retry_authorized: bool,
    pub lane_release_authorized: bool,
    pub successor_advance_authorized: bool,
    pub dispatch_performed_this_invocation: bool,
    pub database_written_this_invocation: bool,
    pub metadata_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_text: Option<String>,
}

impl GroupAgentScheduledNodeDispatchExecutionCliOutput {
    pub fn from_result(
        result: ExecuteGroupAgentScheduledNodeDispatchResult,
        include_result: bool,
    ) -> Self {
        let (inspection, performed) = match result {
            ExecuteGroupAgentScheduledNodeDispatchResult::Terminalized(inspection) => {
                (inspection, true)
            }
            ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(inspection) => {
                (inspection, false)
            }
        };
        Self::from_inspection(&inspection, performed, include_result)
    }

    fn from_inspection(
        inspection: &GroupAgentScheduledNodeLifecycleInspection,
        performed: bool,
        include_result: bool,
    ) -> Self {
        let artifact = inspection.artifact.as_ref();
        let receipt = inspection.terminal_receipt.as_ref();
        let result_text = include_result
            .then(|| artifact.map(|value| value.output_text.clone()))
            .flatten();
        Self {
            v: inspection.v,
            r#type: "group_agent_scheduled_node_dispatch_execution",
            status: inspection.status,
            provider_request_id: inspection.claim.provider_request_id.clone(),
            graph_run_id: inspection.claim.graph_run_id.clone(),
            node_id: inspection.claim.node_id.clone(),
            attempt: inspection.claim.attempt,
            dispatch_id: inspection.claim.dispatch_id.clone(),
            artifact_kind: artifact.map(|value| value.artifact_kind),
            classification: artifact.map(|value| value.classification),
            outcome: receipt.map(|value| value.node_outcome),
            provider_poll_started: artifact.is_some_and(|value| value.provider_poll_started),
            terminal_seen: artifact.is_some_and(|value| value.terminal_seen),
            stream_eof_seen: artifact.is_some_and(|value| value.stream_eof_seen),
            lane_active: inspection.active_lane.is_some(),
            retry_authorized: artifact.is_some_and(|value| value.retry_authorized)
                || receipt.is_some_and(|value| value.retry_authorized),
            lane_release_authorized: receipt.is_some_and(|value| value.lane_release_authorized),
            successor_advance_authorized: receipt
                .is_some_and(|value| value.successor_advance_authorized),
            dispatch_performed_this_invocation: performed,
            database_written_this_invocation: performed,
            metadata_only: result_text.is_none(),
            result_text,
        }
    }
}

pub fn write_dispatch_execution_output(
    output: &GroupAgentScheduledNodeDispatchExecutionCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer(&mut *writer, output)?;
        writeln!(writer)
    } else {
        writeln!(
            writer,
            "scheduled graph dispatch {} · provider_request={} · graph_run={} · node={} · attempt={} · dispatch={} · lane_active={} · retry={}",
            status_text(output.status),
            terminal_text(&output.provider_request_id),
            terminal_text(&output.graph_run_id),
            terminal_text(&output.node_id),
            output.attempt,
            terminal_text(&output.dispatch_id),
            output.lane_active,
            output.retry_authorized,
        )?;
        if let Some(result) = &output.result_text {
            writeln!(writer, "result: {}", terminal_text(result))?;
        }
        Ok(())
    }
}

fn status_text(status: GroupAgentScheduledNodeLifecycleStatus) -> &'static str {
    match status {
        GroupAgentScheduledNodeLifecycleStatus::Claimed => "claimed",
        GroupAgentScheduledNodeLifecycleStatus::Terminalized => "terminalized",
        GroupAgentScheduledNodeLifecycleStatus::Quarantined => "quarantined",
    }
}
