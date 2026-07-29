use std::io::{self, Write};

use serde::Serialize;

use crate::{
    group_context_output::terminal_text,
    runtime_domain::{
        BeginGroupExecutionDisposition, GroupExecutionInspection, GroupExecutionMode,
        GroupExecutionReceipt, GroupExecutionRecord, GroupExecutionRecovery, GroupExecutionStatus,
    },
};

#[derive(Debug, Serialize)]
pub struct GroupExecutionInspectionView {
    pub v: u16,
    pub execution: GroupExecutionRecord,
    pub event_count: usize,
    pub recovery: GroupExecutionRecovery,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<GroupExecutionReceipt>,
}

impl From<GroupExecutionInspection> for GroupExecutionInspectionView {
    fn from(inspection: GroupExecutionInspection) -> Self {
        Self {
            v: inspection.v,
            execution: inspection.execution,
            event_count: inspection.events.len(),
            recovery: inspection.recovery,
            receipt: inspection.receipt,
        }
    }
}

pub fn write_group_execution_started(
    disposition: BeginGroupExecutionDisposition,
    inspection: &GroupExecutionInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "snapshot-validation receipt {} — {}",
        terminal_text(&inspection.execution.execution_id),
        disposition_label(disposition)
    )?;
    write_inspection_summary(inspection, writer)
}

pub fn write_group_execution(
    inspection: &GroupExecutionInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "snapshot-validation receipt {} — {}",
        terminal_text(&inspection.execution.execution_id),
        status_label(inspection.execution.status)
    )?;
    write_inspection_summary(inspection, writer)
}

pub fn write_group_execution_list(
    executions: &[GroupExecutionRecord],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "local snapshot-validation executions: {}",
        executions.len()
    )?;
    for execution in executions {
        writeln!(
            writer,
            "{}\tgroup_run={}\tstatus={}\tcreated={}",
            terminal_text(&execution.execution_id),
            terminal_text(&execution.group_run_id),
            status_label(execution.status),
            execution.created_at_ms
        )?;
    }
    writeln!(
        writer,
        "status is recorded metadata; use group execution show EXECUTION_ID to validate source and journal"
    )?;
    write_execution_boundary(writer)
}

fn write_inspection_summary(
    inspection: &GroupExecutionInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let execution = &inspection.execution;
    writeln!(
        writer,
        "group_run={} · mode={} · status={} · events={}",
        terminal_text(&execution.group_run_id),
        mode_label(execution.mode),
        status_label(execution.status),
        inspection.event_count
    )?;
    writeln!(
        writer,
        "source snapshot sha256 {}",
        execution.source_snapshot_sha256
    )?;
    if let Some(receipt) = &inspection.receipt {
        write_receipt(receipt, writer)?;
    }
    write_integrity_state(inspection, writer)?;
    write_execution_boundary(writer)
}

fn write_receipt(
    receipt: &GroupExecutionReceipt,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "receipt: group={} · context_v={} · snapshot_bytes={}",
        terminal_text(&receipt.group_id),
        receipt.context_version,
        receipt.snapshot_bytes
    )?;
    writeln!(
        writer,
        "receipt stats: {} member(s) · {} conversation(s) · {} prompt(s) · {} content byte(s)",
        receipt.stats.member_count,
        receipt.stats.conversation_count,
        receipt.stats.prompt_count,
        receipt.stats.content_bytes
    )
}

fn write_integrity_state(
    inspection: &GroupExecutionInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let validated = inspection.receipt.is_some()
        && matches!(inspection.recovery, GroupExecutionRecovery::Terminal { .. });
    if validated {
        writeln!(writer, "frozen snapshot integrity: validated")
    } else {
        writeln!(writer, "frozen snapshot integrity: incomplete")
    }
}

fn write_execution_boundary(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(writer, "model/provider: not invoked")?;
    writeln!(writer, "analysis/discussion/task result: not produced")?;
    writeln!(writer, "workspace/tools/network: unavailable")
}

fn disposition_label(disposition: BeginGroupExecutionDisposition) -> &'static str {
    match disposition {
        BeginGroupExecutionDisposition::Created => "created",
        BeginGroupExecutionDisposition::Replayed => "idempotent replay",
    }
}

fn status_label(status: GroupExecutionStatus) -> &'static str {
    match status {
        GroupExecutionStatus::Incomplete => "incomplete",
        GroupExecutionStatus::Completed => "completed",
    }
}

fn mode_label(mode: GroupExecutionMode) -> &'static str {
    match mode {
        GroupExecutionMode::OfflineSnapshotValidation => "offline_snapshot_validation",
    }
}

#[cfg(test)]
mod tests {
    use super::write_group_execution_list;

    #[test]
    fn metadata_list_does_not_claim_it_revalidated_the_source_or_journal() {
        let mut output = Vec::new();

        write_group_execution_list(&[], &mut output).expect("write list");
        let text = String::from_utf8(output).expect("UTF-8 output");

        assert!(text.contains("status is recorded metadata"));
        assert!(!text.contains("frozen snapshot integrity: validated"));
    }
}
