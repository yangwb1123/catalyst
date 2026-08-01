use std::io::{self, Write};

use serde::Serialize;

use crate::{
    group_context_output::terminal_text,
    runtime_domain::{
        GroupPanelSynthesisDispatchClaim, GroupPanelSynthesisInspection,
        GroupPanelSynthesisOutcome, GroupPanelSynthesisRecord, GroupPanelSynthesisRecovery,
        GroupPanelSynthesisResult, GroupPanelSynthesisResultReceipt, GroupPanelSynthesisSource,
        GroupPanelSynthesisStatus, PrepareGroupPanelSynthesisDisposition,
    },
};

#[derive(Debug, Serialize)]
pub struct GroupPanelSynthesisInspectionView {
    pub v: u16,
    #[serde(flatten)]
    pub safety: GroupPanelSynthesisSafetyView,
    pub synthesis: GroupPanelSynthesisRecord,
    pub recovery: GroupPanelSynthesisRecovery,
    pub source: GroupPanelSynthesisSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<GroupPanelSynthesisDispatchClaim>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<GroupPanelSynthesisResultReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<GroupPanelSynthesisResult>,
}

#[derive(Debug, Serialize)]
pub struct GroupPanelSynthesisListItemView {
    #[serde(flatten)]
    pub safety: GroupPanelSynthesisSafetyView,
    #[serde(flatten)]
    pub synthesis: GroupPanelSynthesisRecord,
}

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupPanelSynthesisSafetyView {
    pub single_model: bool,
    pub synthesis_performed: bool,
    pub discussion_performed: bool,
    pub consensus_reached: bool,
    pub factual_verification_performed: bool,
    pub tools_used: bool,
    pub workspace_accessed: bool,
    pub writeback_performed: bool,
    pub prompt_included: bool,
    pub input_included: bool,
    pub request_included: bool,
    pub panel_results_included: bool,
    pub result_included: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupPanelSynthesisSendDisposition {
    DispatchedAndCompleted,
    AlreadyClaimed,
}

impl GroupPanelSynthesisInspectionView {
    pub fn from_inspection(
        inspection: GroupPanelSynthesisInspection,
        include_result: bool,
    ) -> Self {
        let source = inspection
            .prepared
            .as_ref()
            .expect("validated synthesis has a prepared receipt")
            .source
            .clone();
        let synthesis_performed = matches!(
            inspection.recovery,
            GroupPanelSynthesisRecovery::Terminal { .. }
        );
        let result = include_result
            .then(|| inspection.result.map(|artifact| artifact.result))
            .flatten();
        Self {
            v: inspection.v,
            safety: GroupPanelSynthesisSafetyView::new(synthesis_performed, result.is_some()),
            synthesis: inspection.synthesis,
            recovery: inspection.recovery,
            source,
            dispatch: inspection.dispatch,
            completion: inspection.completion,
            result,
        }
    }
}

impl GroupPanelSynthesisListItemView {
    pub fn from_record(synthesis: GroupPanelSynthesisRecord) -> Self {
        Self {
            safety: GroupPanelSynthesisSafetyView::new(false, false),
            synthesis,
        }
    }
}

impl GroupPanelSynthesisSafetyView {
    const fn new(synthesis_performed: bool, result_included: bool) -> Self {
        Self {
            single_model: true,
            synthesis_performed,
            discussion_performed: false,
            consensus_reached: false,
            factual_verification_performed: false,
            tools_used: false,
            workspace_accessed: false,
            writeback_performed: false,
            prompt_included: false,
            input_included: false,
            request_included: false,
            panel_results_included: false,
            result_included,
        }
    }
}

pub fn write_prepared(
    disposition: PrepareGroupPanelSynthesisDisposition,
    inspection: &GroupPanelSynthesisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "prepared Group panel synthesis {} — {}",
        terminal_text(&inspection.synthesis.synthesis_id),
        prepare_disposition(disposition)
    )?;
    write_metadata(inspection, writer)?;
    writeln!(
        writer,
        "single-model synthesis: not performed; request remains local"
    )?;
    write_private_input(writer)?;
    write_honesty(writer)?;
    writeln!(
        writer,
        "send only with fresh consent: group synthesis send {} --confirm-off-machine",
        terminal_text(&inspection.synthesis.synthesis_id)
    )
}

pub fn write_synthesis(
    inspection: &GroupPanelSynthesisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Group panel synthesis {} — {}",
        terminal_text(&inspection.synthesis.synthesis_id),
        status_label(inspection.synthesis.status)
    )?;
    write_metadata(inspection, writer)?;
    write_recovery(&inspection.recovery, writer)?;
    write_optional_result(inspection, writer)?;
    write_private_input(writer)?;
    write_honesty(writer)
}

