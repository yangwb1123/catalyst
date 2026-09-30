use super::*;

#[test]
fn a_project_run_binds_conversation_prompt_and_read_target() {
    let args = parse(&[
        "-C",
        "/srv/api",
        "--idempotency-key",
        "run-retry-1",
        "run",
        "start",
        "session-1",
        "prompt-1",
        "--read",
        "src/lib.rs",
    ]);
    assert_eq!(
        args.command,
        Command::Run(RunCommand::Start {
            conversation_id: "session-1".into(),
            prompt_id: "prompt-1".into(),
            read_path: "src/lib.rs".into(),
            allowed_read_paths: Vec::new(),
            live: false,
            model: None,
            max_output_tokens: 4_096,
        })
    );
}

#[test]
fn live_run_requires_explicit_controls_and_defaults_to_no_read_consent() {
    let args = parse(&[
        "-C",
        "/srv/api",
        "--idempotency-key",
        "live-run-1",
        "run",
        "start",
        "session-1",
        "prompt-1",
        "--live",
        "--model",
        "explicit-model",
        "--max-output-tokens",
        "2048",
    ]);
    assert_eq!(
        args.command,
        Command::Run(RunCommand::Start {
            conversation_id: "session-1".into(),
            prompt_id: "prompt-1".into(),
            read_path: "README.md".into(),
            allowed_read_paths: Vec::new(),
            live: true,
            model: Some("explicit-model".into()),
            max_output_tokens: 2_048,
        })
    );
}

#[test]
fn unsafe_or_misleading_live_combinations_are_rejected() {
    let no_key = [
        "-C", "/srv/api", "run", "start", "session", "prompt", "--live",
    ];
    let read_live = [
        "-C",
        "/srv/api",
        "--idempotency-key",
        "key",
        "run",
        "start",
        "session",
        "prompt",
        "--live",
        "--read",
        "README.md",
    ];
    let model_offline = [
        "-C", "/srv/api", "run", "start", "session", "prompt", "--model", "model",
    ];
    let allow_read_offline = [
        "-C",
        "/srv/api",
        "run",
        "start",
        "session",
        "prompt",
        "--allow-read",
        "README.md",
    ];

    assert!(parse_error(&no_key).contains("explicit --idempotency-key"));
    assert!(parse_error(&read_live).contains("cannot be combined"));
    assert!(parse_error(&model_offline).contains("require --live"));
    assert!(parse_error(&allow_read_offline).contains("only valid with --live"));
}

fn parse_error(tokens: &[&str]) -> String {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect_err("arguments must fail")
}

#[test]
fn live_read_consent_is_repeatable_and_preserves_exact_paths() {
    let args = parse(&[
        "-C",
        "/srv/api",
        "--idempotency-key",
        "live-run-reads",
        "run",
        "start",
        "session-1",
        "prompt-1",
        "--live",
        "--allow-read",
        ".env",
        "--allow-read",
        "proc/self/environ",
    ]);
    let Command::Run(RunCommand::Start {
        allowed_read_paths, ..
    }) = args.command
    else {
        panic!("run start command");
    };

    assert_eq!(allowed_read_paths, [".env", "proc/self/environ"]);
}

#[test]
fn live_read_consent_rejects_unclean_duplicate_or_oversized_paths() {
    for path in ["/etc/passwd", "../.env", "./.env", "src//lib.rs", ""] {
        let error = parse_error(&[
            "-C",
            "/srv/api",
            "--idempotency-key",
            "live-run-invalid-read",
            "run",
            "start",
            "session-1",
            "prompt-1",
            "--live",
            "--allow-read",
            path,
        ]);
        assert!(error.contains("--allow-read"), "{error}");
    }
    assert_duplicate_live_read_rejected();
    assert_oversized_live_read_rejected();
}

