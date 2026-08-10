use super::{
    OutputKind, write_group_context, write_group_execution, write_group_execution_list,
    write_group_execution_started, write_group_model_kind, write_group_panel_kind, write_group_run,
    write_group_run_list, write_group_run_prepared, write_group_synthesis_kind, write_groups,
    write_run, write_runs,
};

pub(super) fn write(
    kind: &OutputKind,
    writer: &mut impl std::io::Write,
) -> Result<(), std::io::Error> {
    match kind {
        OutputKind::GroupContext { context } => write_group_context(context, writer),
        OutputKind::GroupRunPrepared {
            disposition,
            snapshot,
        } => write_group_run_prepared(*disposition, snapshot, writer),
        OutputKind::GroupRun { snapshot } => write_group_run(snapshot, writer),
        OutputKind::GroupRuns { runs } => write_group_run_list(runs, writer),
        OutputKind::GroupExecutionStarted {
            disposition,
            inspection,
        } => write_group_execution_started(*disposition, inspection, writer),
        OutputKind::GroupExecution { inspection } => write_group_execution(inspection, writer),
        OutputKind::GroupExecutions { executions, .. } => {
            write_group_execution_list(executions, writer)
        }
        OutputKind::GroupModelAnalysisPrepared { .. }
        | OutputKind::GroupModelAnalysisSent { .. }
        | OutputKind::GroupModelAnalysis { .. }
        | OutputKind::GroupModelAnalyses { .. } => write_group_model_kind(kind, writer),
        OutputKind::GroupAnalysisPanelPrepared { .. }
        | OutputKind::GroupAnalysisPanel { .. }
        | OutputKind::GroupAnalysisPanels { .. } => write_group_panel_kind(kind, writer),
        OutputKind::GroupPanelSynthesisPrepared { .. }
        | OutputKind::GroupPanelSynthesisSent { .. }
        | OutputKind::GroupPanelSynthesis { .. }
        | OutputKind::GroupPanelSyntheses { .. } => write_group_synthesis_kind(kind, writer),
        OutputKind::Groups { groups } => write_groups(groups, writer),
        OutputKind::Runs { runs } => write_runs(runs, writer),
        OutputKind::Run { inspection } => write_run(inspection, writer),
        _ => unreachable!("non-group output routed to group writer"),
    }
}
