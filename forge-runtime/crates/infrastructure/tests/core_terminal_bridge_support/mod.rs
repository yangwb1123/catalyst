use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use forge_runtime_domain::GroupAgentNodeTerminalControl;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
pub(crate) struct SharedTerminalFixture {
    pub(crate) canonical_terminal_control_json: String,
    pub(crate) canonical_terminal_receipt_json: String,
    pub(crate) terminal_receipt_sha256: String,
}

pub(crate) fn shared_terminal_fixture() -> SharedTerminalFixture {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-terminal-lifecycle-v1.json"
    )))
    .expect("shared terminal fixture")
}

pub(crate) fn shared_terminal_control() -> GroupAgentNodeTerminalControl {
    let fixture = shared_terminal_fixture();
    GroupAgentNodeTerminalControl::decode_exact(fixture.canonical_terminal_control_json.as_bytes())
        .expect("strict shared terminal control")
}

pub(crate) fn write_script(directory: &Path, name: &str, body: &str, executable: bool) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, body).expect("write Core test script");
    set_executable(&path, executable);
    path.canonicalize().expect("canonical Core test script")
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set script mode");
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) {}

pub(crate) fn script_digest(path: &Path) -> String {
    let bytes = fs::read(path).expect("read Core test executable");
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn core_script(decision: &str) -> String {
    format!(
        "#!/bin/sh\n\
         if [ \"$1\" != \"graph-node-terminal-receipt\" ]; then exit 90; fi\n\
         if [ \"$2\" = \"--protocol-version\" ]; then printf '%s' '1'; exit 0; fi\n\
         if [ \"$2\" != \"--control\" ] || [ \"$3\" != \"-\" ]; then exit 91; fi\n\
         {decision}\n"
    )
}

pub(crate) fn environment_probe_script() -> String {
    core_script("if [ \"${FORGE_CORE_ENV_WORKER+x}\" = x ]; then exit 92; fi; printf '%s' '{}'")
}

pub(crate) fn build_go_forge(directory: &Path) -> PathBuf {
    let output = directory.join("forge");
    let status = Command::new("go")
        .args(["build", "-trimpath", "-o"])
        .arg(&output)
        .arg("./cmd/forge")
        .current_dir(repository_root().join("forge-core"))
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env("GOTOOLCHAIN", "local")
        .status()
        .expect("start deterministic Go build");
    assert!(status.success(), "deterministic Go Core build failed");
    output.canonicalize().expect("canonical Go Core binary")
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root")
}
