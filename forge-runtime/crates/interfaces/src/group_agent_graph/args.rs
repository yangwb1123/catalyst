//! Argument parsing for the explicit-ID Group Agent Graph management surface.

use std::collections::VecDeque;

#[path = "dispatch_execute_args.rs"]
mod dispatch_execute_args;
#[path = "ready_release_args.rs"]
mod ready_release_args;
#[path = "reconcile_args.rs"]
mod reconcile_args;
#[path = "scheduled_contract_args.rs"]
mod scheduled_contract_args;
#[path = "scheduled_release/args.rs"]
mod scheduled_release_args;

use crate::{
    group_context_output::terminal_text,
    runtime_domain::{
        MAX_GROUP_AGENT_GRAPH_LIST_LIMIT, MAX_GROUP_AGENT_NODE_DISPATCH_REQUEST_LIST_LIMIT,
    },
};

use super::{
    Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand, GroupGraphRunContractCommand,
    GroupGraphRunControlCommand, GroupGraphRunDispatchCommand, next_value, require_empty, usage,
};

pub(super) fn parse(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("prepare") => parse_prepare(tokens, idempotency_key),
        Some("run") => parse_run(tokens, idempotency_key),
        Some("show") => parse_show(tokens),
        Some("list") => parse_list(tokens),
        Some(value) => Err(with_usage(&format!(
            "unknown group graph command '{}'",
            terminal_text(value)
        ))),
        None => Err(with_usage("group graph command is required")),
    }
}

fn parse_run(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("prepare") => parse_run_prepare(tokens, idempotency_key),
        Some("control") => parse_run_control(tokens),
        Some("contract") => parse_run_contract(tokens, idempotency_key),
        Some("dispatch") => parse_run_dispatch(tokens, idempotency_key),
        Some("schedule") => {
            crate::group_agent_graph::schedule_command::parse_schedule(tokens, idempotency_key)
        }
        Some("scheduled-contract") => scheduled_contract_args::parse(tokens, idempotency_key),
        Some("reconcile") => reconcile_args::parse(tokens),
        Some("ready-release") => ready_release_args::parse(tokens),
        Some("show") => parse_run_show(tokens),
        Some("list") => parse_run_list(tokens),
        Some(value) => Err(with_usage(&format!(
            "unknown group graph run command '{}'",
            terminal_text(value)
        ))),
        None => Err(with_usage("group graph run command is required")),
    }
}

fn parse_run_dispatch(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("prepare") => parse_dispatch_prepare(tokens, idempotency_key),
        Some("show") => parse_dispatch_show(tokens),
        Some("list") => parse_dispatch_list(tokens),
        Some("release-control") => parse_dispatch_release_control(tokens),
        Some("authorization") => parse_dispatch_authorization(tokens),
        Some("readiness") => dispatch_execute_args::parse_readiness(tokens),
        Some("execute") => dispatch_execute_args::parse(tokens),
        Some("adjudicate") => dispatch_execute_args::parse_adjudicate(tokens),
        Some(_) => Err(unknown_dispatch("group graph run dispatch")),
        None => Err(with_usage("group graph run dispatch command is required")),
    }
}

fn parse_dispatch_release_control(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("export") => {
            let graph_run_id = required_id(
                tokens,
                "group graph run dispatch release-control export",
                "GRAPH_RUN_ID",
            )?;
            require_empty(tokens)?;
            Ok(run_command(GroupGraphRunCommand::Dispatch(
                GroupGraphRunDispatchCommand::ReleaseControlExport { graph_run_id },
            )))
        }
        Some(_) => Err(unknown_dispatch("group graph run dispatch release-control")),
        None => Err(with_usage(
            "group graph run dispatch release-control command is required",
        )),
    }
}

fn parse_dispatch_authorization(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("verify") => parse_dispatch_authorization_verify(tokens),
        Some(_) => Err(unknown_dispatch("group graph run dispatch authorization")),
        None => Err(with_usage(
            "group graph run dispatch authorization command is required",
        )),
    }
}

