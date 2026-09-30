use super::*;
use crate::args::parse_tokens;

#[path = "remote_args/changes_tests.rs"]
mod changes_tests;
#[path = "remote_args/execution_preview_tests.rs"]
mod execution_preview_tests;
#[path = "remote_args/pending_run_intents_tests.rs"]
mod pending_run_intents_tests;
#[path = "remote_args/runner_preview_tests.rs"]
mod runner_preview_tests;
#[path = "remote_args/runs_tests.rs"]
mod runs_tests;
#[path = "remote_args/sessions_tests.rs"]
mod sessions_tests;

#[test]
fn remote_login_and_tui_commands_reject_unsupported_options() {
    assert_eq!(
        parse_tokens(["remote", "login"].map(str::to_owned))
            .unwrap()
            .command,
        Command::Remote(RemoteCommand::Login)
    );
    assert!(parse_tokens(["remote", "login", "extra"].map(str::to_owned)).is_err());
    let args = parse_tokens(["remote", "tui"].map(str::to_owned)).unwrap();
    assert_eq!(args.command, Command::Remote(RemoteCommand::Tui));
    assert!(parse_tokens(["--json", "remote", "tui"].map(str::to_owned)).is_err());
    assert!(
        parse_tokens(["--state-dir", "/tmp/local", "remote", "tui"].map(str::to_owned)).is_err()
    );

    let local_scope =
        parse_tokens(["-C", "/workspace", "remote", "sessions", "list"].map(str::to_owned));
    assert!(local_scope.unwrap_err().contains("project/group selectors"));
}

#[test]
fn remote_credentials_status_is_a_local_read_only_command() {
    let args = parse_tokens(["remote", "credentials", "status"].map(str::to_owned)).unwrap();
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::CredentialStorageStatus)
    );
    assert!(parse_tokens(["remote", "credentials"].map(str::to_owned)).is_err());
    assert!(parse_tokens(["remote", "credentials", "status", "extra"].map(str::to_owned)).is_err());
}

#[test]
fn remote_inventory_show_is_a_bounded_read_without_options() {
    assert_eq!(
        parse_remote_command(&["remote", "inventory", "show"]).unwrap(),
        RemoteCommand::InventoryShow
    );
    assert!(parse_remote_command(&["remote", "inventory"]).is_err());
    assert!(parse_remote_command(&["remote", "inventory", "show", "extra"]).is_err());
}

#[test]
fn remote_lifecycle_registry_is_an_explicit_read_only_candidate() {
    assert_eq!(
        parse_remote_command(&["remote", "lifecycle-registry", "show"]).unwrap(),
        RemoteCommand::LifecycleRegistryShow
    );
    assert_eq!(
        parse_remote_command(&["remote", "lifecycle-registry", "read"]).unwrap(),
        RemoteCommand::LifecycleRegistryShow
    );
    assert!(parse_remote_command(&["remote", "lifecycle-registry"]).is_err());
    assert!(parse_remote_command(&["remote", "lifecycle-registry", "put"]).is_err());
    assert!(parse_remote_command(&["remote", "lifecycle-registry", "show", "extra"]).is_err());
}

#[test]
fn remote_inventory_show_v2_is_an_explicit_candidate_read() {
    assert_eq!(
        parse_remote_command(&["remote", "inventory", "show-v2"]).unwrap(),
        RemoteCommand::InventoryShowV2
    );
    assert!(parse_remote_command(&["remote", "inventory", "show-v2", "extra"]).is_err());
}

#[test]
fn remote_inventory_show_converged_is_an_explicit_pair_read() {
    assert_eq!(
        parse_remote_command(&["remote", "inventory", "show-converged"]).unwrap(),
        RemoteCommand::InventoryShowConverged
    );
    assert!(parse_remote_command(&["remote", "inventory", "show-converged", "extra"]).is_err());
}

#[test]
fn remote_client_instance_views_parse_as_exact_read_only_commands() {
    assert_eq!(
        parse_remote_command(&["remote", "client-instances", "session-view"]).unwrap(),
        RemoteCommand::ClientInstanceSessionView
    );
    assert_eq!(
        parse_remote_command(&["remote", "client-instances", "resource-view"]).unwrap(),
        RemoteCommand::ClientInstanceResourceView
    );
    assert!(parse_remote_command(&["remote", "client-instances"]).is_err());
    assert!(parse_remote_command(&["remote", "client-instances", "unknown"]).is_err());
    assert!(
        parse_remote_command(&["remote", "client-instances", "session-view", "extra",]).is_err()
    );
}

#[test]
fn remote_client_instance_show_converged_is_an_explicit_pair_read() {
    assert_eq!(
        parse_remote_command(&["remote", "client-instances", "show-converged"]).unwrap(),
        RemoteCommand::ClientInstancesShowConverged
    );
    assert!(
        parse_remote_command(&["remote", "client-instances", "show-converged", "extra",]).is_err()
    );
}

fn parse_remote_command(tokens: &[&str]) -> Result<RemoteCommand, String> {
    parse_tokens(tokens.iter().map(|token| (*token).to_owned())).map(|args| match args.command {
        Command::Remote(command) => command,
        _ => unreachable!("remote argument helper parsed a non-remote command"),
    })
}

#[test]
fn remote_mutations_require_explicit_idempotency_keys() {
    for tokens in [
        vec!["remote", "sessions", "create"],
        vec![
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "1",
            "hello",
        ],
    ] {
        let error = parse_tokens(tokens.into_iter().map(str::to_owned)).unwrap_err();
        assert!(error.contains("explicit --idempotency-key"));
    }
}

#[test]
fn remote_prompt_commands_keep_prompt_text_and_reject_local_scope_selection() {
    let args = parse_tokens(
        [
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "1",
            "hello",
            "world",
        ]
        .map(str::to_owned),
    );
    assert!(args.is_err());

    let args = parse_tokens(
        [
            "--idempotency-key",
            "prompt-1",
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "1",
            "hello",
            "world",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::PromptsAdd {
            conversation_id: "conversation-1".into(),
            expected_version: 1,
            content: "hello world".into(),
            instance_id: None,
            instance_view: None,
        })
    );

    let scoped =
        parse_tokens(["-C", "/workspace", "remote", "sessions", "list"].map(str::to_owned))
            .unwrap_err();
    assert!(scoped.contains("project/group selectors"));
}

#[cfg(test)]
#[path = "remote_args/prompt_tests.rs"]
mod prompt_tests;

#[path = "remote_args/local_runner_preview_tests.rs"]
mod local_runner_preview_tests;
#[cfg(test)]
#[path = "remote_args/placement_tests.rs"]
mod placement_tests;
#[path = "remote_args/run_attempt_lease_dispatch_preflight_tests.rs"]
mod run_attempt_lease_dispatch_preflight_tests;
#[cfg(test)]
#[path = "remote_args/run_execution_evidence_tests.rs"]
mod run_execution_evidence_tests;
#[path = "remote_args/run_timeline_tests.rs"]
mod run_timeline_tests;
#[cfg(test)]
#[path = "remote_args/session_observation_tests.rs"]
mod session_observation_tests;
#[cfg(test)]
#[path = "remote_args/session_runner_receipt_history_tests.rs"]
mod session_runner_receipt_history_tests;
#[cfg(test)]
#[path = "remote_args/session_runner_receipt_tests.rs"]
mod session_runner_receipt_tests;
