use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    sync::Arc,
};

use forge_runtime_application::{
    GROUP_AGENT_GRAPH_RUN_VERSION, GroupAgentGraphCorePlan, GroupAgentGraphRunService,
    MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES, PrepareGroupAgentGraphRunInput,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupGraphRunCommand},
    state_path::{hub_database_path, idempotency_key, unique_id, unix_time_millis},
};

use super::{
    contract_command::{self, GroupAgentGraphControlContractCliOutput},
    contract_output::{self, GroupAgentNodeExecutionContractCliOutput},
    dispatch_command::{self, GroupAgentGraphRunDispatchCommandCliOutput},
    run_output::{self, GroupAgentGraphRunCliOutput},
    schedule_command,
    schedule_output::{self, GroupAgentGraphExecutionScheduleCliOutput},
    scheduled_contract_command,
    scheduled_contract_output::{self, GroupAgentScheduledNodeContractCliOutput},
    scheduled_provider_request_command,
    scheduled_provider_request_output::{self, GroupAgentScheduledNodeProviderRequestCliOutput},
};

pub enum GroupAgentGraphRunCommandCliOutput {
    Run(Box<GroupAgentGraphRunCliOutput>),
    ControlSnapshot(String),
    Contract(Box<GroupAgentNodeExecutionContractCliOutput>),
    Dispatch(Box<GroupAgentGraphRunDispatchCommandCliOutput>),
    Schedule(Box<GroupAgentGraphExecutionScheduleCliOutput>),
    ScheduledContract(Box<GroupAgentScheduledNodeContractCliOutput>),
    ScheduledProviderRequest(Box<GroupAgentScheduledNodeProviderRequestCliOutput>),
}

pub async fn execute(
    args: &Args,
    command: &GroupGraphRunCommand,
) -> Result<GroupAgentGraphRunCommandCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunCommand::Prepare {
            graph_id,
            plan_source,
        } => execute_prepare(args, graph_id, plan_source),
        GroupGraphRunCommand::Show {
            graph_run_id,
            include_plan,
        } => {
            let service = service(args)?;
            Ok(run_output(GroupAgentGraphRunCliOutput::run(
                service.inspect(graph_run_id)?,
                *include_plan,
            )))
        }
        GroupGraphRunCommand::List { graph_id, limit } => {
            let service = service(args)?;
            Ok(run_output(GroupAgentGraphRunCliOutput::list(
                GROUP_AGENT_GRAPH_RUN_VERSION,
                service.list(graph_id.as_deref(), *limit)?,
            )))
        }
        GroupGraphRunCommand::Control(command) => Ok(aux_output(
            contract_command::execute_control(args, command)?,
        )),
        GroupGraphRunCommand::Contract(command) => Ok(aux_output(
            contract_command::execute_contract(args, command)?,
        )),
        GroupGraphRunCommand::Dispatch(command) => Ok(dispatch_cli_output(
            dispatch_command::execute(args, command).await?,
        )),
        GroupGraphRunCommand::Schedule(command) => Ok(schedule_cli_output(
            schedule_command::execute(args, command)?,
        )),
        GroupGraphRunCommand::ScheduledContract(
            crate::args::GroupGraphRunScheduledContractCommand::ProviderRequest(command),
        ) => execute_scheduled_provider_request(args, command),
        GroupGraphRunCommand::ScheduledContract(command) => {
            execute_scheduled_contract(args, command)
        }
    }
}

fn execute_prepare(
    args: &Args,
    graph_id: &str,
    plan_source: &str,
) -> Result<GroupAgentGraphRunCommandCliOutput, Box<dyn Error>> {
    let plan_json = read_plan(plan_source)?;
    let service = service(args)?;
    Ok(run_output(prepare(
        args,
        &service,
        graph_id,
        plan_json,
        plan_source != "-",
    )?))
}

