use std::{error::Error, sync::Arc};

use forge_runtime_application::{PrepareRunBranch, RunService};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::Args,
    run_branch_output::RunBranchOutput,
    run_selection::validate_project_binding,
    state_path::{canonical_project, hub_database_path, unix_time_millis},
};

pub(crate) fn prepare(args: &Args, parent_run_id: &str) -> Result<RunBranchOutput, Box<dyn Error>> {
    let selected = args
        .project
        .as_deref()
        .ok_or("run branch requires a Project")?;
    let key = args
        .idempotency_key
        .as_deref()
        .ok_or("run branch requires an explicit idempotency key")?;
    let workspace = canonical_project(selected)?;
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service = RunService::new(store.clone());
    let parent = service.inspect_run(parent_run_id)?;
    validate_project_binding(&store, parent_run_id, &workspace, &parent)?;
    let prepared = service.prepare_branch(&PrepareRunBranch {
        parent_run_id: parent_run_id.into(),
        idempotency_key: key.into(),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(RunBranchOutput::new(prepared))
}
