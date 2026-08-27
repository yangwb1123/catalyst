use crate::args::{
    Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand, GroupGraphRunControllerCommand,
    GroupGraphRunControllerStartOptions, GroupGraphRunControllerStepOptions, parse_tokens, usage,
};

fn controller(tokens: Vec<String>) -> Result<crate::args::Args, String> {
    let mut command = vec![
        "group".into(),
        "graph".into(),
        "run".into(),
        "controller".into(),
    ];
    command.extend(tokens);
    parse_tokens(command)
}

fn start(tokens: Vec<String>) -> Result<crate::args::Args, String> {
    let mut command = vec!["start".into(), "graph-run-1".into()];
    command.extend(tokens);
    controller(command)
}

fn step(tokens: Vec<String>) -> Result<crate::args::Args, String> {
    let mut command = vec!["step".into(), "graph-run-1".into()];
    command.extend(tokens);
    controller(command)
}

fn start_tail() -> Vec<String> {
    pairs(&[
        ("--expected-schedule-sha256", &"a".repeat(64)),
        ("--core-bin", "/opt/forge/bin/forge"),
        ("--core-bin-sha256", &"b".repeat(64)),
        ("--endpoint", "https://api.openai.com/v1/responses"),
        ("--model", "gpt-test"),
        ("--max-output-tokens", "17"),
        ("--max-model-output-bytes", "4097"),
        ("--max-model-events", "23"),
        ("--timeout-ms", "1234"),
        ("--max-cost-usd-micros", "5678"),
        ("--pricing-snapshot-sha256", &"c".repeat(64)),
        ("--max-result-bytes", "8192"),
        ("--max-effectful-steps", "2"),
        ("--max-total-cost-usd-micros", "11356"),
    ])
}

fn step_tail() -> Vec<String> {
    pairs(&[
        ("--expected-awaiting-event-sha256", &"7".repeat(64)),
        ("--expected-provider-request-id", "provider-request-exact"),
        ("--expected-authorization-sha256", &"d".repeat(64)),
        ("--pricing", "pricing.json"),
        ("--core-bin", "/opt/forge/bin/forge"),
        ("--core-bin-sha256", &"e".repeat(64)),
    ])
}

fn pairs(values: &[(&str, &str)]) -> Vec<String> {
    values
        .iter()
        .flat_map(|(option, value)| [(*option).to_owned(), (*value).to_owned()])
        .collect()
}

fn replace(tail: &mut [String], option: &str, value: &str) {
    let index = tail
        .iter()
        .position(|candidate| candidate == option)
        .expect("fixture option");
    tail[index + 1] = value.into();
}

#[test]
fn start_parses_exact_profile_and_durable_budgets() {
    let parsed = start(start_tail()).expect("controller start parses");
    let expected = GroupGraphRunControllerStartOptions {
        graph_run_id: "graph-run-1".into(),
        expected_schedule_sha256: "a".repeat(64),
        core_bin: "/opt/forge/bin/forge".into(),
        core_bin_sha256: "b".repeat(64),
        endpoint: "https://api.openai.com/v1/responses".into(),
        model: "gpt-test".into(),
        max_output_tokens: 17,
        max_model_output_bytes: 4097,
        max_model_events: 23,
        timeout_ms: 1234,
        max_cost_usd_micros: 5678,
        pricing_snapshot_sha256: "c".repeat(64),
        max_result_bytes: 8192,
        max_effectful_steps: 2,
        max_total_cost_usd_micros: 11356,
    };
    assert_eq!(
        parsed.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Controller(GroupGraphRunControllerCommand::Start(Box::new(
                expected,
            )))
        )))
    );
}

#[test]
fn start_requires_every_profile_anchor_and_unsigned_budget() {
    let required = start_tail();
    for index in (0..required.len()).step_by(2) {
        let mut missing = required.clone();
        let option = missing[index].clone();
        missing.drain(index..=index + 1);
        let error = start(missing).expect_err("missing start option fails");
        assert!(error.contains(&option), "{option}: {error}");
    }
    for option in [
        "--max-output-tokens",
        "--max-cost-usd-micros",
        "--max-effectful-steps",
        "--max-total-cost-usd-micros",
    ] {
        let mut invalid = required.clone();
        replace(&mut invalid, option, "not-a-number");
        assert!(
            start(invalid)
                .expect_err("invalid number fails")
                .contains(option)
        );
    }
}

