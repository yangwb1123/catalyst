use std::collections::VecDeque;

use crate::args::{
    GroupGraphRunScheduledContractCommand, GroupGraphRunScheduledContractProviderRequestCommand,
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
        Some(value) => Err(unknown(
            "group graph run scheduled-contract provider-request",
            value,
        )),
        None => Err(with_usage(
            "group graph run scheduled-contract provider-request command is required",
        )),
    }
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
mod tests {
    use crate::args::{
        Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand,
        GroupGraphRunScheduledContractCommand,
        GroupGraphRunScheduledContractProviderRequestCommand, parse_tokens,
    };

    fn parse(args: &[&str]) -> Result<crate::args::Args, String> {
        parse_tokens(args.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn parses_admit() {
        let admitted = parse(&[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "admit",
            "run-1",
            "--contract",
            "-",
            "--idempotency-key",
            "candidate-key",
        ])
        .expect("admission parses");
        assert_eq!(admitted.idempotency_key.as_deref(), Some("candidate-key"));
        assert!(matches!(
            admitted.command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::Admit {
                        graph_run_id,
                        contract_source,
                    }
                )
            ))) if graph_run_id == "run-1" && contract_source == "-"
        ));
    }

    #[test]
    fn parses_show() {
        assert!(matches!(
            parse(&[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "show",
                "candidate-1",
                "--include-contract",
            ])
            .expect("show parses")
            .command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::Show {
                        include_contract: true,
                        ..
                    }
                )
            )))
        ));
    }

    #[test]
    fn parses_list() {
        assert!(matches!(
            parse(&[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "list",
                "run-1",
                "--limit",
                "7",
            ])
            .expect("list parses")
            .command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::List { limit: 7, .. }
                )
            )))
        ));
    }

    #[test]
    fn rejects_missing_duplicate_and_read_only_keys() {
        assert!(
            parse(&[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "admit",
                "run-1",
            ])
            .expect_err("missing contract rejects")
            .contains("requires --contract")
        );
        assert!(
            parse(&[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "show",
                "candidate-1",
                "--include-contract",
                "--include-contract",
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "--idempotency-key",
                "wrong",
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "show",
                "candidate-1",
            ])
            .expect_err("show rejects key")
            .contains("only valid for mutating commands")
        );
    }

    #[test]
    fn parses_provider_request_prepare() {
        let prepared = parse(&[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "prepare",
            "scheduled-contract-1",
            "--idempotency-key",
            "request-key",
        ])
        .expect("provider request preparation parses");
        assert_eq!(prepared.idempotency_key.as_deref(), Some("request-key"));
        assert!(matches!(
            prepared.command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::ProviderRequest(
                        GroupGraphRunScheduledContractProviderRequestCommand::Prepare {
                            scheduled_contract_id,
                        }
                    )
                )
            ))) if scheduled_contract_id == "scheduled-contract-1"
        ));
    }

    #[test]
    fn parses_provider_request_show() {
        assert!(matches!(
            parse(&[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "provider-request",
                "show",
                "scheduled-request-1",
                "--include-request",
            ])
            .expect("provider request show parses")
            .command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::ProviderRequest(
                        GroupGraphRunScheduledContractProviderRequestCommand::Show {
                            include_request: true,
                            ..
                        }
                    )
                )
            )))
        ));
    }

    #[test]
    fn parses_provider_request_list() {
        assert!(matches!(
            parse(&[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "provider-request",
                "list",
                "run-1",
                "--limit",
                "7",
            ])
            .expect("provider request list parses")
            .command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::ProviderRequest(
                        GroupGraphRunScheduledContractProviderRequestCommand::List { limit: 7, .. }
                    )
                )
            )))
        ));
    }

    #[test]
    fn rejects_invalid_provider_request_arguments() {
        assert!(
            parse(&[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "provider-request",
                "prepare",
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "provider-request",
                "show",
                "request-1",
                "--include-request",
                "--include-request",
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "--idempotency-key",
                "wrong",
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "provider-request",
                "list",
            ])
            .expect_err("read-only request list rejects key")
            .contains("only valid for mutating commands")
        );
    }
}
