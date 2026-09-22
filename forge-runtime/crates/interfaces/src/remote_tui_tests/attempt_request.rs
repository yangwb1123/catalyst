use std::path::Path;

use super::super::{commands::dispatch_command, state::TuiState};
use super::helpers::test_client;

#[tokio::test]
async fn remote_tui_can_preview_attempt_request_without_a_network_request() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-attempt-request-v1.json");
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut TuiState::default(),
        None,
        &format!("attempt-request-preview --input {}", fixture.display()),
        &mut writer,
    )
    .await
    .expect("TUI run");
    let output = String::from_utf8(writer).expect("UTF-8 output");
    assert!(
        output.contains(
            "offline Attempt request preview [forge.attempt-request/v1] cases=18 accepted=1 rejected=17"
        ),
        "{output}"
    );
    assert!(
        output.contains("valid_normalizes_order: accepted=true"),
        "{output}"
    );
    assert!(output.contains("error=reference_mismatch"), "{output}");
    assert!(
        output.contains("authority: device_identity_verified=false references_resolved=false"),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_attempt_request_preview_requires_a_file_path() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut TuiState::default(),
        None,
        "attempt-request-preview --input -",
        &mut writer,
    )
    .await
    .expect("TUI run");
    let output = String::from_utf8(writer).expect("UTF-8 output");
    assert!(
        output.contains("Use attempt-request-preview --input FILE."),
        "{output}"
    );
}
