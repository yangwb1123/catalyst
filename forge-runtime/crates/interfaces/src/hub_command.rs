use std::{
    error::Error,
    io::{self, Read},
    path::Path,
    sync::Arc,
};

use forge_runtime_application::{HubService, MAX_PROMPT_BYTES};
use forge_runtime_domain::ConversationScope;
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, Command, GroupCommand, PromptCommand, SessionCommand},
    hub_output::{CliOutput, OutputKind, RemoteStatus},
    state_path::{canonical_project, hub_database_path, idempotency_key},
};

pub fn execute(args: &Args) -> Result<CliOutput, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let service = HubService::new(Arc::new(SqliteHubStore::open(database)?));
    match &args.command {
        Command::Hub => show_hub(&service, args.project.as_deref(), args.group.as_deref()),
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
        Command::Group(command) => {
            execute_group(&service, args.idempotency_key.as_deref(), command)
        }
        Command::Demo(_) | Command::Help => Err("command is not a Hub operation".into()),
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
    supplied_key: Option<&str>,
    command: &GroupCommand,
) -> Result<CliOutput, Box<dyn Error>> {
    match command {
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
        GroupCommand::List => Ok(CliOutput::new(OutputKind::Groups {
            groups: service.list_groups()?,
        })),
    }
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
