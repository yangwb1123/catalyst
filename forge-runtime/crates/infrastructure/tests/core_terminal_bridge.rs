#![cfg(unix)]

mod core_terminal_bridge_support;

use std::{
    fs,
    os::unix::fs::symlink,
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
    thread,
    time::{Duration, Instant},
};

use forge_runtime_domain::{
    GroupAgentNodeCoreTerminalReceiptPort, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodeTerminalReceiptPort, MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES,
};
use forge_runtime_infrastructure::{PinnedCoreTerminalBridge, PinnedScheduledCoreTerminalBridge};
use tempfile::tempdir;

use core_terminal_bridge_support::{
    build_go_forge, core_script, environment_probe_script, scheduled_terminal_control,
    script_digest, shared_terminal_control, shared_terminal_fixture, write_script,
};

#[test]
fn accepts_an_exact_successful_protocol_handshake() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let path = write_script(
        directory.path(),
        "core",
        &core_script("printf '%s' '{}'"),
        true,
    );
    let digest = script_digest(&path);
    PinnedCoreTerminalBridge::new(path, digest).expect("exact protocol handshake");
}

#[test]
fn rejects_relative_and_noncanonical_paths() {
    let _guard = core_test_lock();
    let relative = PinnedCoreTerminalBridge::new(PathBuf::from("core"), "0".repeat(64));
    assert!(relative.is_err());

    let directory = tempdir().expect("temporary Core directory");
    let canonical = write_script(
        directory.path(),
        "core",
        &core_script("printf '%s' '{}'"),
        true,
    );
    let digest = script_digest(&canonical);
    fs::create_dir(directory.path().join("nested")).expect("noncanonical path parent");
    let noncanonical = directory.path().join("nested").join("..").join("core");
    let result = PinnedCoreTerminalBridge::new(noncanonical, digest);
    assert!(result.is_err());
}

#[test]
fn rejects_digest_mismatch_and_non_executable_file() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let executable = write_script(
        directory.path(),
        "executable",
        &core_script("printf '%s' '{}'"),
        true,
    );
    assert!(PinnedCoreTerminalBridge::new(executable, "0".repeat(64)).is_err());

    let inert = write_script(
        directory.path(),
        "inert",
        &core_script("printf '%s' '{}'"),
        false,
    );
    let digest = script_digest(&inert);
    assert!(PinnedCoreTerminalBridge::new(inert, digest).is_err());
}

#[test]
fn rejects_a_symlink_even_when_its_target_is_pinned() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let target = write_script(
        directory.path(),
        "target",
        &core_script("printf '%s' '{}'"),
        true,
    );
    let link = directory.path().join("link");
    symlink(&target, &link).expect("create Core symlink");
    let digest = script_digest(&target);
    assert!(PinnedCoreTerminalBridge::new(link, digest).is_err());
}

#[test]
fn rejects_stderr_and_nonzero_terminal_processes() {
    let _guard = core_test_lock();
    let cases = [
        ("stderr", "printf '%s' 'private' >&2; printf '%s' '{}'"),
        ("nonzero", "printf '%s' '{}'; exit 7"),
    ];
    for (name, decision) in cases {
        let directory = tempdir().expect("temporary Core directory");
        let path = write_script(directory.path(), name, &core_script(decision), true);
        let bridge = PinnedCoreTerminalBridge::new(path.clone(), script_digest(&path))
            .expect("valid test handshake");
        assert!(bridge.decide_json(b"{}").is_err(), "accepted {name}");
    }
}

#[test]
fn rejects_a_core_protocol_handshake_timeout() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let path = write_script(
        directory.path(),
        "slow-core",
        "#!/bin/sh\n/bin/sleep 30\n",
        true,
    );
    let error = PinnedCoreTerminalBridge::new(path.clone(), script_digest(&path))
        .expect_err("Core handshake timeout must fail closed");
    assert_eq!(error.message, "Core terminal process timed out");
}

#[cfg(target_os = "linux")]
#[test]
fn inherited_descendant_pipes_are_bounded_and_the_process_group_is_killed() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let pid_path = directory.path().join("descendant.pid");
    let decision = format!(
        "/bin/sleep 30 & descendant=$!; printf '%s' \"$descendant\" > '{}'; printf '%s' '{{}}'",
        pid_path.display()
    );
    let path = write_script(
        directory.path(),
        "forking-core",
        &core_script(&decision),
        true,
    );
    let bridge = PinnedCoreTerminalBridge::new(path.clone(), script_digest(&path))
        .expect("valid test handshake");

    let started = Instant::now();
    let error = bridge
        .decide_json(b"{}")
        .expect_err("inherited pipe must fail within the I/O bound");
    assert_eq!(error.message, "Core terminal process I/O failed");
    assert!(started.elapsed() < Duration::from_secs(3));

    let pid = fs::read_to_string(pid_path)
        .expect("descendant pid")
        .parse::<u32>()
        .expect("numeric descendant pid");
    assert_process_stopped(pid);
}

