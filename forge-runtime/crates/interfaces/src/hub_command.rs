use std::{
    error::Error,
    io::{self, Read},
    path::Path,
    sync::Arc,
};

use forge_runtime_application::{
    GroupExecutionService, GroupRunService, HubService, MAX_PROMPT_BYTES, RunService,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{
        Args, Command, GroupCommand, GroupExecutionCommand, GroupRunCommand, PromptCommand,
        RunCommand, SessionCommand,
    },
    group_context_output::GroupContextView,
    group_execution_output::GroupExecutionInspectionView,
    group_run_output::GroupRunSnapshotView,
    hub_output::{CliOutput, OutputKind, RemoteStatus},
    runtime_domain::{
        BeginGroupExecution, ConversationScope, GROUP_EXECUTION_VERSION, GROUP_RUN_VERSION,
        GroupContextPolicy, GroupExecutionMode, PrepareGroupRun,
    },
    state_path::{
        canonical_project, hub_database_path, idempotency_key, unique_id, unix_time_millis,
    },
};

pub fn execute(args: &Args) -> Result<CliOutput, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    if matches!(args.command, Command::HubStatus) {
        // Readiness probe: report BEFORE any store open, which would
        // migrate the hub. The probe never migrates or creates.
        return hub_status(args);
    }
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service = HubService::new(store.clone());
    let group_runs = GroupRunService::new(store.clone());
    let group_executions = GroupExecutionService::new(store.clone());
    match &args.command {
        Command::Hub => show_hub(&service, args.project.as_deref(), args.group.as_deref()),
        Command::HubStatus => hub_status(args),
        Command::Session(command) => execute_session(
            &service,
            args.project.as_deref(),
            args.group.as_deref(),
            args.idempotency_key.as_deref(),
            command,
        ),
        Command::Prompt(command) => {
            execute_prompt(&service, args.idempotency_key.as_deref(), command)
        }
        Command::Group(command) => execute_group(
            &service,
            &group_runs,
            &group_executions,
            args.idempotency_key.as_deref(),
            command,
        ),
        Command::Run(command) => execute_run(&RunService::new(store), command),
        Command::Demo(_) | Command::Help => Err("command is not a Hub operation".into()),
    }
}

fn execute_run(service: &RunService, command: &RunCommand) -> Result<CliOutput, Box<dyn Error>> {
    match command {
        RunCommand::List {
            conversation_id,
            limit,
        } => Ok(CliOutput::new(OutputKind::Runs {
            runs: service.list_runs(conversation_id.as_deref(), *limit)?,
        })),
        RunCommand::Show { run_id } => Ok(CliOutput::new(OutputKind::Run {
            inspection: service.inspect_run(run_id)?,
        })),
        RunCommand::Start { .. } => Err("run start must use the runtime execution path".into()),
    }
}

fn show_hub(
    service: &HubService,
    project: Option<&Path>,
    group: Option<&str>,
) -> Result<CliOutput, Box<dyn Error>> {
    let scope = selected_scope(service, project, group)?;
    let snapshot = match &scope {
        ConversationScope::Global => service.global_snapshot()?,
        ConversationScope::Project(id) => service.project_snapshot(id)?,
        ConversationScope::Group(id) => service.group_snapshot(id)?,
    };
    Ok(CliOutput::new(OutputKind::Hub {
        snapshot,
        remote: RemoteStatus::NotConfigured,
    }))
}

fn execute_session(
    service: &HubService,
    project: Option<&Path>,
    group: Option<&str>,
    supplied_key: Option<&str>,
    command: &SessionCommand,
) -> Result<CliOutput, Box<dyn Error>> {
    let scope = selected_scope(service, project, group)?;
    match command {
        SessionCommand::List => Ok(CliOutput::new(OutputKind::Sessions {
            sessions: service.list_sessions(&scope)?,
            scope,
        })),
        SessionCommand::New { title } => {
            let key = operation_key(supplied_key, "session");
            let session = service.create_session(&scope, title, &key)?;
            Ok(CliOutput::new(OutputKind::SessionCreated { session }))
        }
    }
}

