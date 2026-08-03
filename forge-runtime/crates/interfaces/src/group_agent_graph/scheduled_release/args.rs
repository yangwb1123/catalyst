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