pub fn write_sent(
    disposition: GroupPanelSynthesisSendDisposition,
    inspection: &GroupPanelSynthesisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Group panel synthesis send {} — {}",
        terminal_text(&inspection.synthesis.synthesis_id),
        send_disposition(disposition)
    )?;
    write_metadata(inspection, writer)?;
    write_recovery(&inspection.recovery, writer)?;
    write_optional_result(inspection, writer)?;
    write_private_input(writer)?;
    write_honesty(writer)
}

pub fn write_list(
    items: &[GroupPanelSynthesisListItemView],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Group panel syntheses: {} (metadata only; source/journal not revalidated; use show for integrity validation)",
        items.len()
    )?;
    for item in items {
        let record = &item.synthesis;
        writeln!(
            writer,
            "{}\tpanel={}\tstatus={}\tmodel={}\tcreated={}",
            terminal_text(&record.synthesis_id),
            terminal_text(&record.panel_id),
            status_label(record.status),
            terminal_text(&record.config.model),
            record.created_at_ms
        )?;
    }
    Ok(())
}

fn write_metadata(
    inspection: &GroupPanelSynthesisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let synthesis = &inspection.synthesis;
    writeln!(
        writer,
        "source panel={} · group_run={} · analyses={} · request_bytes={}",
        terminal_text(&synthesis.panel_id),
        terminal_text(&synthesis.group_run_id),
        inspection.source.analysis_count,
        synthesis.request_bytes
    )?;
    writeln!(
        writer,
        "destination {} · model={}",
        terminal_text(&synthesis.config.endpoint),
        terminal_text(&synthesis.config.model)
    )?;
    writeln!(
        writer,
        "panel manifest sha256 {}",
        synthesis.panel_manifest_sha256
    )?;
    writeln!(writer, "request sha256 {}", synthesis.request_sha256)
}

fn write_recovery(
    recovery: &GroupPanelSynthesisRecovery,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    match recovery {
        GroupPanelSynthesisRecovery::Unprepared => {
            writeln!(writer, "recovery: invalid unprepared durable state")
        }
        GroupPanelSynthesisRecovery::AwaitingConsent => {
            writeln!(writer, "recovery: awaiting fresh off-machine consent")
        }
        GroupPanelSynthesisRecovery::DispatchUnknown { dispatch_id } => writeln!(
            writer,
            "recovery: dispatch outcome unknown ({}) — automatic resend is forbidden",
            terminal_text(dispatch_id)
        ),
        GroupPanelSynthesisRecovery::Terminal { outcome } => writeln!(
            writer,
            "recovery: terminal single-model synthesis ({})",
            outcome_label(*outcome)
        ),
    }
}

fn write_optional_result(
    inspection: &GroupPanelSynthesisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if let Some(result) = &inspection.result {
        writeln!(
            writer,
            "single-model synthesis ({}): {}",
            outcome_label(result.outcome),
            terminal_text(&result.answer)
        )
    } else if inspection.synthesis.status == GroupPanelSynthesisStatus::Completed {
        writeln!(
            writer,
            "synthesis text: hidden (pass --include-result to reveal)"
        )
    } else {
        writeln!(
            writer,
            "synthesis text: unavailable before a validated terminal result"
        )
    }
}

fn write_honesty(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "single model only: no discussion, consensus, factual verification, tools, workspace, or writeback"
    )
}

fn write_private_input(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "model Prompt, ordered panel input, and exact request: hidden"
    )
}

const fn status_label(status: GroupPanelSynthesisStatus) -> &'static str {
    match status {
        GroupPanelSynthesisStatus::AwaitingConsent => "awaiting consent",
        GroupPanelSynthesisStatus::DispatchUnknown => "dispatch unknown",
        GroupPanelSynthesisStatus::Completed => "completed",
    }
}

const fn outcome_label(outcome: GroupPanelSynthesisOutcome) -> &'static str {
    match outcome {
        GroupPanelSynthesisOutcome::Completed => "completed",
        GroupPanelSynthesisOutcome::Length => "length-limited",
    }
}

const fn prepare_disposition(disposition: PrepareGroupPanelSynthesisDisposition) -> &'static str {
    match disposition {
        PrepareGroupPanelSynthesisDisposition::Created => "created",
        PrepareGroupPanelSynthesisDisposition::Replayed => "idempotent replay",
    }
}

const fn send_disposition(disposition: GroupPanelSynthesisSendDisposition) -> &'static str {
    match disposition {
        GroupPanelSynthesisSendDisposition::DispatchedAndCompleted => {
            "dispatched once and completed"
        }
        GroupPanelSynthesisSendDisposition::AlreadyClaimed => {
            "already claimed; no request sent by this invocation"
        }
    }
}

#[cfg(test)]
#[path = "group_panel_synthesis_output_tests.rs"]
mod tests;
