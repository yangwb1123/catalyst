use std::collections::VecDeque;

use crate::args::{
    GroupCommand, GroupGraphCommand, GroupGraphRunScheduledContractCommand,
    GroupGraphRunScheduledContractProviderRequestCommand,
    GroupGraphRunScheduledContractSuccessorCommand,
};

use super::{
    Command, GroupGraphRunCommand, duplicate, next_value, parse_limit, required_id, run_command,
    unknown, with_usage,
};

pub(super) fn parse(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("admit") => parse_admit(tokens, idempotency_key),
        Some("show") => parse_show(tokens),
        Some("list") => parse_list(tokens),
        Some("provider-request") => parse_provider_request(tokens, idempotency_key),
        Some("predecessor-receipt") => parse_predecessor_receipt(tokens),
        Some("successor") => parse_successor(tokens, idempotency_key),
        Some("wave-admit") => parse_wave_admit(tokens, idempotency_key),
        Some(value) => Err(unknown("group graph run scheduled-contract", value)),
        None => Err(with_usage(
            "group graph run scheduled-contract command is required",
        )),
    }
}

fn parse_provider_request(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("prepare") => parse_provider_request_prepare(tokens, idempotency_key),
        Some("show") => parse_provider_request_show(tokens),
        Some("list") => parse_provider_request_list(tokens),
        Some("release-control") => super::scheduled_release_args::parse_release_control(tokens),
        Some("authorization") => super::scheduled_release_args::parse_authorization(tokens),
        Some("readiness") => super::scheduled_release_args::parse_readiness(tokens),
        Some("dispatch") => parse_dispatch(tokens),
        Some(value) => Err(unknown(
            "group graph run scheduled-contract provider-request",
            value,
        )),
        None => Err(with_usage(
            "group graph run scheduled-contract provider-request command is required",
        )),
    }
}

fn parse_dispatch(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("execute") => parse_dispatch_execute(tokens),
        Some("adjudicate") => parse_dispatch_adjudicate(tokens),
        Some(value) => Err(unknown(
            "group graph run scheduled-contract provider-request dispatch",
            value,
        )),
        None => Err(with_usage(
            "scheduled provider-request dispatch command is required",
        )),
    }
}

fn parse_dispatch_adjudicate(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract provider-request dispatch adjudicate";
    let provider_request_id = required_id(tokens, operation, "PROVIDER_REQUEST_ID")?;
    super::require_empty(tokens)?;
    Ok(provider_request_command(
        GroupGraphRunScheduledContractProviderRequestCommand::Adjudicate {
            provider_request_id,
        },
    ))
}

fn parse_dispatch_execute(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract provider-request dispatch execute";
    let provider_request_id = required_id(tokens, operation, "PROVIDER_REQUEST_ID")?;
    let mut options = ExecuteOptions::default();
    parse_execute_options(tokens, operation, &mut options)?;
    let authorization_source = options
        .authorization_source
        .ok_or_else(|| with_usage("scheduled dispatch execute requires --authorization FILE|-"))?;
    let pricing_source = options
        .pricing_source
        .ok_or_else(|| with_usage("scheduled dispatch execute requires --pricing FILE|-"))?;
    let core_bin = options.core_bin.ok_or_else(|| {
        with_usage("scheduled dispatch execute requires --core-bin ABSOLUTE_FILE")
    })?;
    let core_bin_sha256 = options.core_bin_sha256.ok_or_else(|| {
        with_usage("scheduled dispatch execute requires --core-bin-sha256 SHA256")
    })?;
    if authorization_source == "-" && pricing_source == "-" {
        return Err(with_usage(
            "authorization and pricing cannot both read from stdin",
        ));
    }
    Ok(provider_request_command(
        GroupGraphRunScheduledContractProviderRequestCommand::Execute {
            provider_request_id,
            authorization_source,
            pricing_source,
            core_bin,
            core_bin_sha256,
            confirm_off_machine: options.confirm_off_machine,
            confirm_predecessor_content: options.confirm_predecessor_content,
            include_result: options.include_result,
        },
    ))
}

#[derive(Default)]
struct ExecuteOptions {
    authorization_source: Option<String>,
    pricing_source: Option<String>,
    core_bin: Option<String>,
    core_bin_sha256: Option<String>,
    confirm_off_machine: bool,
    confirm_predecessor_content: bool,
    include_result: bool,
}

