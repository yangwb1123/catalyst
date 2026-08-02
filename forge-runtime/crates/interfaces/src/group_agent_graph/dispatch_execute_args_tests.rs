use super::{
    Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand, GroupGraphRunDispatchCommand,
    parse_tokens, usage,
};

fn parse(tokens: &[&str]) -> crate::args::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

fn parse_error(tokens: &[&str]) -> String {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect_err("arguments must fail")
}

#[test]
fn dispatch_execute_binds_every_explicit_authority_boundary() {
    let parsed = parse(&[
        "group",
        "graph",
        "run",
        "dispatch",
        "execute",
        "graph-run-1",
        "--authorization",
        "authorization.json",
        "--pricing",
        "-",
        "--core-bin",
        "/opt/forge/bin/forge",
        "--core-bin-sha256",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--confirm-off-machine",
        "--include-result",
    ]);
    assert_eq!(
        parsed.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Dispatch(GroupGraphRunDispatchCommand::Execute {
                graph_run_id: "graph-run-1".into(),
                authorization_source: "authorization.json".into(),
                pricing_source: "-".into(),
                core_bin: "/opt/forge/bin/forge".into(),
                core_bin_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
                confirm_off_machine: true,
                include_result: true,
            })
        )))
    );
}

#[test]
fn dispatch_execute_requires_explicit_artifacts_and_core_pin() {
    let base = ["group", "graph", "run", "dispatch", "execute", "run-1"];
    for suffix in [
        vec![],
        vec!["--authorization", "a.json"],
        vec!["--authorization", "a.json", "--pricing", "p.json"],
        vec![
            "--authorization",
            "-",
            "--pricing",
            "-",
            "--core-bin",
            "/forge",
            "--core-bin-sha256",
            "aa",
        ],
    ] {
        let mut tokens = base.to_vec();
        tokens.extend(suffix);
        assert!(parse_error(&tokens).contains("usage:"));
    }
}

#[test]
fn dispatch_execute_defaults_to_no_consent_and_metadata_only_output() {
    let parsed = parse(&[
        "group",
        "graph",
        "run",
        "dispatch",
        "execute",
        "graph-run-1",
        "--authorization",
        "authorization.json",
        "--pricing",
        "pricing.json",
        "--core-bin",
        "/opt/forge/bin/forge",
        "--core-bin-sha256",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ]);
    assert_eq!(
        parsed.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Dispatch(GroupGraphRunDispatchCommand::Execute {
                graph_run_id: "graph-run-1".into(),
                authorization_source: "authorization.json".into(),
                pricing_source: "pricing.json".into(),
                core_bin: "/opt/forge/bin/forge".into(),
                core_bin_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
                confirm_off_machine: false,
                include_result: false,
            })
        )))
    );
}

#[test]
fn dispatch_execute_rejects_duplicate_or_unowned_authority() {
    let base = [
        "group",
        "graph",
        "run",
        "dispatch",
        "execute",
        "run-1",
        "--authorization",
        "authorization.json",
        "--pricing",
        "pricing.json",
        "--core-bin",
        "/forge",
        "--core-bin-sha256",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ];
    for suffix in [
        ["--include-result", "--include-result"].as_slice(),
        ["--confirm-off-machine", "--confirm-off-machine"].as_slice(),
        ["--authorization", "other.json"].as_slice(),
    ] {
        let mut tokens = base.to_vec();
        tokens.extend_from_slice(suffix);
        assert!(parse_error(&tokens).contains("usage:"));
    }

    let credential_fixture = "private-inline-credential-must-not-echo";
    let option = format!("--api-key={credential_fixture}");
    let mut tokens = base.to_vec();
    tokens.push(&option);
    let error = parse_error(&tokens);
    assert!(error.contains("usage:"));
    assert!(!error.contains(credential_fixture));
}

#[test]
fn dispatch_execute_help_names_every_required_boundary_and_no_retry_surface() {
    let help = usage();
    assert!(help.contains("group graph run dispatch execute GRAPH_RUN_ID"));
    for required in [
        "--authorization FILE|- --pricing FILE|-",
        "--core-bin ABSOLUTE_FILE --core-bin-sha256 SHA256",
        "--confirm-off-machine [--include-result]",
    ] {
        assert!(help.contains(required));
    }
    assert!(help.contains("Result text is\n  hidden unless --include-result is explicit"));
    assert!(!help.contains("group graph run dispatch retry"));
    assert!(!help.contains("group graph run dispatch resume"));
}
