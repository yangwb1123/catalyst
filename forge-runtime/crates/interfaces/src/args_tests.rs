use std::path::{Path, PathBuf};

use super::{Command, GroupCommand, PromptCommand, RunCommand, SessionCommand, parse_tokens};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

#[test]
fn no_arguments_enters_the_global_hub() {
    let args = parse(&[]);
    assert_eq!(args.project, None);
    assert_eq!(args.command, Command::Hub);
}

#[test]
fn a_bare_path_enters_its_project_hub() {
    let args = parse(&["./frontend"]);
    assert_eq!(args.project.as_deref(), Some(Path::new("./frontend")));
    assert_eq!(args.command, Command::Hub);
}

#[test]
fn project_option_scopes_a_new_session() {
    let args = parse(&["-C", "/srv/api", "session", "new", "--title", "API"]);
    assert_eq!(args.project.as_deref(), Some(Path::new("/srv/api")));
    assert_eq!(
        args.command,
        Command::Session(SessionCommand::New {
            title: "API".into()
        })
    );
}

#[test]
fn group_option_scopes_a_new_session() {
    let args = parse(&["--group", "group-1", "session", "new", "SSO rollout"]);
    assert_eq!(args.group.as_deref(), Some("group-1"));
    assert_eq!(
        args.command,
        Command::Session(SessionCommand::New {
            title: "SSO rollout".into()
        })
    );
}

#[test]
fn prompt_add_preserves_the_remaining_text() {
    let args = parse(&["prompt", "add", "session-1", "connect", "SSO"]);
    assert_eq!(
        args.command,
        Command::Prompt(PromptCommand::Add {
            conversation_id: "session-1".into(),
            text: "connect SSO".into()
        })
    );
}

#[test]
fn group_add_accepts_a_project_role() {
    let args = parse(&["group", "add", "group-1", "../identity", "--role", "sso"]);
    assert_eq!(
        args.command,
        Command::Group(GroupCommand::Add {
            group_id: "group-1".into(),
            project: PathBuf::from("../identity"),
            role: "sso".into()
        })
    );
}

#[test]
fn group_context_defaults_to_a_content_manifest_and_accepts_explicit_bounds() {
    let manifest = parse(&["group", "context", "group-1"]);
    assert_eq!(
        manifest.command,
        Command::Group(GroupCommand::Context {
            group_id: "group-1".into(),
            include_content: false,
            max_bytes: forge_runtime_application::DEFAULT_GROUP_CONTEXT_CONTENT_BYTES,
        })
    );
    let content = parse(&[
        "group",
        "context",
        "group-1",
        "--include-content",
        "--max-bytes",
        "1024",
    ]);
    assert_eq!(
        content.command,
        Command::Group(GroupCommand::Context {
            group_id: "group-1".into(),
            include_content: true,
            max_bytes: 1_024,
        })
    );
}

#[test]
fn group_context_rejects_an_option_in_place_of_group_id() {
    let error = parse_tokens(
        ["group", "context", "--include-content"]
            .into_iter()
            .map(str::to_owned),
    )
    .expect_err("GROUP_ID is required");

    assert!(error.contains("requires GROUP_ID"));
}

#[test]
fn the_original_demo_form_remains_compatible() {
    let args = parse(&[
        "--workspace",
        "..",
        "--read",
        "README.md",
        "Inspect",
        "README.md",
    ]);
    let Command::Demo(demo) = args.command else {
        panic!("legacy invocation must remain a demo");
    };
    assert_eq!(demo.read_path, "README.md");
    assert_eq!(demo.prompt, "Inspect README.md");
}

#[test]
fn a_prompt_cannot_be_mistaken_for_a_project_path() {
    let error = parse_tokens(["not-a-path", "not-a-command"].map(str::to_owned))
        .expect_err("ambiguous input is rejected");
    assert!(error.contains("expected a command after the project path"));
}

#[test]
fn management_commands_reject_ignored_space_selectors() {
    for tokens in [
        vec!["--group", "missing", "group", "list"],
        vec!["-C", "missing", "group", "list"],
        vec!["--group", "missing", "prompt", "list"],
    ] {
        let error = parse_tokens(tokens.into_iter().map(str::to_owned))
            .expect_err("ignored selectors must fail");
        assert!(error.contains("selectors are not valid"));
    }
}

#[test]
fn a_mutation_accepts_a_reusable_idempotency_key() {
    let args = parse(&["--idempotency-key", "retry-session-42", "session", "new"]);
    assert_eq!(args.idempotency_key.as_deref(), Some("retry-session-42"));
}

#[test]
fn a_read_command_rejects_an_idempotency_key() {
    let error = parse_tokens(["--idempotency-key", "unused", "session", "list"].map(str::to_owned))
        .expect_err("a read cannot consume a mutation key");
    assert!(error.contains("only valid for mutating commands"));
}

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
