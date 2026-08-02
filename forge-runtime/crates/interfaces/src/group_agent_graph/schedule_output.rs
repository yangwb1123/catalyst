use std::io::{self, Write};

use forge_runtime_application::{
    AdmitGroupAgentGraphExecutionScheduleDisposition, GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
    GroupAgentGraphExecutionSchedule, GroupAgentGraphExecutionScheduleInspection,
    GroupAgentGraphExecutionScheduleRecord,
};
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupAgentGraphExecutionScheduleCliOutput {
    #[serde(rename = "group_agent_graph_execution_schedule_admitted")]
    Admitted {
        v: u16,
        disposition: AdmitGroupAgentGraphExecutionScheduleDisposition,
        inspection: ScheduleInspectionView,
    },
    #[serde(rename = "group_agent_graph_execution_schedule")]
    Schedule {
        v: u16,
        inspection: ScheduleInspectionView,
    },
    #[serde(rename = "group_agent_graph_execution_schedules")]
    Schedules {
        v: u16,
        metadata_only: bool,
        returned_schedules_present: bool,
        schedule_included: bool,
        artifact_execution_contract_present: bool,
        artifact_dispatch_authority_released: bool,
        artifact_progress_observed: bool,
        artifact_successor_advanced: bool,
        current_run_lifecycle_included: bool,
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
        schedules: Vec<ScheduleMetadataView>,
    },
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScheduleInspectionView {
    v: u16,
    schedule_admitted: bool,
    source_graph_validated: bool,
    control_snapshot_validated: bool,
    schedule_validated: bool,
    passive_policy_only: bool,
    artifact_execution_contract_present: bool,
    artifact_dispatch_authority_released: bool,
    artifact_progress_observed: bool,
    artifact_successor_advanced: bool,
    current_run_lifecycle_included: bool,
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
    explicit_schedule_file_read: bool,
    schedule_included: bool,
    control_snapshot_included: bool,
    record: ScheduleMetadataView,
    #[serde(skip_serializing_if = "Option::is_none")]
    schedule: Option<GroupAgentGraphExecutionSchedule>,
}

#[derive(Serialize)]
pub struct ScheduleMetadataView {
    v: u16,
    schedule_id: String,
    graph_run_id: String,
    graph_id: String,
    control_snapshot_sha256: String,
    schedule_sha256: String,
    schedule_bytes: usize,
    node_count: usize,
    wave_count: usize,
    expected_last_event_seq: u64,
    expected_last_event_sha256: String,
    created_at_ms: u64,
}

impl GroupAgentGraphExecutionScheduleCliOutput {
    pub fn admitted(
        disposition: AdmitGroupAgentGraphExecutionScheduleDisposition,
        inspection: GroupAgentGraphExecutionScheduleInspection,
        explicit_schedule_file_read: bool,
    ) -> Self {
        Self::Admitted {
            v: inspection.v,
            disposition,
            inspection: ScheduleInspectionView::new(inspection, false, explicit_schedule_file_read),
        }
    }

    pub fn schedule(
        inspection: GroupAgentGraphExecutionScheduleInspection,
        include_schedule: bool,
    ) -> Self {
        Self::Schedule {
            v: inspection.v,
            inspection: ScheduleInspectionView::new(inspection, include_schedule, false),
        }
    }

    pub fn list(records: Vec<GroupAgentGraphExecutionScheduleRecord>) -> Self {
        let returned_schedules_present = !records.is_empty();
        Self::Schedules {
            v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
            metadata_only: true,
            returned_schedules_present,
            schedule_included: false,
            artifact_execution_contract_present: false,
            artifact_dispatch_authority_released: false,
            artifact_progress_observed: false,
            artifact_successor_advanced: false,
            current_run_lifecycle_included: false,
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
            schedules: records.into_iter().map(Into::into).collect(),
        }
    }
}

impl ScheduleInspectionView {
    fn new(
        inspection: GroupAgentGraphExecutionScheduleInspection,
        include_schedule: bool,
        explicit_schedule_file_read: bool,
    ) -> Self {
        Self {
            v: inspection.v,
            schedule_admitted: true,
            source_graph_validated: true,
            control_snapshot_validated: true,
            schedule_validated: true,
            passive_policy_only: true,
            artifact_execution_contract_present: false,
            artifact_dispatch_authority_released: false,
            artifact_progress_observed: false,
            artifact_successor_advanced: false,
            current_run_lifecycle_included: false,
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
            explicit_schedule_file_read,
            schedule_included: include_schedule,
            control_snapshot_included: false,
            record: ScheduleMetadataView::from(inspection.record),
            schedule: include_schedule.then_some(inspection.schedule),
        }
    }
}

impl From<GroupAgentGraphExecutionScheduleRecord> for ScheduleMetadataView {
    fn from(record: GroupAgentGraphExecutionScheduleRecord) -> Self {
        Self {
            v: record.v,
            schedule_id: record.schedule_id,
            graph_run_id: record.graph_run_id,
            graph_id: record.graph_id,
            control_snapshot_sha256: record.control_snapshot_sha256,
            schedule_sha256: record.schedule_sha256,
            schedule_bytes: record.schedule_bytes,
            node_count: record.node_count,
            wave_count: record.wave_count,
            expected_last_event_seq: record.expected_last_event_seq,
            expected_last_event_sha256: record.expected_last_event_sha256,
            created_at_ms: record.created_at_ms,
        }
    }
}

pub fn write_output(
    output: &GroupAgentGraphExecutionScheduleCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    match output {
        GroupAgentGraphExecutionScheduleCliOutput::Admitted {
            disposition,
            inspection,
            ..
        } => {
            writeln!(
                writer,
                "admitted Graph Execution Schedule — {}",
                disposition_label(*disposition)
            )?;
            write_inspection(inspection, writer)
        }
        GroupAgentGraphExecutionScheduleCliOutput::Schedule { inspection, .. } => {
            write_inspection(inspection, writer)
        }
        GroupAgentGraphExecutionScheduleCliOutput::Schedules { schedules, .. } => {
            write_list(schedules, writer)
        }
    }
}

fn write_inspection(
    inspection: &ScheduleInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "schedule {} · graph_run={} · nodes={} · waves={} · status=passive_policy_only",
        terminal_text(&inspection.record.schedule_id),
        terminal_text(&inspection.record.graph_run_id),
        inspection.record.node_count,
        inspection.record.wave_count
    )?;
    if let Some(schedule) = &inspection.schedule {
        let json = schedule
            .canonical_json()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        writeln!(writer, "schedule: {}", terminal_text(&json))?;
    } else {
        writeln!(
            writer,
            "schedule hidden; use --include-schedule to reveal node and lane identities"
        )?;
    }
    write_boundaries(writer)
}