/// Consumes the option tokens for `dispatch execute`, rejecting duplicates and
/// unknown options.
fn parse_execute_options(
    tokens: &mut VecDeque<String>,
    operation: &str,
    options: &mut ExecuteOptions,
) -> Result<(), String> {
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--authorization" if options.authorization_source.is_none() => {
                options.authorization_source = Some(next_value(tokens, "--authorization")?);
            }
            "--authorization" => return Err(duplicate("--authorization")),
            "--pricing" if options.pricing_source.is_none() => {
                options.pricing_source = Some(next_value(tokens, "--pricing")?);
            }
            "--pricing" => return Err(duplicate("--pricing")),
            "--core-bin" if options.core_bin.is_none() => {
                options.core_bin = Some(next_value(tokens, "--core-bin")?);
            }
            "--core-bin" => return Err(duplicate("--core-bin")),
            "--core-bin-sha256" if options.core_bin_sha256.is_none() => {
                options.core_bin_sha256 = Some(next_value(tokens, "--core-bin-sha256")?);
            }
            "--core-bin-sha256" => return Err(duplicate("--core-bin-sha256")),
            "--confirm-off-machine" => {
                if options.confirm_off_machine {
                    return Err(duplicate("--confirm-off-machine"));
                }
                options.confirm_off_machine = true;
            }
            "--confirm-predecessor-content" => {
                if options.confirm_predecessor_content {
                    return Err(duplicate("--confirm-predecessor-content"));
                }
                options.confirm_predecessor_content = true;
            }
            "--include-result" => {
                if options.include_result {
                    return Err(duplicate("--include-result"));
                }
                options.include_result = true;
            }
            _ => return Err(unknown(operation, &option)),
        }
    }
    Ok(())
}

fn parse_provider_request_prepare(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract provider-request prepare";
    let scheduled_contract_id = required_id(tokens, operation, "CONTRACT_ID")?;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown(operation, &option)),
        }
    }
    Ok(provider_request_command(
        GroupGraphRunScheduledContractProviderRequestCommand::Prepare {
            scheduled_contract_id,
        },
    ))
}

fn parse_provider_request_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract provider-request show";
    let provider_request_id = required_id(tokens, operation, "PROVIDER_REQUEST_ID")?;
    let include_request = match tokens.pop_front().as_deref() {
        Some("--include-request") => true,
        Some(value) => return Err(unknown(operation, value)),
        None => false,
    };
    super::require_empty(tokens)?;
    Ok(provider_request_command(
        GroupGraphRunScheduledContractProviderRequestCommand::Show {
            provider_request_id,
            include_request,
        },
    ))
}

fn parse_provider_request_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    super::require_empty(tokens)?;
    Ok(provider_request_command(
        GroupGraphRunScheduledContractProviderRequestCommand::List {
            graph_run_id,
            limit,
        },
    ))
}

fn provider_request_command(
    value: GroupGraphRunScheduledContractProviderRequestCommand,
) -> Command {
    command(GroupGraphRunScheduledContractCommand::ProviderRequest(
        value,
    ))
}

fn parse_predecessor_receipt(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract predecessor-receipt";
    match tokens.pop_front().as_deref() {
        Some("export") => {
            let provider_request_id = required_id(
                tokens,
                "group graph run scheduled-contract predecessor-receipt export",
                "PROVIDER_REQUEST_ID",
            )?;
            super::require_empty(tokens)?;
            Ok(command(
                GroupGraphRunScheduledContractCommand::PredecessorReceiptExport {
                    provider_request_id,
                },
            ))
        }
        Some(value) => Err(unknown(operation, value)),
        None => Err(with_usage(
            "group graph run scheduled-contract predecessor-receipt command is required",
        )),
    }
}

fn parse_successor(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract successor";
    match tokens.pop_front().as_deref() {
        Some("admit") => parse_successor_admit(tokens, idempotency_key),
        Some("show") => parse_successor_show(tokens),
        Some("list") => parse_successor_list(tokens),
        Some(value) => Err(unknown(operation, value)),
        None => Err(with_usage(
            "group graph run scheduled-contract successor command is required",
        )),
    }
}

