use std::{path::Path, process::Command};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str =
    include_str!("../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json");

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json")
}

fn run_path(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "inventory", "persisted-observation-v2", "--input"])
        .arg(path)
        .output()
        .expect("run persisted inventory observation v2")
}

#[test]
fn persisted_observation_v2_fixture_emits_lossless_shared_envelope() {
    let output = run_path(&fixture_path(), true);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    let expected: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    assert_eq!(value, expected);
    assert_eq!(
        value["devices"][0]["device"]["reservation_state"],
        "reserved"
    );
    assert_eq!(
        value["devices"][0]["device"]["gpus"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for field in [
        "execution_authorized",
        "reservation_created",
        "dispatch_performed",
    ] {
        assert_eq!(value[field], false, "v2 authority field {field}");
    }
}

#[test]
fn persisted_observation_v2_rejects_unknown_authority_and_duplicate_keys() {
    let mut authority: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    authority["execution_authorized"] = Value::Bool(true);
    let file = NamedTempFile::new().expect("temporary authority input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&authority).expect("authority JSON"),
    )
    .expect("write authority input");
    assert!(!run_path(file.path(), false).status.success());

    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let file = NamedTempFile::new().expect("temporary unknown input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&unknown).expect("unknown JSON"),
    )
    .expect("write unknown input");
    assert!(!run_path(file.path(), false).status.success());

    let file = NamedTempFile::new().expect("temporary duplicate input");
    let duplicate = FIXTURE.replacen(
        "\"evaluation_mode\"",
        "\"evaluation_mode\":\"duplicate\",\"evaluation_mode\"",
        1,
    );
    std::fs::write(file.path(), duplicate).expect("write duplicate input");
    assert!(!run_path(file.path(), false).status.success());
}

#[test]
fn persisted_observation_v2_rejects_noncanonical_tags() {
    let mut uppercase_os: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    uppercase_os["devices"][0]["device"]["os"] = Value::String("Linux".into());
    let file = NamedTempFile::new().expect("temporary uppercase OS input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&uppercase_os).expect("uppercase OS JSON"),
    )
    .expect("write uppercase OS input");
    assert!(!run_path(file.path(), false).status.success());

    let mut punctuation_runtime: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    punctuation_runtime["devices"][0]["device"]["runtimes"] = serde_json::json!(["oci/container"]);
    let file = NamedTempFile::new().expect("temporary punctuation runtime input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&punctuation_runtime).expect("punctuation runtime JSON"),
    )
    .expect("write punctuation runtime input");
    assert!(!run_path(file.path(), false).status.success());

    let mut unicode_architecture: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unicode_architecture["devices"][0]["device"]["architecture"] = Value::String("amd64☃".into());
    let file = NamedTempFile::new().expect("temporary Unicode architecture input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&unicode_architecture).expect("Unicode architecture JSON"),
    )
    .expect("write Unicode architecture input");
    assert!(!run_path(file.path(), false).status.success());
}

#[test]
fn persisted_observation_v2_enforces_capability_lease_ttl_bounds() {
    let mut below_minimum: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    below_minimum["devices"][0]["device"]["lease_expires_at_ms"] = 100_999_u64.into();
    let file = NamedTempFile::new().expect("temporary short lease input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&below_minimum).expect("short lease JSON"),
    )
    .expect("write short lease input");
    assert!(!run_path(file.path(), false).status.success());

    let mut above_maximum: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    above_maximum["devices"][0]["device"]["lease_expires_at_ms"] = 700_001_u64.into();
    let file = NamedTempFile::new().expect("temporary long lease input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&above_maximum).expect("long lease JSON"),
    )
    .expect("write long lease input");
    assert!(!run_path(file.path(), false).status.success());

    let mut minimum: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    minimum["devices"][0]["device"]["lease_expires_at_ms"] = 101_000_u64.into();
    let file = NamedTempFile::new().expect("temporary minimum lease input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&minimum).expect("minimum lease JSON"),
    )
    .expect("write minimum lease input");
    assert!(run_path(file.path(), false).status.success());

    let mut maximum: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    maximum["devices"][0]["device"]["lease_expires_at_ms"] = 700_000_u64.into();
    let file = NamedTempFile::new().expect("temporary maximum lease input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&maximum).expect("maximum lease JSON"),
    )
    .expect("write maximum lease input");
    assert!(run_path(file.path(), false).status.success());
}

#[test]
fn persisted_observation_v2_text_output_displays_reservation_and_gpu_declarations() {
    let output = run_path(&fixture_path(), false);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("text output");
    assert!(
        text.contains("offline persisted inventory observation v2"),
        "{text}"
    );
    assert!(text.contains("reservation=reserved"), "{text}");
    assert!(
        text.contains("gpus=[gpu-a:12884901888,gpu-b:4294967296]"),
        "{text}"
    );
    assert!(text.contains("execution_authorized=false"), "{text}");
    assert!(!text.contains("/api/v1/devices"));
}
