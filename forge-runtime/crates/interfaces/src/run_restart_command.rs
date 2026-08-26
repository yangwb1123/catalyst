use std::{error::Error, sync::Arc};

use forge_runtime_application::{PrepareRunRestart, RunService};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::Args,
    run_restart_output::RunRestartOutput,
    run_selection::validate_project_binding,
    state_path::{canonical_project, hub_database_path, unix_time_millis},
};

pub(crate) fn prepare(
    args: &Args,
    source_run_id: &str,
) -> Result<RunRestartOutput, Box<dyn Error>> {
    let selected = args
        .project
        .as_deref()
        .ok_or("run restart requires a Project")?;
    let key = args
        .idempotency_key
        .as_deref()
        .ok_or("run restart requires an explicit idempotency key")?;
    let workspace = canonical_project(selected)?;
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service = RunService::new(store.clone());
    let source = service.inspect_run(source_run_id)?;
    validate_project_binding(&store, source_run_id, &workspace, &source)?;
    let prepared = service.prepare_restart(&PrepareRunRestart {
        source_run_id: source_run_id.into(),
        idempotency_key: key.into(),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(RunRestartOutput::new(prepared))
}
