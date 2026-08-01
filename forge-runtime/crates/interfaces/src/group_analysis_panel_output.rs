use std::io::{self, Write};

use serde::Serialize;

use crate::{
    group_context_output::terminal_text,
    runtime_domain::{
        GroupAnalysisPanelInspection, GroupAnalysisPanelRecord, GroupModelAnalysisOutcome,
        GroupModelAnalysisRecord, GroupModelAnalysisResult, GroupModelAnalysisSource,
        PrepareGroupAnalysisPanelDisposition,
    },
};

#[derive(Debug, Serialize)]
pub struct GroupAnalysisPanelInspectionView {
    pub v: u16,
    pub assembly_only: bool,
    pub synthesis_performed: bool,
    pub results_included: bool,
    pub panel: GroupAnalysisPanelRecord,
    pub source: GroupModelAnalysisSource,
    pub contributions: Vec<GroupAnalysisPanelContributionView>,
}

#[derive(Debug, Serialize)]
pub struct GroupAnalysisPanelContributionView {
    pub analysis: GroupModelAnalysisRecord,
    pub outcome: GroupModelAnalysisOutcome,
    pub result_sha256: String,
    pub result_bytes: usize,
    pub completed_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<GroupModelAnalysisResult>,
}

impl GroupAnalysisPanelInspectionView {
    pub fn from_inspection(
        inspection: GroupAnalysisPanelInspection,
        include_results: bool,
    ) -> Self {
        let contributions = inspection
            .manifest
            .contributions
            .into_iter()
            .map(|contribution| GroupAnalysisPanelContributionView {
                analysis: contribution.analysis,
                outcome: contribution.result.result.outcome,
                result_sha256: contribution.result.result_sha256,
                result_bytes: contribution.result.result_bytes,
                completed_at_ms: contribution.result.created_at_ms,
                result: include_results.then_some(contribution.result.result),
            })
            .collect();
        Self {
            v: inspection.v,
            assembly_only: true,
            synthesis_performed: false,
            results_included: include_results,
            panel: inspection.panel,
            source: inspection.manifest.source,
            contributions,
        }
    }
}

pub fn write_prepared(
    disposition: PrepareGroupAnalysisPanelDisposition,
    panel: &GroupAnalysisPanelInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "prepared local Group analysis panel {} — {}",
        terminal_text(&panel.panel.panel_id),
        disposition_label(disposition)
    )?;
    write_panel(panel, writer)
}

pub fn write_panel(
    panel: &GroupAnalysisPanelInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "source group_run={} · analyses={} · manifest_bytes={}",
        terminal_text(&panel.panel.group_run_id),
        panel.panel.analysis_count,
        panel.panel.manifest_bytes
    )?;
    writeln!(writer, "manifest sha256 {}", panel.panel.manifest_sha256)?;
    write_contributions(&panel.contributions, writer)?;
    writeln!(
        writer,
        "assembly only: no synthesis, discussion, consensus, model, tools, workspace, or writeback"
    )
}

pub fn write_list(
    panels: &[GroupAnalysisPanelRecord],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Group analysis panels: {} (metadata only; use show for integrity validation)",
        panels.len()
    )?;
    for panel in panels {
        writeln!(
            writer,
            "{}\tgroup_run={}\tanalyses={}\tcreated={}",
            terminal_text(&panel.panel_id),
            terminal_text(&panel.group_run_id),
            panel.analysis_count,
            panel.created_at_ms
        )?;
    }
    Ok(())
}

fn write_contributions(
    contributions: &[GroupAnalysisPanelContributionView],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    for (position, contribution) in contributions.iter().enumerate() {
        writeln!(
            writer,
            "{}. analysis={} · model={} · outcome={} · result_bytes={}",
            position + 1,
            terminal_text(&contribution.analysis.analysis_id),
            terminal_text(&contribution.analysis.config.model),
            outcome_label(contribution.outcome),
            contribution.result_bytes
        )?;
        if let Some(result) = &contribution.result {
            writeln!(writer, "   result: {}", terminal_text(&result.answer))?;
        }
    }
    Ok(())
}

fn disposition_label(disposition: PrepareGroupAnalysisPanelDisposition) -> &'static str {
    match disposition {
        PrepareGroupAnalysisPanelDisposition::Created => "created",
        PrepareGroupAnalysisPanelDisposition::Replayed => "replayed",
    }
}

fn outcome_label(outcome: GroupModelAnalysisOutcome) -> &'static str {
    match outcome {
        GroupModelAnalysisOutcome::Completed => "completed",
        GroupModelAnalysisOutcome::Length => "length",
    }
}