fn execute_prompt(
    service: &HubService,
    supplied_key: Option<&str>,
    command: &PromptCommand,
) -> Result<CliOutput, Box<dyn Error>> {
    match command {
        PromptCommand::Add {
            conversation_id,
            text,
        } => {
            let content = prompt_content(text)?;
            let key = operation_key(supplied_key, "prompt");
            let prompt = service.append_prompt(conversation_id, "user", &content, &key)?;
            Ok(CliOutput::new(OutputKind::PromptAdded {
                prompt: prompt.into(),
            }))
        }
        PromptCommand::List {
            conversation_id,
            limit,
        } => Ok(CliOutput::new(OutputKind::Prompts {
            prompts: service.list_prompts(conversation_id.as_deref(), *limit)?,
        })),
    }
}

fn execute_group(
    service: &HubService,
    group_runs: &GroupRunService,
    group_executions: &GroupExecutionService,
    supplied_key: Option<&str>,
    command: &GroupCommand,
) -> Result<CliOutput, Box<dyn Error>> {
    match command {
        GroupCommand::Analysis(_) => {
            Err("group analysis must use the dedicated model-analysis path".into())
        }
        GroupCommand::Graph(_) => Err("group graph must use the dedicated Agent-graph path".into()),
        GroupCommand::Panel(_) => {
            Err("group panel must use the dedicated analysis-panel path".into())
        }
        GroupCommand::Synthesis(_) => Err("group synthesis requires its dedicated path".into()),
        GroupCommand::Create { name } => {
            let key = operation_key(supplied_key, "group");
            let group = service.create_group(name, &key)?;
            Ok(CliOutput::new(OutputKind::GroupCreated { group }))
        }
        GroupCommand::Add {
            group_id,
            project,
            role,
        } => {
            let canonical = canonical_project(project)?;
            let key = operation_key(supplied_key, "group-link");
            let member = service.add_project_path_to_group(group_id, &canonical, role, &key)?;
            Ok(CliOutput::new(OutputKind::GroupLinked { member }))
        }
        GroupCommand::Context {
            group_id,
            include_content,
            max_bytes,
        } => {
            let slice = service.group_context(group_id, *max_bytes)?;
            Ok(CliOutput::new(OutputKind::GroupContext {
                context: GroupContextView::from_slice(slice, *include_content),
            }))
        }
        GroupCommand::Execution(command) => {
            execute_group_execution(group_executions, supplied_key, command)
        }
        GroupCommand::Run(command) => execute_group_run(group_runs, supplied_key, command),
        GroupCommand::List => Ok(CliOutput::new(OutputKind::Groups {
            groups: service.list_groups()?,
        })),
    }
}

fn execute_group_execution(
    service: &GroupExecutionService,
    supplied_key: Option<&str>,
    command: &GroupExecutionCommand,
) -> Result<CliOutput, Box<dyn Error>> {
    match command {
        GroupExecutionCommand::Start { group_run_id } => {
            start_group_execution(service, supplied_key, group_run_id)
        }
        GroupExecutionCommand::Show { execution_id } => {
            let inspection = service.inspect(execution_id)?;
            Ok(CliOutput::new(OutputKind::GroupExecution {
                inspection: GroupExecutionInspectionView::from(inspection),
            }))
        }
        GroupExecutionCommand::List {
            group_run_id,
            limit,
        } => Ok(CliOutput::new(OutputKind::GroupExecutions {
            metadata_only: true,
            source_and_journal_validated: false,
            inspect_with: "group execution show EXECUTION_ID",
            executions: service.list(group_run_id.as_deref(), *limit)?,
        })),
    }
}

