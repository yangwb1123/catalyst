use crate::args::{
    Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand, GroupGraphRunReadyStepOptions,
    parse_tokens, usage,
};

fn parse(tokens: &[&str]) -> Result<crate::args::Args, String> {
    parse_tokens(tokens.iter().map(ToString::to_string))
}

fn required_command_tail() -> Vec<String> {
    vec![
        "--expected-provider-request-id".into(),
        "provider-request-1".into(),
        "--expected-ready-authorization-sha256".into(),
        "a".repeat(64),
        "--pricing".into(),
        "pricing.json".into(),
        "--core-bin".into(),
        "/opt/forge/bin/forge".into(),
        "--core-bin-sha256".into(),
        "b".repeat(64),
    ]
}

fn command(tokens: Vec<String>) -> Result<crate::args::Args, String> {
    let mut all = vec![
        "group".into(),
        "graph".into(),
        "run".into(),
        "step".into(),
        "graph-run-1".into(),
    ];
    all.extend(tokens);
    parse_tokens(all)
}

#[test]
fn parses_every_ready_step_authority_boundary() {
    let mut tail = required_command_tail();
    tail.extend([
        "--confirm-off-machine".into(),
        "--confirm-predecessor-content".into(),
        "--include-result".into(),
    ]);
    let parsed = command(tail).expect("ready step arguments parse");
    let expected = GroupGraphRunReadyStepOptions {
        graph_run_id: "graph-run-1".into(),
        expected_provider_request_id: "provider-request-1".into(),
        expected_ready_authorization_sha256: "a".repeat(64),
        pricing_source: "pricing.json".into(),
        core_bin: "/opt/forge/bin/forge".into(),
        core_bin_sha256: "b".repeat(64),
        confirm_off_machine: true,
        confirm_predecessor_content: true,
        include_result: true,
    };
    assert_eq!(
        parsed.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Step(Box::new(expected))
        )))
    );
}

#[test]
fn defaults_to_no_consent_and_metadata_only() {
    let parsed = command(required_command_tail()).expect("safe defaults parse");
    let Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(GroupGraphRunCommand::Step(
        options,
    )))) = parsed.command
    else {
        panic!("expected ready step command");
    };
    assert!(!options.confirm_off_machine);
    assert!(!options.confirm_predecessor_content);
    assert!(!options.include_result);
}

#[test]
fn requires_every_exact_anchor_and_rejects_duplicates() {
    let required = required_command_tail();
    for index in (0..required.len()).step_by(2) {
        let mut missing = required.clone();
        missing.drain(index..=index + 1);
        assert!(command(missing).is_err());
    }
    let mut duplicate = required;
    duplicate.extend(["--pricing".into(), "other.json".into()]);
    assert!(command(duplicate).is_err());
}

#[test]
fn unknown_options_are_redacted_and_help_names_the_surface() {
    // secret-scan:ignore -- inert rejected-option fixture, never a credential.
    let secret = "private-inline-credential-must-not-echo";
    let error = command(vec![format!("--api-key={secret}")]).expect_err("unknown fails");
    assert!(!error.contains(secret));
    for expected in [
        "group graph run step GRAPH_RUN_ID",
        "--expected-provider-request-id ID",
        "--expected-ready-authorization-sha256 SHA256",
        "[--confirm-predecessor-content] [--include-result]",
    ] {
        assert!(usage().contains(expected));
    }
}

#[test]
fn graph_run_identifier_remains_required() {
    assert!(parse(&["group", "graph", "run", "step"]).is_err());
}

#[test]
fn caller_idempotency_keys_cannot_create_a_second_claim_namespace() {
    let mut tokens = vec!["--idempotency-key".into(), "caller-key".into()];
    tokens.extend([
        "group".into(),
        "graph".into(),
        "run".into(),
        "step".into(),
        "graph-run-1".into(),
    ]);
    tokens.extend(required_command_tail());
    let error = parse_tokens(tokens).expect_err("caller key must fail");
    assert!(error.contains("expected request and authorization anchors"));
}
