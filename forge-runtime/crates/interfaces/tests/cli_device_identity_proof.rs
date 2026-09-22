use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-identity-proof-contract-v1.json"
);

fn run_path(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "identity-proof-preview", "--input"])
        .arg(path)
        .output()
        .expect("run identity proof preview")
}

fn assert_preview(value: &Value) {
    assert_eq!(value["type"], "device_identity_proof_preview");
    assert_eq!(
        value["schema_version"],
        "forge.device-identity-proof-contract/v1"
    );
    assert_eq!(value["evaluation_mode"], "pure_binding_only");
    assert_eq!(value["device_id"], "device-a");
    assert_eq!(value["cases"].as_array().map(Vec::len), Some(12));
    assert_eq!(value["cases"][0]["accepted"], true);
    assert_eq!(value["cases"][0]["reason"], "bound_approved");
    assert_eq!(value["cases"][1]["approval_required"], true);
    assert_eq!(value["cases"][2]["reason"], "owner_mismatch");
    assert_eq!(value["cases"][4]["reason"], "device_mismatch");
    assert_eq!(value["cases"][6]["reason"], "challenge_mismatch");
    assert_eq!(value["cases"][7]["reason"], "challenge_replayed");
    assert_eq!(value["cases"][10]["reason"], "credential_revoked");
    for field in [
        "identity_verified",
        "challenge_consumed",
        "enrollment_persisted",
        "owner_approval_recorded",
        "credential_issued",
        "inventory_authoritative",
        "execution_authorized",
    ] {
        assert_eq!(value["authority"][field], false, "authority field {field}");
    }
}

#[test]
fn identity_proof_fixture_is_evaluated_as_pure_json() {
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
    assert_preview(&value);
}

#[test]
fn identity_proof_preview_supports_bounded_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--json", "device", "identity-proof-preview", "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn identity proof preview");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = child.wait_with_output().expect("wait for CLI");
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_preview(&value);
}

#[test]
fn identity_proof_preview_rejects_authority_unknown_and_duplicate_keys() {
    let mut authorized: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    authorized["authority"]["identity_verified"] = Value::Bool(true);
    let authorized_file = NamedTempFile::new().expect("temporary authority input");
    std::fs::write(
        authorized_file.path(),
        serde_json::to_vec(&authorized).expect("authority JSON"),
    )
    .expect("write authority input");
    assert!(!run_path(authorized_file.path(), false).status.success());

    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let unknown_file = NamedTempFile::new().expect("temporary unknown input");
    std::fs::write(
        unknown_file.path(),
        serde_json::to_vec(&unknown).expect("unknown JSON"),
    )
    .expect("write unknown input");
    assert!(!run_path(unknown_file.path(), false).status.success());

    let duplicate = FIXTURE.replacen(
        r#"  "schema_version": "forge.device-identity-proof-contract/v1","#,
        r#"  "schema_version": "forge.device-identity-proof-contract/v1",
  "schema_version": "forge.device-identity-proof-contract/v1","#,
        1,
    );
    let duplicate_file = NamedTempFile::new().expect("temporary duplicate input");
    duplicate_file
        .as_file()
        .write_all(duplicate.as_bytes())
        .expect("write duplicate input");
    let output = run_path(duplicate_file.path(), false);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate JSON object key"));
}

#[test]
fn identity_proof_preview_rejects_fixture_envelope_drift() {
    let cases = [
        ("notice", "changed notice"),
        ("device_id", "foreign-device"),
        ("key_id", "foreign-key"),
    ];
    for (field, value) in cases {
        let mut mutated: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        match field {
            "notice" => mutated["notice"] = Value::String((*value).to_owned()),
            "device_id" => mutated["device"]["device_id"] = Value::String((*value).to_owned()),
            "key_id" => mutated["device"]["key_id"] = Value::String((*value).to_owned()),
            _ => unreachable!(),
        }
        let file = NamedTempFile::new().expect("temporary envelope input");
        std::fs::write(
            file.path(),
            serde_json::to_vec(&mutated).expect("envelope JSON"),
        )
        .expect("write envelope input");
        assert!(!run_path(file.path(), false).status.success(), "{field}");
    }

    let mut owner_drift: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    owner_drift["device"]["owner"]["subject"] = Value::String("foreign-user".into());
    let owner_file = NamedTempFile::new().expect("temporary owner input");
    std::fs::write(
        owner_file.path(),
        serde_json::to_vec(&owner_drift).expect("owner JSON"),
    )
    .expect("write owner input");
    assert!(!run_path(owner_file.path(), false).status.success());

    let mut challenge_drift: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    challenge_drift["challenge"]["consumed"] = Value::Bool(true);
    let challenge_file = NamedTempFile::new().expect("temporary challenge input");
    std::fs::write(
        challenge_file.path(),
        serde_json::to_vec(&challenge_drift).expect("challenge JSON"),
    )
    .expect("write challenge input");
    assert!(!run_path(challenge_file.path(), false).status.success());

    let mut case_drift: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    case_drift["cases"]
        .as_array_mut()
        .expect("cases array")
        .pop();
    let case_file = NamedTempFile::new().expect("temporary cases input");
    std::fs::write(
        case_file.path(),
        serde_json::to_vec(&case_drift).expect("cases JSON"),
    )
    .expect("write cases input");
    assert!(!run_path(case_file.path(), false).status.success());
}
