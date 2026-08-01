use std::{error::Error, sync::Arc};

use forge_runtime_application::{GroupAnalysisPanelService, PrepareGroupAnalysisPanelInput};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupPanelCommand},
    group_analysis_panel_output::GroupAnalysisPanelInspectionView,
    hub_output::{CliOutput, OutputKind},
    state_path::{hub_database_path, idempotency_key, unique_id, unix_time_millis},
};

pub fn execute(args: &Args, command: &GroupPanelCommand) -> Result<CliOutput, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service = GroupAnalysisPanelService::new(store.clone(), store.clone(), store);
    match command {
        GroupPanelCommand::Prepare {
            group_run_id,
            analysis_ids,
        } => prepare(args, &service, group_run_id, analysis_ids),
        GroupPanelCommand::Show {
            panel_id,
            include_results,
        } => show(&service, panel_id, *include_results),
        GroupPanelCommand::List {
            group_run_id,
            limit,
        } => list(&service, group_run_id.as_deref(), *limit),
    }
}

fn prepare(
    args: &Args,
    service: &GroupAnalysisPanelService,
    group_run_id: &str,
    analysis_ids: &[String],
) -> Result<CliOutput, Box<dyn Error>> {
    let result = service.prepare(&PrepareGroupAnalysisPanelInput {
        panel_id: unique_id("group-panel"),
        group_run_id: group_run_id.into(),
        analysis_ids: analysis_ids.to_vec(),
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-panel")),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(CliOutput::new(OutputKind::GroupAnalysisPanelPrepared {
        disposition: result.disposition,
        panel: GroupAnalysisPanelInspectionView::from_inspection(result.inspection, false),
    }))
}

fn show(
    service: &GroupAnalysisPanelService,
    panel_id: &str,
    include_results: bool,
) -> Result<CliOutput, Box<dyn Error>> {
    let inspection = service.inspect(panel_id)?;
    Ok(CliOutput::new(OutputKind::GroupAnalysisPanel {
        panel: GroupAnalysisPanelInspectionView::from_inspection(inspection, include_results),
    }))
}

fn list(
    service: &GroupAnalysisPanelService,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<CliOutput, Box<dyn Error>> {
    Ok(CliOutput::new(OutputKind::GroupAnalysisPanels {
        metadata_only: true,
        source_and_results_validated: false,
        inspect_with: "group panel show PANEL_ID",
        panels: service.list(group_run_id, limit)?,
    }))
}