fn assert_duplicate_live_read_rejected() {
    let duplicate = parse_error(&[
        "-C",
        "/srv/api",
        "--idempotency-key",
        "live-run-duplicate-read",
        "run",
        "start",
        "session-1",
        "prompt-1",
        "--live",
        "--allow-read",
        "README.md",
        "--allow-read",
        "README.md",
    ]);
    assert!(duplicate.contains("specified more than once"));
}

fn assert_oversized_live_read_rejected() {
    let oversized = "x".repeat(1_025);
    let oversized_error = parse_error(&[
        "-C",
        "/srv/api",
        "--idempotency-key",
        "live-run-long-read",
        "run",
        "start",
        "session-1",
        "prompt-1",
        "--live",
        "--allow-read",
        &oversized,
    ]);
    assert!(oversized_error.contains("at most 1024 bytes"));
}

#[test]
fn live_read_consent_is_limited_to_thirty_two_files() {
    let mut tokens = [
        "-C",
        "/srv/api",
        "--idempotency-key",
        "live-run-many-reads",
        "run",
        "start",
        "session-1",
        "prompt-1",
        "--live",
    ]
    .map(str::to_owned)
    .to_vec();
    for index in 0..33 {
        tokens.push("--allow-read".into());
        tokens.push(format!("file-{index}.txt"));
    }

    let error = parse_tokens(tokens).expect_err("the thirty-third read must fail");
    assert!(error.contains("at most 32 times"));
}

#[test]
fn run_queries_parse_without_a_space_selector() {
    assert_eq!(
        parse(&["run", "list", "session-1", "--limit", "7"]).command,
        Command::Run(RunCommand::List {
            conversation_id: Some("session-1".into()),
            limit: 7,
        })
    );
    assert_eq!(
        parse(&["run", "show", "run-1"]).command,
        Command::Run(RunCommand::Show {
            run_id: "run-1".into(),
        })
    );
    assert_eq!(
        parse(&["run", "explain", "run-1"]).command,
        Command::Run(RunCommand::Explain {
            run_id: "run-1".into(),
        })
    );
    assert_eq!(
        parse(&["-C", "/srv/api", "run", "resume", "run-1"]).command,
        Command::Run(RunCommand::Resume {
            run_id: "run-1".into(),
        })
    );
    assert_eq!(
        parse(&[
            "--idempotency-key",
            "restart-key",
            "-C",
            "/srv/api",
            "run",
            "restart",
            "run-1",
        ])
        .command,
        Command::Run(RunCommand::Restart {
            run_id: "run-1".into(),
        })
    );
}

#[test]
fn run_execution_requires_a_project_and_queries_reject_selectors() {
    let missing_project =
        parse_tokens(["run", "start", "session-1", "prompt-1"].map(str::to_owned))
            .expect_err("run start needs an explicit workspace");
    assert!(missing_project.contains("requires -C/--project"));

    let missing_resume_project = parse_tokens(["run", "resume", "run-1"].map(str::to_owned))
        .expect_err("run resume needs an explicit workspace");
    assert!(missing_resume_project.contains("run resume requires -C/--project"));

    let missing_restart_key =
        parse_tokens(["-C", "/srv/api", "run", "restart", "run-1"].map(str::to_owned))
            .expect_err("run restart needs an explicit key");
    assert!(missing_restart_key.contains("explicit --idempotency-key"));

    let missing_restart_project = parse_tokens(
        [
            "--idempotency-key",
            "restart-key",
            "run",
            "restart",
            "run-1",
        ]
        .map(str::to_owned),
    )
    .expect_err("run restart needs an explicit workspace");
    assert!(missing_restart_project.contains("run restart requires -C/--project"));

    let selected_query =
        parse_tokens(["-C", "/srv/api", "run", "show", "run-1"].map(str::to_owned))
            .expect_err("run queries cannot ignore a selector");
    assert!(selected_query.contains("selectors are not valid"));

    let selected_explanation =
        parse_tokens(["-C", "/srv/api", "run", "explain", "run-1"].map(str::to_owned))
            .expect_err("run explanations cannot ignore a selector");
    assert!(selected_explanation.contains("selectors are not valid"));
}
