use crate::args::{
    Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand,
    GroupGraphRunScheduledContractCommand, GroupGraphRunScheduledContractProviderRequestCommand,
    parse_tokens,
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
            GroupGraphRunCommand::ScheduledContract(GroupGraphRunScheduledContractCommand::Show {
                include_contract: true,
                ..
            })
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
            GroupGraphRunCommand::ScheduledContract(GroupGraphRunScheduledContractCommand::List {
                limit: 7,
                ..
            })
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
fn parses_provider_request_dispatch_execute() {
    let parsed = parse(&[
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "provider-request",
        "dispatch",
        "execute",
        "scheduled-request-1",
        "--authorization",
        "auth.json",
        "--pricing",
        "pricing.json",
        "--core-bin",
        "/opt/forge-core",
        "--core-bin-sha256",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--confirm-off-machine",
        "--include-result",
    ])
    .expect("scheduled dispatch execute parses");
    assert!(matches!(
        parsed.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::ScheduledContract(
                GroupGraphRunScheduledContractCommand::ProviderRequest(
                    GroupGraphRunScheduledContractProviderRequestCommand::Execute {
                        provider_request_id,
                        confirm_off_machine: true,
                        include_result: true,
                        ..
                    }
                )
            )
        ))) if provider_request_id == "scheduled-request-1"
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
}

#[test]
fn rejects_key_on_read_only_and_dual_stdin_dispatch() {
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
    assert!(
        parse(&[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "dispatch",
            "execute",
            "request-1",
            "--authorization",
            "-",
            "--pricing",
            "-",
            "--core-bin",
            "/opt/forge-core",
            "--core-bin-sha256",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ])
        .expect_err("two stdin sources reject")
        .contains("cannot both read from stdin")
    );
}
