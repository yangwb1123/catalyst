use std::{path::Path, sync::Arc};

use forge_runtime_application::HubService;
use forge_runtime_domain::RunInspection;
use forge_runtime_infrastructure::SqliteHubStore;

pub(crate) fn validate_project_binding(
    store: &Arc<SqliteHubStore>,
    run_id: &str,
    workspace: &Path,
    inspection: &RunInspection,
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = HubService::new(store.clone()).global_snapshot()?;
    if snapshot
        .projects
        .iter()
        .any(|project| project.id == inspection.run.project_id && project.path == workspace)
    {
        return Ok(());
    }
    Err(format!("selected Project does not match the Project bound to Run {run_id}").into())
}
