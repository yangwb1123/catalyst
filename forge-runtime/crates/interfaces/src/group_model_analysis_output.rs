use std::io::{self, Write};

use serde::Serialize;

use crate::{
    group_context_output::terminal_text,
    runtime_domain::{
        GroupModelAnalysisDispatchClaim, GroupModelAnalysisInspection, GroupModelAnalysisOutcome,
        GroupModelAnalysisRecord, GroupModelAnalysisRecovery, GroupModelAnalysisResult,
        GroupModelAnalysisResultReceipt, GroupModelAnalysisSource, GroupModelAnalysisStatus,
        PrepareGroupModelAnalysisDisposition,
    },
};

#[derive(Debug, Serialize)]
pub struct GroupModelAnalysisInspectionView {
    pub v: u16,
    pub analysis: GroupModelAnalysisRecord,
    pub recovery: GroupModelAnalysisRecovery,
    pub source: GroupModelAnalysisSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<GroupModelAnalysisDispatchClaim>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<GroupModelAnalysisResultReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<GroupModelAnalysisResult>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupModelAnalysisSendDisposition {
    DispatchedAndCompleted,
    AlreadyClaimed,
}

impl GroupModelAnalysisInspectionView {
    pub fn from_inspection(inspection: GroupModelAnalysisInspection, include_result: bool) -> Self {
        let source = inspection
            .prepared
            .expect("validated durable analysis has a prepared receipt")
            .source;
        let result = include_result
            .then(|| inspection.result.map(|artifact| artifact.result))
            .flatten();
        Self {
            v: inspection.v,
            analysis: inspection.analysis,
            recovery: inspection.recovery,
            source,
            dispatch: inspection.dispatch,
            completion: inspection.completion,
            result,
        }
    }
}

pub fn write_prepared(
    disposition: PrepareGroupModelAnalysisDisposition,
    inspection: &GroupModelAnalysisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "prepared group model analysis {} — {}",
        terminal_text(&inspection.analysis.analysis_id),
        prepare_disposition(disposition)
    )?;
    write_metadata(inspection, writer)?;
    writeln!(
        writer,
        "provider dispatch: not released; frozen dossier remains local"
    )?;
    writeln!(
        writer,
        "send only with explicit consent: group analysis send {} --confirm-off-machine",
        terminal_text(&inspection.analysis.analysis_id)
    )
}

pub fn write_analysis(
    inspection: &GroupModelAnalysisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "group model analysis {} — {}",
        terminal_text(&inspection.analysis.analysis_id),
        status_label(inspection.analysis.status)
    )?;
    write_metadata(inspection, writer)?;
    write_recovery(&inspection.recovery, writer)?;
    write_optional_result(
        inspection.analysis.status,
        inspection.result.as_ref(),
        writer,
    )
}

pub fn write_sent(
    disposition: GroupModelAnalysisSendDisposition,
    inspection: &GroupModelAnalysisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "group model analysis send {} — {}",
        terminal_text(&inspection.analysis.analysis_id),
        send_disposition(disposition)
    )?;
    write_metadata(inspection, writer)?;
    write_recovery(&inspection.recovery, writer)?;
    write_optional_result(
        inspection.analysis.status,
        inspection.result.as_ref(),
        writer,
    )
}

pub fn write_list(
    analyses: &[GroupModelAnalysisRecord],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "group model analyses: {} (metadata only; use show for integrity validation)",
        analyses.len()
    )?;
    for analysis in analyses {
        writeln!(
            writer,
            "{}\tgroup_run={}\tstatus={}\tmodel={}\tcreated={}",
            terminal_text(&analysis.analysis_id),
            terminal_text(&analysis.group_run_id),
            status_label(analysis.status),
            terminal_text(&analysis.config.model),
            analysis.created_at_ms
        )?;
    }
    Ok(())
}

fn write_metadata(
    inspection: &GroupModelAnalysisInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let analysis = &inspection.analysis;
    writeln!(
        writer,
        "source group_run={} · snapshot_bytes={} · request_bytes={}",
        terminal_text(&analysis.group_run_id),
        inspection.source.snapshot_bytes,
        analysis.request_bytes
    )?;
    writeln!(
        writer,
        "destination {} · model={}",
        terminal_text(&analysis.config.endpoint),
        terminal_text(&analysis.config.model)
    )?;
    writeln!(
        writer,
        "snapshot sha256 {}",
        analysis.source_snapshot_sha256
    )?;
    writeln!(writer, "request sha256 {}", analysis.request_sha256)
}

fn write_recovery(
    recovery: &GroupModelAnalysisRecovery,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    match recovery {
        GroupModelAnalysisRecovery::Unprepared => {
            writeln!(writer, "recovery: invalid unprepared durable state")
        }
        GroupModelAnalysisRecovery::AwaitingConsent => {
            writeln!(writer, "recovery: awaiting explicit off-machine consent")
        }
        GroupModelAnalysisRecovery::DispatchUnknown { dispatch_id } => writeln!(
            writer,
            "recovery: dispatch outcome unknown ({}) — automatic resend is forbidden",
            terminal_text(dispatch_id)
        ),
        GroupModelAnalysisRecovery::Terminal { outcome } => writeln!(
            writer,
            "recovery: terminal single-model result ({})",
            outcome_label(*outcome)
        ),
    }
}