fn write_list(
    schedules: &[ScheduleMetadataView],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Graph Execution Schedules: {} (metadata only; use show for full validation)",
        schedules.len()
    )?;
    for schedule in schedules {
        writeln!(
            writer,
            "{}\tgraph_run={}\tnodes={}\twaves={}\tcreated={}",
            terminal_text(&schedule.schedule_id),
            terminal_text(&schedule.graph_run_id),
            schedule.node_count,
            schedule.wave_count,
            schedule.created_at_ms
        )?;
    }
    write_boundaries(writer)
}

fn write_boundaries(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "passive scheduling artifact only; it records no progress or successor advance"
    )?;
    writeln!(
        writer,
        "the artifact records no execution contract or dispatch authority; current Run lifecycle is not reported"
    )?;
    writeln!(
        writer,
        "no credential/model/provider/network/workspace/tool/result effect"
    )?;
    writeln!(
        writer,
        "no Conversation/Prompt/memory/writeback operation occurred"
    )
}

fn disposition_label(
    disposition: AdmitGroupAgentGraphExecutionScheduleDisposition,
) -> &'static str {
    match disposition {
        AdmitGroupAgentGraphExecutionScheduleDisposition::Created => "created",
        AdmitGroupAgentGraphExecutionScheduleDisposition::Replayed => "replayed",
    }
}