#[test]
fn rejects_control_and_receipt_bytes_outside_protocol_bounds() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let path = write_script(
        directory.path(),
        "core",
        &core_script("/bin/dd if=/dev/zero bs=65537 count=1 2>/dev/null"),
        true,
    );
    let bridge = PinnedCoreTerminalBridge::new(path.clone(), script_digest(&path))
        .expect("valid test handshake");

    assert!(bridge.decide_json(b"").is_err());
    let oversized_control = vec![0_u8; MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES + 1];
    assert!(bridge.decide_json(&oversized_control).is_err());
    assert!(bridge.decide_json(b"{}").is_err());
}

#[test]
fn rechecks_the_pinned_binary_before_every_decision() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let path = write_script(
        directory.path(),
        "core",
        &core_script("printf '%s' '{}'"),
        true,
    );
    let bridge = PinnedCoreTerminalBridge::new(path.clone(), script_digest(&path))
        .expect("valid test handshake");
    fs::write(&path, core_script("printf '%s' '{\"changed\":true}'"))
        .expect("replace pinned executable bytes");

    assert!(bridge.decide_json(b"{}").is_err());
}

#[test]
fn rejects_noncanonical_terminal_receipt_output() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let path = write_script(
        directory.path(),
        "core",
        &core_script("printf '%s\\n' '{}'"),
        true,
    );
    let bridge = PinnedCoreTerminalBridge::new(path.clone(), script_digest(&path))
        .expect("valid test handshake");
    assert!(bridge.decide(&shared_terminal_control()).is_err());
}

#[test]
fn clears_the_parent_environment_before_protocol_handshake() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Core directory");
    let path = write_script(directory.path(), "core", &environment_probe_script(), true);
    let status = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", "core_environment_worker", "--nocapture"])
        .env("FORGE_CORE_ENV_WORKER", "1")
        .env("FORGE_CORE_ENV_PATH", &path)
        .env("FORGE_CORE_ENV_SHA256", script_digest(&path))
        .status()
        .expect("start environment worker");
    assert!(status.success(), "environment worker rejected env_clear");
}

#[test]
fn core_environment_worker() {
    if std::env::var_os("FORGE_CORE_ENV_WORKER").is_none() {
        return;
    }
    let path = PathBuf::from(std::env::var_os("FORGE_CORE_ENV_PATH").expect("worker path"));
    let digest = std::env::var("FORGE_CORE_ENV_SHA256").expect("worker digest");
    PinnedCoreTerminalBridge::new(path, digest).expect("bridge clears inherited environment");
}

#[test]
fn compiled_go_core_returns_the_exact_shared_receipt() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Go build directory");
    let path = build_go_forge(directory.path());
    let bridge = PinnedCoreTerminalBridge::new(path.clone(), script_digest(&path))
        .expect("pinned compiled Go Core");
    let fixture = shared_terminal_fixture();
    let output = bridge
        .decide_json(fixture.canonical_terminal_control_json.as_bytes())
        .expect("Go terminal receipt decision");
    assert_eq!(output, fixture.canonical_terminal_receipt_json.as_bytes());

    let envelope = bridge
        .decide(&shared_terminal_control())
        .expect("validated Go terminal receipt");
    assert_eq!(
        envelope.receipt_json,
        fixture.canonical_terminal_receipt_json
    );
    assert_eq!(
        envelope.receipt.receipt_sha256,
        fixture.terminal_receipt_sha256
    );
}

#[test]
fn compiled_go_core_accepts_the_rust_scheduled_control_digest() {
    let _guard = core_test_lock();
    let directory = tempdir().expect("temporary Go build directory");
    let path = build_go_forge(directory.path());
    let bridge = PinnedScheduledCoreTerminalBridge::new(path.clone(), script_digest(&path))
        .expect("pinned compiled scheduled Go Core");
    let control = scheduled_terminal_control();

    let envelope = bridge
        .decide(&control)
        .expect("Go accepts Rust scheduled control digest");

    assert_eq!(
        envelope.receipt.terminal_control_sha256,
        control.snapshot_sha256
    );
    assert_eq!(
        envelope.receipt.artifact_sha256,
        control.artifact.artifact_sha256
    );
    assert_eq!(
        envelope.receipt.node_outcome,
        GroupAgentNodeTerminalOutcome::Completed
    );
    assert_eq!(
        envelope.receipt_json,
        envelope
            .receipt
            .canonical_json()
            .expect("canonical receipt")
    );
}

fn core_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

#[cfg(target_os = "linux")]
fn assert_process_stopped(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if !process_is_running(pid) {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("Core descendant {pid} survived process-group termination");
}

#[cfg(target_os = "linux")]
fn process_is_running(pid: u32) -> bool {
    let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    stat.rsplit_once(") ")
        .and_then(|(_, fields)| fields.chars().next())
        .is_some_and(|state| !matches!(state, 'Z' | 'X'))
}
