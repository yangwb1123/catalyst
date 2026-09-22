use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const CPU_FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-placement-policy-parity-v1.json"
);
const GPU_FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-placement-gpu-policy-parity-v1.json"
);

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures")
        .join(name)
}

fn run_path(path: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "placement",
            "dry-run",
            "--input",
            path.to_str().expect("fixture path is UTF-8"),
        ])
        .output()
        .expect("run device placement dry-run");
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON output")
}

fn assert_fixture_result(value: &Value, fixture_text: &str) {
    let fixture: Value = serde_json::from_str(fixture_text).expect("fixture JSON");
    assert_eq!(value["type"], "device_placement_dry_run");
    assert_eq!(value["evaluation_mode"], "offline_placement_dry_run");
    assert_eq!(value["schema_version"], fixture["schema_version"]);
    assert_eq!(value["evaluated_at_ms"], fixture["evaluated_at_ms"]);
    let authority = &value["authority"];
    for field in [
        "identity_verified",
        "heartbeat_persisted",
        "inventory_authoritative",
        "reservation_created",
        "execution_authorized",
        "dispatch_performed",
    ] {
        assert_eq!(authority[field], false, "authority field {field}");
    }
    let decisions = value["decisions"].as_array().expect("decisions");
    let expected = fixture["expected"].as_array().expect("fixture expected");
    assert_eq!(decisions.len(), expected.len());
    for (decision, expected) in decisions.iter().zip(expected) {
        assert_eq!(decision["device_id"], expected["device_id"]);
        assert_eq!(
            decision["matches_requirements"],
            expected["matches_requirements"]
        );
        assert_eq!(decision["exclusion_reasons"], expected["exclusion_reasons"]);
    }
}

#[test]
fn cpu_placement_fixture_runs_through_the_bounded_cli() {
    let value = run_path(&fixture_path(
        "forge-device-placement-policy-parity-v1.json",
    ));
    assert_fixture_result(&value, CPU_FIXTURE);
}

#[test]
fn gpu_placement_fixture_runs_through_the_same_domain_projection() {
    let value = run_path(&fixture_path(
        "forge-device-placement-gpu-policy-parity-v1.json",
    ));
    assert_fixture_result(&value, GPU_FIXTURE);
}

#[test]
fn stdin_input_is_supported_without_creating_hub_state() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--json", "device", "placement", "dry-run", "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn device placement dry-run");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(CPU_FIXTURE.as_bytes())
        .expect("write fixture");
    let output = child.wait_with_output().expect("wait for CLI");
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_fixture_result(&value, CPU_FIXTURE);
}

#[test]
fn unknown_fields_and_oversized_input_fail_closed() {
    let mut unknown: Value = serde_json::from_str(CPU_FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let unknown_file = NamedTempFile::new().expect("temporary input");
    std::fs::write(
        unknown_file.path(),
        serde_json::to_vec(&unknown).expect("unknown field JSON"),
    )
    .expect("write unknown field input");
    let unknown_output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "device",
            "placement",
            "dry-run",
            "--input",
            unknown_file
                .path()
                .to_str()
                .expect("temporary path is UTF-8"),
        ])
        .output()
        .expect("run unknown field input");
    assert!(!unknown_output.status.success());

    let oversized_file = NamedTempFile::new().expect("temporary oversized input");
    std::fs::write(oversized_file.path(), vec![b'x'; 2 * 1024 * 1024 + 1])
        .expect("write oversized input");
    let oversized_output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "device",
            "placement",
            "dry-run",
            "--input",
            oversized_file
                .path()
                .to_str()
                .expect("temporary path is UTF-8"),
        ])
        .output()
        .expect("run oversized input");
    assert!(!oversized_output.status.success());
}
