use std::path::PathBuf;

use super::{execute, write_output};
use crate::args::DeviceCommand;

fn fixture_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-pending-run-intent-v1.json")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn pending_run_intent_preview_evaluates_the_shared_fixture() {
    let output = execute(&DeviceCommand::PendingRunIntentPreview {
        input: fixture_path(),
    })
    .expect("pending Run-intent preview");
    assert_eq!(output.schema_version, "forge.pending-run-intent/v1");
    assert_eq!(
        output.evaluation_mode,
        "owner_scoped_pending_intent_preview"
    );
    assert_eq!(output.conversation_id, "conversation-001");
    assert_eq!(output.intent_id, "intent-001");
    assert_eq!(output.status, "pending");
    assert_eq!(output.timeline_event_count, 1);
    assert!(!output.replayed);
    assert!(!output.authority.device_identity_verified);
    assert!(!output.authority.run_created);
}

#[test]
fn pending_run_intent_preview_rejects_authority_and_duplicate_keys() {
    let fixture = std::fs::read_to_string(fixture_path()).expect("fixture");
    let authority = fixture.replacen("\"run_created\": false", "\"run_created\": true", 1);
    let path = tempfile::NamedTempFile::new().expect("authority fixture");
    std::fs::write(path.path(), authority).expect("write authority fixture");
    let error = execute(&DeviceCommand::PendingRunIntentPreview {
        input: path.path().to_string_lossy().into_owned(),
    })
    .expect_err("authority mutation must fail");
    assert!(error.to_string().contains("claims authority"));

    let duplicate = fixture.replacen(
        "\"schema_version\": \"forge.pending-run-intent/v1\"",
        "\"schema_version\": \"forge.pending-run-intent/v1\",\n  \"schema_version\": \"forge.pending-run-intent/v1\"",
        1,
    );
    let path = tempfile::NamedTempFile::new().expect("duplicate fixture");
    std::fs::write(path.path(), duplicate).expect("write duplicate fixture");
    let error = execute(&DeviceCommand::PendingRunIntentPreview {
        input: path.path().to_string_lossy().into_owned(),
    })
    .expect_err("duplicate keys must fail");
    assert!(error.to_string().contains("duplicate JSON keys"));
}

#[test]
fn pending_run_intent_preview_output_is_metadata_only() {
    let output = execute(&DeviceCommand::PendingRunIntentPreview {
        input: fixture_path(),
    })
    .expect("pending Run-intent preview");
    let mut bytes = Vec::new();
    write_output(&output, false, &mut bytes).expect("write preview");
    let text = String::from_utf8(bytes).expect("utf8");
    assert!(text.contains("offline pending Run-intent preview"));
    assert!(text.contains("content_bytes="));
    assert!(!text.contains("prepare the report"));
    assert!(!text.contains("run_created=true"));
}
