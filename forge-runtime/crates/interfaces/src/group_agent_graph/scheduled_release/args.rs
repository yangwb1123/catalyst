use std::collections::VecDeque;

use crate::args::{
    Command, GroupGraphRunCommand, GroupGraphRunScheduledContractCommand,
    GroupGraphRunScheduledContractProviderRequestCommand,
};

use super::{duplicate, next_value, required_id, run_command, unknown, with_usage};

pub(super) fn parse_release_control(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("export") => parse_release_control_export(tokens),
        Some(value) => Err(unknown(
            "group graph run scheduled-contract provider-request release-control",
            value,
        )),
        None => Err(with_usage(
            "group graph run scheduled-contract provider-request release-control command is required",
        )),
    }
}

pub(super) fn parse_authorization(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("verify") => parse_authorization_verify(tokens),
        Some(value) => Err(unknown(
            "group graph run scheduled-contract provider-request authorization",
            value,
        )),
        None => Err(with_usage(
            "group graph run scheduled-contract provider-request authorization command is required",
        )),
    }
}

pub(super) fn parse_readiness(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("verify") => parse_readiness_verify(tokens),
        Some(value) => Err(unknown(
            "group graph run scheduled-contract provider-request readiness",
            value,
        )),
        None => Err(with_usage(
            "group graph run scheduled-contract provider-request readiness command is required",
        )),
    }
}

fn parse_release_control_export(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract provider-request release-control export";
    let provider_request_id = required_id(tokens, operation, "PROVIDER_REQUEST_ID")?;
    super::require_empty(tokens)?;
    Ok(command(
        GroupGraphRunScheduledContractProviderRequestCommand::ReleaseControlExport {
            provider_request_id,
        },
    ))
}

fn parse_authorization_verify(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract provider-request authorization verify";
    let provider_request_id = required_id(tokens, operation, "PROVIDER_REQUEST_ID")?;
    let authorization_source = parse_authorization_source(tokens, operation)?;
    Ok(command(
        GroupGraphRunScheduledContractProviderRequestCommand::AuthorizationVerify {
            provider_request_id,
            authorization_source,
        },
    ))
}

fn parse_authorization_source(
    tokens: &mut VecDeque<String>,
    operation: &str,
) -> Result<String, String> {
    let mut source = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--authorization" if source.is_none() => {
                source = Some(next_value(tokens, "--authorization")?);
            }
            "--authorization" => return Err(duplicate("--authorization")),
            _ => return Err(unknown(operation, &option)),
        }
    }
    source.ok_or_else(|| {
        with_usage(
            "scheduled provider-request authorization verify requires --authorization FILE|-",
        )
    })
}

fn parse_readiness_verify(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run scheduled-contract provider-request readiness verify";
    let provider_request_id = required_id(tokens, operation, "PROVIDER_REQUEST_ID")?;
    let (authorization_source, pricing_source) = parse_readiness_sources(tokens, operation)?;
    if authorization_source == "-" && pricing_source == "-" {
        return Err(with_usage(
            "authorization and pricing cannot both read from stdin",
        ));
    }
    Ok(command(
        GroupGraphRunScheduledContractProviderRequestCommand::ReadinessVerify {
            provider_request_id,
            authorization_source,
            pricing_source,
        },
    ))
}

fn parse_readiness_sources(
    tokens: &mut VecDeque<String>,
    operation: &str,
) -> Result<(String, String), String> {
    let mut authorization = None;
    let mut pricing = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--authorization" if authorization.is_none() => {
                authorization = Some(next_value(tokens, "--authorization")?);
            }
            "--pricing" if pricing.is_none() => {
                pricing = Some(next_value(tokens, "--pricing")?);
            }
            "--authorization" => return Err(duplicate("--authorization")),
            "--pricing" => return Err(duplicate("--pricing")),
            _ => return Err(unknown(operation, &option)),
        }
    }
    let authorization = authorization.ok_or_else(|| {
        with_usage("scheduled provider-request readiness verify requires --authorization FILE|-")
    })?;
    let pricing = pricing.ok_or_else(|| {
        with_usage("scheduled provider-request readiness verify requires --pricing FILE|-")
    })?;
    Ok((authorization, pricing))
}