fn parse_dispatch_authorization_verify(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = required_id(
        tokens,
        "group graph run dispatch authorization verify",
        "GRAPH_RUN_ID",
    )?;
    let mut authorization_source = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--authorization" if authorization_source.is_none() => {
                authorization_source = Some(next_value(tokens, "--authorization")?);
            }
            "--authorization" => return Err(duplicate("--authorization")),
            _ => {
                return Err(unknown_dispatch(
                    "group graph run dispatch authorization verify",
                ));
            }
        }
    }
    let authorization_source = authorization_source.ok_or_else(|| {
        with_usage("group graph run dispatch authorization verify requires --authorization FILE|-")
    })?;
    Ok(run_command(GroupGraphRunCommand::Dispatch(
        GroupGraphRunDispatchCommand::AuthorizationVerify {
            graph_run_id,
            authorization_source,
        },
    )))
}

fn parse_dispatch_prepare(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, "group graph run dispatch prepare", "GRAPH_RUN_ID")?;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown_dispatch("group graph run dispatch prepare")),
        }
    }
    Ok(run_command(GroupGraphRunCommand::Dispatch(
        GroupGraphRunDispatchCommand::Prepare { graph_run_id },
    )))
}

fn parse_dispatch_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let dispatch_request_id = required_id(
        tokens,
        "group graph run dispatch show",
        "DISPATCH_REQUEST_ID",
    )?;
    let include_request = match tokens.pop_front().as_deref() {
        Some("--include-request") => true,
        Some(_) => return Err(unknown_dispatch("group graph run dispatch show")),
        None => false,
    };
    if !tokens.is_empty() {
        return Err(unknown_dispatch("group graph run dispatch show"));
    }
    Ok(run_command(GroupGraphRunCommand::Dispatch(
        GroupGraphRunDispatchCommand::Show {
            dispatch_request_id,
            include_request,
        },
    )))
}

fn parse_dispatch_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_bounded_limit(tokens, MAX_GROUP_AGENT_NODE_DISPATCH_REQUEST_LIST_LIMIT)?;
    }
    if !tokens.is_empty() {
        return Err(unknown_dispatch("group graph run dispatch list"));
    }
    Ok(run_command(GroupGraphRunCommand::Dispatch(
        GroupGraphRunDispatchCommand::List {
            graph_run_id,
            limit,
        },
    )))
}

fn parse_run_control(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("export") => {
            let graph_run_id =
                required_id(tokens, "group graph run control export", "GRAPH_RUN_ID")?;
            require_empty(tokens)?;
            Ok(run_command(GroupGraphRunCommand::Control(
                GroupGraphRunControlCommand::Export { graph_run_id },
            )))
        }
        Some(value) => Err(unknown("group graph run control", value)),
        None => Err(with_usage("group graph run control command is required")),
    }
}

fn parse_run_contract(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("admit") => parse_contract_admit(tokens, idempotency_key),
        Some("show") => parse_contract_show(tokens),
        Some("list") => parse_contract_list(tokens),
        Some(value) => Err(unknown("group graph run contract", value)),
        None => Err(with_usage("group graph run contract command is required")),
    }
}

fn parse_contract_admit(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, "group graph run contract admit", "GRAPH_RUN_ID")?;
    let mut contract_source = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--contract" if contract_source.is_none() => {
                contract_source = Some(next_value(tokens, "--contract")?);
            }
            "--contract" => return Err(duplicate("--contract")),
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown("group graph run contract admit", &option)),
        }
    }
    let contract_source = contract_source
        .ok_or_else(|| with_usage("group graph run contract admit requires --contract FILE|-"))?;
    Ok(run_command(GroupGraphRunCommand::Contract(
        GroupGraphRunContractCommand::Admit {
            graph_run_id,
            contract_source,
        },
    )))
}

fn parse_contract_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let contract_id = required_id(tokens, "group graph run contract show", "CONTRACT_ID")?;
    let include_contract = match tokens.pop_front().as_deref() {
        Some("--include-contract") => true,
        Some(option) => return Err(unknown("group graph run contract show", option)),
        None => false,
    };
    require_empty(tokens)?;
    Ok(run_command(GroupGraphRunCommand::Contract(
        GroupGraphRunContractCommand::Show {
            contract_id,
            include_contract,
        },
    )))
}