fn parse_successor_admit(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract successor admit";
    let graph_run_id = required_id(tokens, operation, "GRAPH_RUN_ID")?;
    let mut contract_source = None;
    let mut predecessor_receipt_sources = Vec::new();
    let mut predecessor_content_source = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--contract" if contract_source.is_none() => {
                contract_source = Some(next_value(tokens, "--contract")?);
            }
            "--contract" => return Err(duplicate("--contract")),
            "--predecessor-receipt" => {
                predecessor_receipt_sources.push(next_value(tokens, "--predecessor-receipt")?);
            }
            "--predecessor-content" if predecessor_content_source.is_none() => {
                predecessor_content_source = Some(next_value(tokens, "--predecessor-content")?);
            }
            "--predecessor-content" => return Err(duplicate("--predecessor-content")),
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown(operation, &option)),
        }
    }
    let contract_source =
        contract_source.ok_or_else(|| with_usage("successor admit requires --contract FILE|-"))?;
    if predecessor_receipt_sources.is_empty() {
        return Err(with_usage(
            "successor admit requires at least one --predecessor-receipt FILE|-",
        ));
    }
    Ok(command(GroupGraphRunScheduledContractCommand::Successor(
        GroupGraphRunScheduledContractSuccessorCommand::Admit {
            graph_run_id,
            contract_source,
            predecessor_receipt_sources,
            predecessor_content_source,
        },
    )))
}

fn parse_successor_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract successor show";
    let contract_id = required_id(tokens, operation, "SUCCESSOR_ID")?;
    let include_contract = match tokens.pop_front().as_deref() {
        Some("--include-contract") => true,
        Some(value) => return Err(unknown(operation, value)),
        None => false,
    };
    super::require_empty(tokens)?;
    Ok(command(GroupGraphRunScheduledContractCommand::Successor(
        GroupGraphRunScheduledContractSuccessorCommand::Show {
            contract_id,
            include_contract,
        },
    )))
}

fn parse_successor_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    super::require_empty(tokens)?;
    Ok(command(GroupGraphRunScheduledContractCommand::Successor(
        GroupGraphRunScheduledContractSuccessorCommand::List {
            graph_run_id,
            limit,
        },
    )))
}

fn parse_admit(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract admit";
    let graph_run_id = required_id(tokens, operation, "GRAPH_RUN_ID")?;
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
            _ => return Err(unknown(operation, &option)),
        }
    }
    let contract_source = contract_source
        .ok_or_else(|| with_usage("scheduled-contract admit requires --contract FILE|-"))?;
    Ok(command(GroupGraphRunScheduledContractCommand::Admit {
        graph_run_id,
        contract_source,
    }))
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract show";
    let contract_id = required_id(tokens, operation, "CONTRACT_ID")?;
    let include_contract = match tokens.pop_front().as_deref() {
        Some("--include-contract") => true,
        Some(value) => return Err(unknown(operation, value)),
        None => false,
    };
    super::require_empty(tokens)?;
    Ok(command(GroupGraphRunScheduledContractCommand::Show {
        contract_id,
        include_contract,
    }))
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    super::require_empty(tokens)?;
    Ok(command(GroupGraphRunScheduledContractCommand::List {
        graph_run_id,
        limit,
    }))
}

fn command(value: GroupGraphRunScheduledContractCommand) -> Command {
    run_command(GroupGraphRunCommand::ScheduledContract(value))
}

#[cfg(test)]
#[path = "scheduled_contract_args_tests.rs"]
mod tests;


fn parse_wave_admit(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, "wave-admit", "GRAPH_RUN_ID")?;
    let mut receipts: Vec<String> = Vec::new();
    let mut schedule_sha256: Option<String> = None;
    let mut go_core: Option<String> = None;
    while let Some(flag) = tokens.pop_front() {
        match flag.as_str() {
            "--predecessor-receipt" => {
                let value = next_value(tokens, "--predecessor-receipt")?;
                receipts.push(value);
            }
            "--schedule-sha256" => {
                schedule_sha256 = Some(next_value(tokens, "--schedule-sha256")?);
            }
            "--go-core" => {
                go_core = Some(next_value(tokens, "--go-core")?);
            }
            "--idempotency-key" => {
                let value = next_value(tokens, "--idempotency-key")?;
                *idempotency_key = Some(value);
            }
            other => return Err(unknown("group graph run scheduled-contract wave-admit", other)),
        }
    }
    let schedule_sha256 = schedule_sha256.ok_or_else(|| {
        with_usage("group graph run scheduled-contract wave-admit requires --schedule-sha256")
    })?;
    Ok(Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
        GroupGraphRunCommand::ScheduledContract(
            GroupGraphRunScheduledContractCommand::WaveAdmit {
                graph_run_id,
                predecessor_receipt_sources: receipts,
                schedule_sha256,
                go_core,
            },
        ),
    ))))
}