use std::{fs, path::Path};

use serde_json::Value;
use tempfile::NamedTempFile;

use super::super::{commands::dispatch_command, state::TuiState};
use super::helpers::test_client;

fn fixture() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/contracts/fixtures/forge-runner-attempt-boundary-v1.json"),
    )
    .expect("Runner Attempt boundary fixture")
}

async fn dispatch(input: &str) -> String {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut TuiState::default(),
        None,
        &format!("runner-attempt-boundary-preview --input {input}"),
        &mut writer,
    )
    .await
    .expect("TUI command");
    String::from_utf8(writer).expect("UTF-8 output")
}

#[tokio::test]
async fn remote_tui_can_preview_runner_attempt_boundary_without_a_network_request() {
    let input = NamedTempFile::new().unwrap();
    fs::write(input.path(), fixture()).unwrap();

    let output = dispatch(&input.path().display().to_string()).await;

    assert!(
        output
            .contains("offline Runner Attempt boundary preview [forge.runner-attempt-boundary/v1]"),
        "{output}"
    );
    assert!(output.contains("conversation=conversation-1 run=run-1 attempt=attempt-1"));
    assert!(output.contains(
        "lifecycle: accepted -> starting transition=begin_starting execution_boundary_ready=true transition_valid=true transition_dispatchable=true attempt_boundary_ready=true"
    ));
    assert!(output.contains("preview_only=true authority: attempt_persisted=false"));
    assert!(!output.contains("argv"));
    assert!(!output.contains("fencing_token"));
}

#[tokio::test]
async fn remote_tui_runner_attempt_boundary_requires_a_file_path() {
    let output = dispatch("-").await;

    assert!(
        output.contains("Use runner-attempt-boundary-preview --input FILE."),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_runner_attempt_boundary_rejects_wire_and_derived_drift() {
    let original = fixture();
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    let cases = [
        ("unexpected", Value::Bool(true)),
        ("attempt_boundary_ready", Value::Bool(false)),
        ("execution_boundary_ready", Value::Bool(false)),
        (
            "current_attempt_state",
            Value::String("completed".to_owned()),
        ),
        ("preview_only", Value::Bool(false)),
    ];

    for (key, replacement) in cases {
        let mut candidate = value.clone();
        candidate[key] = replacement;
        let input = NamedTempFile::new().unwrap();
        fs::write(input.path(), serde_json::to_vec(&candidate).unwrap()).unwrap();
        let output = dispatch(&input.path().display().to_string()).await;
        assert!(
            output.contains("Runner Attempt boundary preview failed:"),
            "{key}: {output}"
        );
    }

    let duplicate = NamedTempFile::new().unwrap();
    fs::write(
        duplicate.path(),
        br#"{"schema_version":"forge.runner-attempt-boundary/v1","schema_version":"forge.runner-attempt-boundary/v1"}"#,
    )
    .unwrap();
    let output = dispatch(&duplicate.path().display().to_string()).await;
    assert!(
        output.contains("Runner Attempt boundary preview failed:"),
        "{output}"
    );

    let trailing = NamedTempFile::new().unwrap();
    let mut bytes = original;
    bytes.extend_from_slice(b" {}\n");
    fs::write(trailing.path(), bytes).unwrap();
    let output = dispatch(&trailing.path().display().to_string()).await;
    assert!(
        output.contains("Runner Attempt boundary preview failed:"),
        "{output}"
    );

    value["authority"]["dispatch_performed"] = Value::Bool(true);
    let authority = NamedTempFile::new().unwrap();
    fs::write(authority.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    let output = dispatch(&authority.path().display().to_string()).await;
    assert!(
        output.contains("Runner Attempt boundary preview failed:"),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_runner_attempt_boundary_rejects_an_oversized_file() {
    let input = NamedTempFile::new().unwrap();
    fs::write(input.path(), vec![b' '; 2 * 1024 * 1024 + 1]).unwrap();

    let output = dispatch(&input.path().display().to_string()).await;

    assert!(
        output.contains("Runner Attempt boundary preview failed:"),
        "{output}"
    );
    assert!(output.contains("exceeds 2097152 bytes"), "{output}");
}