pub fn write_output(
    output: &GroupAgentGraphRunCommandCliOutput,
    json: bool,
    writer: &mut impl io::Write,
) -> Result<(), io::Error> {
    match output {
        GroupAgentGraphRunCommandCliOutput::Run(output) => {
            run_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunCommandCliOutput::ControlSnapshot(snapshot) => {
            writer.write_all(snapshot.as_bytes())
        }
        GroupAgentGraphRunCommandCliOutput::Contract(output) => {
            contract_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunCommandCliOutput::Dispatch(output) => {
            dispatch_command::write_output(output, json, writer)
        }
        GroupAgentGraphRunCommandCliOutput::Schedule(output) => {
            schedule_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunCommandCliOutput::ScheduledContract(output) => {
            scheduled_contract_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunCommandCliOutput::ScheduledProviderRequest(output) => {
            scheduled_provider_request_output::write_output(output, json, writer)
        }
    }
}

fn run_output(output: GroupAgentGraphRunCliOutput) -> GroupAgentGraphRunCommandCliOutput {
    GroupAgentGraphRunCommandCliOutput::Run(Box::new(output))
}

fn schedule_cli_output(
    output: GroupAgentGraphExecutionScheduleCliOutput,
) -> GroupAgentGraphRunCommandCliOutput {
    GroupAgentGraphRunCommandCliOutput::Schedule(Box::new(output))
}

fn scheduled_contract_cli_output(
    output: GroupAgentScheduledNodeContractCliOutput,
) -> GroupAgentGraphRunCommandCliOutput {
    GroupAgentGraphRunCommandCliOutput::ScheduledContract(Box::new(output))
}

fn execute_scheduled_contract(
    args: &Args,
    command: &crate::args::GroupGraphRunScheduledContractCommand,
) -> Result<GroupAgentGraphRunCommandCliOutput, Box<dyn Error>> {
    Ok(scheduled_contract_cli_output(
        scheduled_contract_command::execute(args, command)?,
    ))
}

fn execute_scheduled_provider_request(
    args: &Args,
    command: &crate::args::GroupGraphRunScheduledContractProviderRequestCommand,
) -> Result<GroupAgentGraphRunCommandCliOutput, Box<dyn Error>> {
    Ok(
        GroupAgentGraphRunCommandCliOutput::ScheduledProviderRequest(Box::new(
            scheduled_provider_request_command::execute(args, command)?,
        )),
    )
}

fn dispatch_cli_output(
    output: GroupAgentGraphRunDispatchCommandCliOutput,
) -> GroupAgentGraphRunCommandCliOutput {
    GroupAgentGraphRunCommandCliOutput::Dispatch(Box::new(output))
}

fn aux_output(
    output: GroupAgentGraphControlContractCliOutput,
) -> GroupAgentGraphRunCommandCliOutput {
    match output {
        GroupAgentGraphControlContractCliOutput::ControlSnapshot(snapshot) => {
            GroupAgentGraphRunCommandCliOutput::ControlSnapshot(snapshot)
        }
        GroupAgentGraphControlContractCliOutput::Contract(output) => {
            GroupAgentGraphRunCommandCliOutput::Contract(output)
        }
    }
}

fn service(args: &Args) -> Result<GroupAgentGraphRunService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentGraphRunService::new(store.clone(), store))
}

fn prepare(
    args: &Args,
    service: &GroupAgentGraphRunService,
    graph_id: &str,
    plan_json: String,
    explicit_plan_file_read: bool,
) -> Result<GroupAgentGraphRunCliOutput, Box<dyn Error>> {
    let result = service.prepare(&PrepareGroupAgentGraphRunInput {
        graph_run_id: unique_id("group-agent-graph-run"),
        graph_id: graph_id.into(),
        plan_json,
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-graph-run")),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(GroupAgentGraphRunCliOutput::prepared(
        result.disposition,
        result.inspection,
        false,
        explicit_plan_file_read,
    ))
}

fn read_plan(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(io::stdin().lock())?
    } else {
        read_bounded(File::open(source)?)?
    };
    let text = String::from_utf8(bytes)
        .map_err(|_| invalid_input("Group Agent Graph Core Plan must be UTF-8"))?;
    let plan: GroupAgentGraphCorePlan = serde_json::from_str(&text)
        .map_err(|_| invalid_input("invalid Group Agent Graph Core Plan JSON"))?;
    plan.validate()
        .map_err(|_| invalid_input("invalid Group Agent Graph Core Plan"))?;
    let canonical = plan
        .canonical_json()
        .map_err(|_| invalid_input("invalid Group Agent Graph Core Plan"))?;
    if canonical != text {
        return Err(invalid_input("Group Agent Graph Core Plan is not canonical").into());
    }
    Ok(text)
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let limit = MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES
        .checked_add(1)
        .expect("Core Plan bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("Core Plan bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES {
        return Err(invalid_input(
            "Group Agent Graph Core Plan exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