fn start_group_execution(
    service: &GroupExecutionService,
    supplied_key: Option<&str>,
    group_run_id: &str,
) -> Result<CliOutput, Box<dyn Error>> {
    let supplied_key = supplied_key.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "group execution start requires an explicit idempotency key for durable recovery",
        )
    })?;
    let result = service.start(&BeginGroupExecution {
        v: GROUP_EXECUTION_VERSION,
        execution_id: unique_id("group-execution"),
        group_run_id: group_run_id.to_owned(),
        mode: GroupExecutionMode::OfflineSnapshotValidation,
        idempotency_key: supplied_key.to_owned(),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(CliOutput::new(OutputKind::GroupExecutionStarted {
        disposition: result.disposition,
        inspection: GroupExecutionInspectionView::from(result.inspection),
    }))
}

fn execute_group_run(
    service: &GroupRunService,
    supplied_key: Option<&str>,
    command: &GroupRunCommand,
) -> Result<CliOutput, Box<dyn Error>> {
    match command {
        GroupRunCommand::Prepare {
            group_id,
            include_content,
            max_bytes,
        } => prepare_group_run(
            service,
            supplied_key,
            group_id,
            *max_bytes,
            *include_content,
        ),
        GroupRunCommand::Show {
            run_id,
            include_content,
        } => {
            let snapshot = service.inspect(run_id)?;
            Ok(CliOutput::new(OutputKind::GroupRun {
                snapshot: GroupRunSnapshotView::from_snapshot(snapshot, *include_content),
            }))
        }
        GroupRunCommand::List { group_id, limit } => Ok(CliOutput::new(OutputKind::GroupRuns {
            runs: service.list(group_id.as_deref(), *limit)?,
        })),
    }
}

fn prepare_group_run(
    service: &GroupRunService,
    supplied_key: Option<&str>,
    group_id: &str,
    max_bytes: usize,
    include_content: bool,
) -> Result<CliOutput, Box<dyn Error>> {
    let policy = GroupContextPolicy {
        max_total_content_bytes: max_bytes,
        ..GroupContextPolicy::default()
    };
    let result = service.prepare(&PrepareGroupRun {
        v: GROUP_RUN_VERSION,
        run_id: unique_id("group-run"),
        group_id: group_id.to_owned(),
        policy,
        idempotency_key: operation_key(supplied_key, "group-run"),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(CliOutput::new(OutputKind::GroupRunPrepared {
        disposition: result.disposition,
        snapshot: GroupRunSnapshotView::from_snapshot(result.snapshot, include_content),
    }))
}

fn open_project_scope(
    service: &HubService,
    project: &Path,
) -> Result<ConversationScope, Box<dyn Error>> {
    let canonical = canonical_project(project)?;
    let project = service.open_project(&canonical)?;
    Ok(ConversationScope::Project(project.id))
}

fn selected_scope(
    service: &HubService,
    project: Option<&Path>,
    group: Option<&str>,
) -> Result<ConversationScope, Box<dyn Error>> {
    if let Some(path) = project {
        return open_project_scope(service, path);
    }
    Ok(group.map_or(ConversationScope::Global, |id| {
        ConversationScope::Group(id.to_owned())
    }))
}

fn operation_key(supplied: Option<&str>, operation: &str) -> String {
    supplied.map_or_else(|| idempotency_key(operation), str::to_owned)
}

fn prompt_content(argument: &str) -> Result<String, io::Error> {
    if argument != "-" {
        return Ok(argument.to_owned());
    }
    let mut content = String::new();
    let limit = u64::try_from(MAX_PROMPT_BYTES + 1).expect("prompt limit fits in u64");
    io::stdin().take(limit).read_to_string(&mut content)?;
    Ok(content)
}

/// `hub_status` — readiness probe: reports the stored schema version, the
/// expected (current) version, whether a migration is pending, and the
/// number of pre-upgrade backups. Does NOT migrate or create the hub
/// (Stage-06 High follow-up).
fn hub_status(args: &Args) -> Result<CliOutput, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let stored = forge_runtime_infrastructure::hub_schema_version(&database)?;
    let expected = forge_runtime_infrastructure::CURRENT_SCHEMA_VERSION;
    let backups = database
        .parent()
        .map(|dir| dir.join("backups"))
        .filter(|dir| dir.is_dir())
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .map_or(0, |entries| entries.filter_map(Result::ok).count());
    Ok(CliOutput {
        v: 1,
        kind: OutputKind::HubStatus {
            schema_version: stored,
            expected_schema_version: expected,
            migration_pending: stored > 0 && stored < expected,
            backups,
            healthy: stored == expected || stored == 0,
        },
    })
}