#[test]
fn show_and_advance_have_minimal_exact_surfaces() {
    let shown = controller(vec!["show".into(), "graph-run-show".into()]).unwrap();
    assert_eq!(
        shown.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Controller(GroupGraphRunControllerCommand::Show {
                graph_run_id: "graph-run-show".into(),
            })
        )))
    );
    let advanced = controller(pairs(&[
        ("advance", "graph-run-advance"),
        ("--core-bin", "/opt/forge/bin/forge"),
        ("--core-bin-sha256", &"f".repeat(64)),
    ]))
    .expect("controller advance parses");
    assert_advance(advanced.command);
}

fn assert_advance(command: Command) {
    let Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
        GroupGraphRunCommand::Controller(GroupGraphRunControllerCommand::Advance {
            graph_run_id,
            core_bin,
            core_bin_sha256,
        }),
    ))) = command
    else {
        panic!("expected controller advance");
    };
    assert_eq!(graph_run_id, "graph-run-advance");
    assert_eq!(core_bin, "/opt/forge/bin/forge");
    assert_eq!(core_bin_sha256, "f".repeat(64));
}

#[test]
fn show_and_advance_reject_missing_or_extra_authority() {
    assert!(controller(vec!["show".into()]).is_err());
    assert!(controller(vec!["show".into(), "run".into(), "--include-result".into()]).is_err());
    assert!(controller(vec!["advance".into(), "run".into()]).is_err());
    assert!(
        controller(pairs(&[
            ("advance", "run"),
            ("--core-bin", "/opt/forge/bin/forge"),
        ]))
        .is_err()
    );
}

#[test]
fn step_preserves_exact_consent_anchors_and_defaults_to_no_consent() {
    let parsed = step(step_tail()).expect("controller step parses");
    let Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
        GroupGraphRunCommand::Controller(GroupGraphRunControllerCommand::Step(options)),
    ))) = parsed.command
    else {
        panic!("expected controller step");
    };
    assert_eq!(
        *options,
        GroupGraphRunControllerStepOptions {
            graph_run_id: "graph-run-1".into(),
            expected_awaiting_event_sha256: "7".repeat(64),
            expected_provider_request_id: "provider-request-exact".into(),
            expected_authorization_sha256: "d".repeat(64),
            pricing_source: "pricing.json".into(),
            core_bin: "/opt/forge/bin/forge".into(),
            core_bin_sha256: "e".repeat(64),
            confirm_off_machine: false,
            confirm_predecessor_content: false,
            include_result: false,
        }
    );
}

#[test]
fn step_requires_every_anchor_and_accepts_each_explicit_consent_once() {
    let required = step_tail();
    for index in (0..required.len()).step_by(2) {
        let mut missing = required.clone();
        missing.drain(index..=index + 1);
        assert!(step(missing).is_err());
    }
    let mut consented = required.clone();
    consented.extend([
        "--confirm-off-machine".into(),
        "--confirm-predecessor-content".into(),
        "--include-result".into(),
    ]);
    let parsed = step(consented).expect("explicit consent flags parse");
    assert_step_flags(parsed.command);

    let mut duplicate = required;
    duplicate.extend([
        "--confirm-off-machine".into(),
        "--confirm-off-machine".into(),
    ]);
    assert!(step(duplicate).is_err());
}

fn assert_step_flags(command: Command) {
    let Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
        GroupGraphRunCommand::Controller(GroupGraphRunControllerCommand::Step(options)),
    ))) = command
    else {
        panic!("expected controller step");
    };
    assert!(options.confirm_off_machine);
    assert!(options.confirm_predecessor_content);
    assert!(options.include_result);
}

#[test]
fn duplicates_unknown_options_and_caller_claim_keys_fail_closed() {
    let mut duplicate = start_tail();
    duplicate.extend(["--model".into(), "other".into()]);
    assert!(start(duplicate).is_err());
    assert!(step(vec!["--api-key=secret-fixture".into()]).is_err());

    let mut keyed = vec!["--idempotency-key".into(), "caller-key".into()];
    keyed.extend([
        "group".into(),
        "graph".into(),
        "run".into(),
        "controller".into(),
        "step".into(),
        "graph-run-1".into(),
    ]);
    keyed.extend(step_tail());
    assert!(parse_tokens(keyed).is_err());
    assert!(usage().contains("group graph run controller step GRAPH_RUN_ID"));
    assert!(usage().contains("--expected-awaiting-event-sha256 SHA256"));
}