fn command(value: GroupGraphRunScheduledContractProviderRequestCommand) -> Command {
    run_command(GroupGraphRunCommand::ScheduledContract(
        GroupGraphRunScheduledContractCommand::ProviderRequest(value),
    ))
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
    fn parses_release_control_export() {
        let parsed = parse(&[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "release-control",
            "export",
            "request-1",
        ])
        .expect("release-control export parses");
        assert!(matches!(
            parsed.command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::ProviderRequest(
                        GroupGraphRunScheduledContractProviderRequestCommand::ReleaseControlExport {
                            provider_request_id,
                        }
                    )
                )
            ))) if provider_request_id == "request-1"
        ));
    }

    #[test]
    fn parses_authorization_verify() {
        let parsed = parse(&[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "authorization",
            "verify",
            "request-1",
            "--authorization",
            "-",
        ])
        .expect("authorization verify parses");
        assert!(matches!(
            parsed.command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::ProviderRequest(
                        GroupGraphRunScheduledContractProviderRequestCommand::AuthorizationVerify {
                            provider_request_id,
                            authorization_source,
                        }
                    )
                )
            ))) if provider_request_id == "request-1" && authorization_source == "-"
        ));
    }

    #[test]
    fn parses_readiness_verify() {
        let parsed = parse(&[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "readiness",
            "verify",
            "request-1",
            "--pricing",
            "pricing.json",
            "--authorization",
            "-",
        ])
        .expect("readiness verify parses");
        assert!(matches!(
            parsed.command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::ScheduledContract(
                    GroupGraphRunScheduledContractCommand::ProviderRequest(
                        GroupGraphRunScheduledContractProviderRequestCommand::ReadinessVerify {
                            provider_request_id,
                            authorization_source,
                            pricing_source,
                        }
                    )
                )
            ))) if provider_request_id == "request-1"
                && authorization_source == "-"
                && pricing_source == "pricing.json"
        ));
    }

    #[test]
    fn readiness_rejects_missing_duplicate_and_two_stdin_sources() {
        let prefix = [
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "readiness",
            "verify",
            "request-1",
        ];
        let mut missing = prefix.to_vec();
        missing.extend(["--authorization", "-"]);
        assert!(parse(&missing).unwrap_err().contains("requires --pricing"));
        let mut duplicate = prefix.to_vec();
        duplicate.extend([
            "--authorization",
            "auth.json",
            "--pricing",
            "-",
            "--pricing",
            "again.json",
        ]);
        assert!(parse(&duplicate).unwrap_err().contains("more than once"));
        let mut two_stdin = prefix.to_vec();
        two_stdin.extend(["--authorization", "-", "--pricing", "-"]);
        assert!(
            parse(&two_stdin)
                .unwrap_err()
                .contains("cannot both read from stdin")
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_read_only_key() {
        let prefix = [
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
        ];
        let mut missing = prefix.to_vec();
        missing.extend(["authorization", "verify", "request-1"]);
        assert!(
            parse(&missing)
                .unwrap_err()
                .contains("requires --authorization")
        );

        let mut duplicate = prefix.to_vec();
        duplicate.extend([
            "authorization",
            "verify",
            "request-1",
            "--authorization",
            "-",
            "--authorization",
            "file.json",
        ]);
        assert!(parse(&duplicate).unwrap_err().contains("more than once"));

        let mut keyed = vec!["--idempotency-key", "wrong"];
        keyed.extend(prefix);
        keyed.extend(["release-control", "export", "request-1"]);
        assert!(
            parse(&keyed)
                .unwrap_err()
                .contains("only valid for mutating commands")
        );
    }
}
