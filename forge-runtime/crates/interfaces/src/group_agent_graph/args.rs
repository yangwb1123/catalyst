//! Argument parsing for the explicit-ID Group Agent Graph management surface.

use std::collections::VecDeque;

use crate::{
    group_context_output::terminal_text, runtime_domain::MAX_GROUP_AGENT_GRAPH_LIST_LIMIT,
};

use super::{
    Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand, GroupGraphRunContractCommand,
    GroupGraphRunControlCommand, next_value, require_empty, usage,
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
        Some("show") => parse_run_show(tokens),
        Some("list") => parse_run_list(tokens),
        Some(value) => Err(with_usage(&format!(
            "unknown group graph run command '{}'",
            terminal_text(value)
        ))),
        None => Err(with_usage("group graph run command is required")),
    }
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

fn run_command(command: GroupGraphRunCommand) -> Command {
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

fn parse_limit(tokens: &mut VecDeque<String>) -> Result<usize, String> {
    let value = next_value(tokens, "--limit")?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| with_usage(&format!("invalid --limit '{}'", terminal_text(&value))))?;
    if (1..=MAX_GROUP_AGENT_GRAPH_LIST_LIMIT).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(with_usage(&format!(
            "--limit must be between 1 and {MAX_GROUP_AGENT_GRAPH_LIST_LIMIT}"
        )))
    }
}

fn required_id(
    tokens: &mut VecDeque<String>,
    operation: &str,
    field: &str,
) -> Result<String, String> {
    match tokens.front() {
        Some(value) if !value.starts_with('-') => next_value(tokens, operation),
        _ => Err(with_usage(&format!("{operation} requires {field}"))),
    }
}

fn duplicate(option: &str) -> String {
    with_usage(&format!("{option} was specified more than once"))
}

fn unknown(operation: &str, option: &str) -> String {
    with_usage(&format!(
        "unknown {operation} option '{}'",
        terminal_text(option)
    ))
}

fn with_usage(message: &str) -> String {
    format!("{message}\n\n{}", usage())
}
