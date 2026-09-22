use std::path::Path;

use super::super::{commands::dispatch_command, state::TuiState};
use super::helpers::test_client;

#[tokio::test]
async fn remote_tui_can_preview_pending_run_intent_without_a_network_request() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-pending-run-intent-v1.json");
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut TuiState::default(),
        None,
        &format!("pending-run-intent-preview --input {}", fixture.display()),
        &mut writer,
    )
    .await
    .expect("TUI run");
    let output = String::from_utf8(writer).expect("UTF-8 output");
    assert!(
        output.contains(
            "offline pending Run-intent preview [forge.pending-run-intent/v1] conversation=conversation-001 intent=intent-001 status=pending replayed=false timeline_events=1"
        ),
        "{output}"
    );
    assert!(output.contains("content_bytes="), "{output}");
    assert!(
        output.contains("authority: device_identity_verified=false"),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_pending_run_intent_preview_requires_a_file_path() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut TuiState::default(),
        None,
        "pending-run-intent-preview --input -",
        &mut writer,
    )
    .await
    .expect("TUI run");
    let output = String::from_utf8(writer).expect("UTF-8 output");
    assert!(
        output.contains("Use pending-run-intent-preview --input FILE."),
        "{output}"
    );
}
