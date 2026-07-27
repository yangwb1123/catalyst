use std::path::{Path, PathBuf};

use super::{Command, GroupCommand, PromptCommand, SessionCommand, parse_tokens};

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
    assert!(error.contains("only valid for mutating Hub commands"));
}