fn parse_contract_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    require_empty(tokens)?;
    Ok(run_command(GroupGraphRunCommand::Contract(
        GroupGraphRunContractCommand::List {
            graph_run_id,
            limit,
        },
    )))
}

pub(crate) fn run_command(command: GroupGraphRunCommand) -> Command {
    Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(command)))
}

fn parse_run_prepare(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let graph_id = required_id(tokens, "group graph run prepare", "GRAPH_ID")?;
    let mut plan_source = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--plan" if plan_source.is_none() => {
                plan_source = Some(next_value(tokens, "--plan")?);
            }
            "--plan" => return Err(duplicate("--plan")),
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown("group graph run prepare", &option)),
        }
    }
    let plan_source =
        plan_source.ok_or_else(|| with_usage("group graph run prepare requires --plan FILE|-"))?;
    Ok(Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
        GroupGraphRunCommand::Prepare {
            graph_id,
            plan_source,
        },
    ))))
}

fn parse_run_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, "group graph run show", "GRAPH_RUN_ID")?;
    let include_plan = match tokens.pop_front().as_deref() {
        Some("--include-plan") => true,
        Some(option) => return Err(unknown("group graph run show", option)),
        None => false,
    };
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
        GroupGraphRunCommand::Show {
            graph_run_id,
            include_plan,
        },
    ))))
}

fn parse_run_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
        GroupGraphRunCommand::List { graph_id, limit },
    ))))
}

fn parse_prepare(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let group_run_id = required_id(tokens, "group graph prepare", "GROUP_RUN_ID")?;
    let mut spec_source = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--spec" if spec_source.is_none() => {
                spec_source = Some(next_value(tokens, "--spec")?);
            }
            "--spec" => return Err(duplicate("--spec")),
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown("group graph prepare", &option)),
        }
    }
    let spec_source =
        spec_source.ok_or_else(|| with_usage("group graph prepare requires --spec FILE|-"))?;
    Ok(Command::Group(GroupCommand::Graph(
        GroupGraphCommand::Prepare {
            group_run_id,
            spec_source,
        },
    )))
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_id = required_id(tokens, "group graph show", "GRAPH_ID")?;
    let include_spec = match tokens.pop_front().as_deref() {
        Some("--include-spec") => true,
        Some(option) => return Err(unknown("group graph show", option)),
        None => false,
    };
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Graph(
        GroupGraphCommand::Show {
            graph_id,
            include_spec,
        },
    )))
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let group_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Graph(
        GroupGraphCommand::List {
            group_run_id,
            limit,
        },
    )))
}

pub(crate) fn parse_limit(tokens: &mut VecDeque<String>) -> Result<usize, String> {
    parse_bounded_limit(tokens, MAX_GROUP_AGENT_GRAPH_LIST_LIMIT)
}

fn parse_bounded_limit(tokens: &mut VecDeque<String>, maximum: usize) -> Result<usize, String> {
    let value = next_value(tokens, "--limit")?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| with_usage(&format!("invalid --limit '{}'", terminal_text(&value))))?;
    if (1..=maximum).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(with_usage(&format!(
            "--limit must be between 1 and {maximum}"
        )))
    }
}

pub(crate) fn required_id(
    tokens: &mut VecDeque<String>,
    operation: &str,
    field: &str,
) -> Result<String, String> {
    match tokens.front() {
        Some(value) if !value.starts_with('-') => next_value(tokens, operation),
        _ => Err(with_usage(&format!("{operation} requires {field}"))),
    }
}

pub(crate) fn duplicate(option: &str) -> String {
    with_usage(&format!("{option} was specified more than once"))
}

pub(crate) fn unknown(operation: &str, option: &str) -> String {
    with_usage(&format!(
        "unknown {operation} option '{}'",
        terminal_text(option)
    ))
}

pub(crate) fn unknown_dispatch(operation: &str) -> String {
    with_usage(&format!("unknown {operation} option"))
}

pub(crate) fn with_usage(message: &str) -> String {
    format!("{message}\n\n{}", usage())
}
