use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

const FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v2.json"
);

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v2.json",
    )
}

fn run_path(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "inventory", "placement-evaluation-v2", "--input"])
        .arg(path)
        .output()
        .expect("run v2 placement evaluation")
}

#[test]
fn placement_evaluation_v2_fixture_is_compared_without_selecting() {
    let output = run_path(&fixture_path(), true);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(
        value["schema_version"],
        "forge.device-inventory-placement-evaluation/v2"
    );
    assert_eq!(value["evaluation_mode"], "offline_static_only");
    assert_eq!(value["eligible_candidate_count"], 0);
    assert_eq!(value["selected_device_id"], Value::Null);
    assert_eq!(value["selected_instance_id"], Value::Null);
    assert_eq!(value["decisions"][0]["reservation_state"], "reserved");
    assert_eq!(value["decisions"][0]["gpu_count"], 2);
    assert_eq!(
        value["decisions"][0]["available_gpu_memory_bytes"],
        17179869184_u64
    );
    assert_eq!(value["decisions"][0]["matches_requirements"], false);
    assert_eq!(value["authority"]["placement_selected"], false);
    assert_eq!(value["authority"]["reservation_created"], false);
    assert_eq!(value["authority"]["execution_authorized"], false);
}

#[test]
fn placement_evaluation_v2_rejects_unknown_authority_and_duplicate_keys() {
    let mut authorized: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    authorized["authority"]["placement_selected"] = Value::Bool(true);
    let file = NamedTempFile::new().expect("temporary authority input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&authorized).expect("authority JSON"),
    )
    .expect("write authority input");
    assert!(!run_path(file.path(), false).status.success());

    let file = NamedTempFile::new().expect("temporary duplicate input");
    let duplicate = FIXTURE.replacen(
        "\"evaluation_mode\"",
        "\"evaluation_mode\":\"duplicate\",\"evaluation_mode\"",
        1,
    );
    std::fs::write(file.path(), duplicate).expect("write duplicate input");
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

    let mut uppercase_os: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    uppercase_os["observation"]["devices"][0]["device"]["os"] = Value::String("Linux".into());
    let file = NamedTempFile::new().expect("temporary uppercase OS input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&uppercase_os).expect("uppercase OS JSON"),
    )
    .expect("write uppercase OS input");
    assert!(!run_path(file.path(), false).status.success());

    let mut unicode_architecture: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unicode_architecture["observation"]["devices"][0]["device"]["architecture"] =
        Value::String("amd64☃".into());
    let file = NamedTempFile::new().expect("temporary Unicode architecture input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&unicode_architecture).expect("Unicode architecture JSON"),
    )
    .expect("write Unicode architecture input");
    assert!(!run_path(file.path(), false).status.success());

    let mut duplicate_device: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let first = duplicate_device["observation"]["devices"][0].clone();
    duplicate_device["observation"]["devices"]
        .as_array_mut()
        .expect("devices")
        .push(first);
    let file = NamedTempFile::new().expect("temporary duplicate device input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&duplicate_device).expect("duplicate device JSON"),
    )
    .expect("write duplicate device input");
    assert!(!run_path(file.path(), false).status.success());

    let mut exhausted_cpu: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    exhausted_cpu["observation"]["devices"][0]["device"]["available_cpu_cores"] = json!(0);
    exhausted_cpu["expected"][0]["exclusion_reasons"] = json!([
        "concurrency_capacity_insufficient",
        "cpu_cores_insufficient",
        "data_residency_zone_mismatch",
        "declared_lease_expired",
        "device_reserved",
        "sandbox_floor_unmet",
        "snapshot_stale",
        "trust_zone_unconfirmed"
    ]);
    let file = NamedTempFile::new().expect("temporary exhausted CPU input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&exhausted_cpu).expect("exhausted CPU JSON"),
    )
    .expect("write exhausted CPU input");
    assert!(
        run_path(file.path(), false).status.success(),
        "zero available CPU is a valid exhausted-resource observation"
    );

    let mut punctuation_runtime: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    punctuation_runtime["observation"]["devices"][0]["device"]["runtimes"] =
        json!(["oci/container"]);
    let file = NamedTempFile::new().expect("temporary punctuation runtime input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&punctuation_runtime).expect("punctuation runtime JSON"),
    )
    .expect("write punctuation runtime input");
    assert!(!run_path(file.path(), false).status.success());

    let mut overlong_runtime: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    overlong_runtime["observation"]["devices"][0]["device"]["runtimes"] = json!(["r".repeat(65)]);
    let file = NamedTempFile::new().expect("temporary overlong runtime input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&overlong_runtime).expect("overlong runtime JSON"),
    )
    .expect("write overlong runtime input");
    assert!(!run_path(file.path(), false).status.success());

    let mut short_lease: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    short_lease["observation"]["devices"][0]["device"]["lease_expires_at_ms"] = 100_999_u64.into();
    let file = NamedTempFile::new().expect("temporary short lease input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&short_lease).expect("short lease JSON"),
    )
    .expect("write short lease input");
    assert!(!run_path(file.path(), false).status.success());

    let mut long_lease: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    long_lease["observation"]["devices"][0]["device"]["lease_expires_at_ms"] = 700_001_u64.into();
    let file = NamedTempFile::new().expect("temporary long lease input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&long_lease).expect("long lease JSON"),
    )
    .expect("write long lease input");
    assert!(!run_path(file.path(), false).status.success());
}

#[test]
fn placement_evaluation_v2_supports_bounded_stdin_without_a_device_request() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "placement-evaluation-v2",
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn v2 placement evaluation");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(FIXTURE.as_bytes())
        .expect("write v2 fixture");
    let output = child
        .wait_with_output()
        .expect("wait for v2 placement evaluation");
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["decisions"].as_array().expect("decisions").len(), 2);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("/api/v1/devices"));
}

#[test]
fn placement_evaluation_v2_text_output_is_metadata_only() {
    let output = run_path(&fixture_path(), false);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("text output");
    assert!(text.contains("offline placement evaluation [offline_static_only]"));
    assert!(text.contains("reservation=reserved"));
    assert!(text.contains("gpus=2"));
    assert!(text.contains("selected=none"));
    assert!(text.contains("execution_authorized=false"));
    assert!(!text.contains("/api/v1/devices"));
}