fn write_optional_result(
    status: GroupModelAnalysisStatus,
    result: Option<&GroupModelAnalysisResult>,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if let Some(result) = result {
        writeln!(
            writer,
            "model-generated analysis ({}): {}",
            outcome_label(result.outcome),
            terminal_text(&result.answer)
        )
    } else if status == GroupModelAnalysisStatus::Completed {
        writeln!(
            writer,
            "result text: hidden (pass --include-result to reveal)"
        )
    } else {
        writeln!(
            writer,
            "result text: unavailable before a validated terminal result"
        )
    }
}

const fn status_label(status: GroupModelAnalysisStatus) -> &'static str {
    match status {
        GroupModelAnalysisStatus::AwaitingConsent => "awaiting consent",
        GroupModelAnalysisStatus::DispatchUnknown => "dispatch unknown",
        GroupModelAnalysisStatus::Completed => "completed",
    }
}

const fn outcome_label(outcome: GroupModelAnalysisOutcome) -> &'static str {
    match outcome {
        GroupModelAnalysisOutcome::Completed => "completed",
        GroupModelAnalysisOutcome::Length => "length-limited",
    }
}

const fn prepare_disposition(disposition: PrepareGroupModelAnalysisDisposition) -> &'static str {
    match disposition {
        PrepareGroupModelAnalysisDisposition::Created => "created",
        PrepareGroupModelAnalysisDisposition::Replayed => "idempotent replay",
    }
}

const fn send_disposition(disposition: GroupModelAnalysisSendDisposition) -> &'static str {
    match disposition {
        GroupModelAnalysisSendDisposition::DispatchedAndCompleted => {
            "dispatched once and completed"
        }
        GroupModelAnalysisSendDisposition::AlreadyClaimed => {
            "already claimed; no request sent by this invocation"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GroupModelAnalysisInspectionView, write_analysis};
    use crate::runtime_domain::{
        GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION, GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT,
        GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION, GROUP_MODEL_ANALYSIS_VERSION,
        GroupModelAnalysisConfig, GroupModelAnalysisInspection, GroupModelAnalysisProvider,
        GroupModelAnalysisRecord, GroupModelAnalysisRecovery, GroupModelAnalysisResult,
        GroupModelAnalysisResultArtifact, GroupModelAnalysisSource, GroupModelAnalysisStatus,
        Usage,
    };

    #[test]
    fn default_view_hides_result_and_private_journal_material() {
        let view = GroupModelAnalysisInspectionView::from_inspection(fixture(), false);
        let json = serde_json::to_string(&view).expect("view JSON");

        for secret in [
            "secret result",
            "system prompt",
            "request body",
            "\"events\"",
        ] {
            assert!(!json.contains(secret), "view leaked {secret}");
        }
    }

    #[test]
    fn included_result_is_terminal_escaped_for_human_output() {
        let view = GroupModelAnalysisInspectionView::from_inspection(fixture(), true);
        let mut output = Vec::new();
        write_analysis(&view, &mut output).expect("human output");
        let text = String::from_utf8(output).expect("UTF-8");

        assert!(text.contains("secret result\\n\\x1b[2J"));
        assert!(!text.contains('\u{1b}'));
    }

    fn fixture() -> GroupModelAnalysisInspection {
        let analysis = record();
        GroupModelAnalysisInspection {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            analysis,
            events: vec![],
            recovery: GroupModelAnalysisRecovery::Terminal {
                outcome: crate::runtime_domain::GroupModelAnalysisOutcome::Completed,
            },
            prepared: Some(crate::runtime_domain::GroupModelAnalysisPreparedReceipt {
                v: GROUP_MODEL_ANALYSIS_VERSION,
                analysis_id: "analysis-1".into(),
                source: source(),
                config_sha256: "33".repeat(32),
                request_sha256: "44".repeat(32),
                request_bytes: 100,
            }),
            dispatch: None,
            completion: None,
            result: Some(result()),
        }
    }

    fn record() -> GroupModelAnalysisRecord {
        GroupModelAnalysisRecord {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            analysis_id: "analysis-1".into(),
            group_run_id: "group-run-1".into(),
            status: GroupModelAnalysisStatus::Completed,
            source_snapshot_sha256: "22".repeat(32),
            config: config(),
            config_sha256: "33".repeat(32),
            request_sha256: "44".repeat(32),
            request_bytes: 100,
            protocol_version: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
            created_at_ms: 10,
        }
    }

    fn config() -> GroupModelAnalysisConfig {
        GroupModelAnalysisConfig {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            provider: GroupModelAnalysisProvider::OpenAiResponses,
            endpoint: GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT.into(),
            model: "test-model".into(),
            system_prompt_version: GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION,
            system_prompt_sha256: "55".repeat(32),
            max_output_tokens: 4_096,
            max_model_output_bytes: 8_192,
            max_model_events: 100,
        }
    }

    fn source() -> GroupModelAnalysisSource {
        GroupModelAnalysisSource {
            group_run_version: 1,
            group_run_id: "group-run-1".into(),
            group_id: "group-1".into(),
            context_version: 1,
            context_slice_sha256: "11".repeat(32),
            snapshot_sha256: "22".repeat(32),
            snapshot_bytes: 200,
        }
    }

    fn result() -> GroupModelAnalysisResultArtifact {
        GroupModelAnalysisResultArtifact {
            result: GroupModelAnalysisResult {
                v: 1,
                analysis_id: "analysis-1".into(),
                dispatch_id: "dispatch-1".into(),
                request_sha256: "44".repeat(32),
                outcome: crate::runtime_domain::GroupModelAnalysisOutcome::Completed,
                answer: "secret result\n\u{1b}[2J".into(),
                usage: Usage::default(),
            },
            result_sha256: "66".repeat(32),
            result_bytes: 200,
            created_at_ms: 12,
        }
    }
}
