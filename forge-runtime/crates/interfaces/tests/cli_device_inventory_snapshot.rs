use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-snapshot-canonical-v1.json"
);

fn run_path(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "inventory", "snapshot-canonical", "--input"])
        .arg(path);
    command
        .output()
        .expect("run device inventory snapshot-canonical")
}

fn assert_read_only_output(value: &Value) {
    assert_eq!(value["type"], "device_inventory_snapshot_canonical");
    assert_eq!(
        value["schema_version"],
        "forge.device-inventory-snapshot-canonical/v1"
    );
    assert_eq!(value["evaluation_mode"], "pure_owner_scoped_snapshot_only");
    assert_eq!(value["owner_declaration_unverified"], true);
    assert_eq!(value["inventory_declarations_unverified"], true);
    assert_eq!(value["cases"].as_array().map(Vec::len), Some(6));
    assert_eq!(
        value["cases"][0]["ordered_keys"],
        serde_json::json!([
            "device-a/runner-a",
            "device-a/runner-z",
            "device-b/runner-z"
        ])
    );
    assert_eq!(
        value["cases"][0]["canonical_sha256"],
        "6e2bfe4ae401c1cf00f4cbbd9cb6c0998df9123ecee3e16555e4f8f8116e0d44"
    );
    assert_eq!(value["cases"][2]["error"], "owner_mismatch");
    assert_eq!(value["cases"][3]["error"], "duplicate_row");
    for field in [
        "identity_verified",
        "heartbeat_persisted",
        "inventory_authoritative",
        "reservation_created",
        "execution_authorized",
        "dispatch_performed",
    ] {
        assert_eq!(value["authority"][field], false, "authority field {field}");
    }
}

#[test]
fn snapshot_fixture_is_canonicalized_in_stable_json_output() {
    let fixture = NamedTempFile::new().expect("temporary fixture");
    fixture
        .as_file()
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = run_path(fixture.path(), true);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_read_only_output(&value);
}

#[test]
fn snapshot_supports_bounded_stdin_without_hub_or_device_request() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "snapshot-canonical",
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn device inventory snapshot-canonical");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = child.wait_with_output().expect("wait for CLI");
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_read_only_output(&value);
}

#[test]
fn unknown_duplicate_oversized_authority_and_expectation_mutations_fail_closed() {
    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let unknown_file = NamedTempFile::new().expect("temporary unknown input");
    std::fs::write(
        unknown_file.path(),
        serde_json::to_vec(&unknown).expect("unknown field JSON"),
    )
    .expect("write unknown field input");
    assert!(!run_path(unknown_file.path(), false).status.success());

    let mut duplicate: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let first = duplicate["cases"][0].clone();
    duplicate["cases"].as_array_mut().unwrap().push(first);
    let duplicate_file = NamedTempFile::new().expect("temporary duplicate input");
    std::fs::write(
        duplicate_file.path(),
        serde_json::to_vec(&duplicate).expect("duplicate JSON"),
    )
    .expect("write duplicate input");
    assert!(!run_path(duplicate_file.path(), false).status.success());

    let mut authorized: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    authorized["authority"]["dispatch_performed"] = Value::Bool(true);
    let authorized_file = NamedTempFile::new().expect("temporary authority input");
    std::fs::write(
        authorized_file.path(),
        serde_json::to_vec(&authorized).expect("authority JSON"),
    )
    .expect("write authority input");
    assert!(!run_path(authorized_file.path(), false).status.success());

    let mut mismatch: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    mismatch["cases"][0]["expected"]["canonical_sha256"] = Value::String("0".repeat(64));
    let mismatch_file = NamedTempFile::new().expect("temporary mismatch input");
    std::fs::write(
        mismatch_file.path(),
        serde_json::to_vec(&mismatch).expect("mismatch JSON"),
    )
    .expect("write mismatch input");
    assert!(!run_path(mismatch_file.path(), false).status.success());

    let oversized_file = NamedTempFile::new().expect("temporary oversized input");
    std::fs::write(oversized_file.path(), vec![b'x'; 2 * 1024 * 1024 + 1])
        .expect("write oversized input");
    assert!(!run_path(oversized_file.path(), false).status.success());
}

#[test]
fn duplicate_json_keys_fail_closed_at_root_and_nested_depths() {
    let duplicate_root = FIXTURE.replacen(
        r#"  "schema_version": "forge.device-inventory-snapshot-canonical/v1","#,
        r#"  "schema_version": "forge.device-inventory-snapshot-canonical/v1",
  "schema_version": "forge.device-inventory-snapshot-canonical/v1","#,
        1,
    );
    let root_file = NamedTempFile::new().expect("temporary duplicate root input");
    root_file
        .as_file()
        .write_all(duplicate_root.as_bytes())
        .expect("write duplicate root input");
    let root_output = run_path(root_file.path(), false);
    assert!(!root_output.status.success());
    assert!(String::from_utf8_lossy(&root_output.stderr).contains("duplicate JSON object key"));

    let duplicate_nested = FIXTURE.replacen(
        r#"        "snapshot_id": "snapshot-1","#,
        r#"        "snapshot_id": "snapshot-1",
        "snapshot_id": "snapshot-1","#,
        1,
    );
    let nested_file = NamedTempFile::new().expect("temporary duplicate nested input");
    nested_file
        .as_file()
        .write_all(duplicate_nested.as_bytes())
        .expect("write duplicate nested input");
    let nested_output = run_path(nested_file.path(), false);
    assert!(!nested_output.status.success());
    assert!(String::from_utf8_lossy(&nested_output.stderr).contains("duplicate JSON object key"));
}
