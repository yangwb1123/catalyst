#![allow(clippy::missing_errors_doc)]

use std::{
    fs::{File, Metadata},
    io::Read,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

#[path = "core_terminal_bridge/pinned_executable.rs"]
mod pinned_executable;
#[path = "core_terminal_bridge/process.rs"]
mod process;

use pinned_executable::PinnedCoreExecutable;
use process::{CoreIo, core_command, process_io, terminate_process_tree, wait_bounded};

use crate::runtime_domain::{
    GroupAgentNodeCoreTerminalReceiptEnvelope, GroupAgentNodeCoreTerminalReceiptPort,
    GroupAgentNodeCoreTerminalReceiptPortError, GroupAgentNodeTerminalControl,
    GroupAgentNodeTerminalReceipt, MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES,
    MAX_GROUP_AGENT_NODE_TERMINAL_RECEIPT_BYTES,
};
use crate::runtime_domain::{
    GroupAgentScheduledNodeCoreTerminalReceiptEnvelope, GroupAgentScheduledNodeTerminalControl,
    GroupAgentScheduledNodeTerminalReceipt, GroupAgentScheduledNodeTerminalReceiptPort,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTROL_BYTES, MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES,
};
use crate::runtime_domain::{
    MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES, MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES,
    ScheduledGraphProgressSnapshot, ScheduledGraphReconcileDecision, ScheduledGraphReconcilePort,
    ScheduledGraphReconcilePortError,
};

const MAX_CORE_BINARY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_CORE_STDOUT_BYTES: usize = MAX_GROUP_AGENT_NODE_TERMINAL_RECEIPT_BYTES;
const MAX_CORE_STDERR_BYTES: usize = 16 * 1024;
const CORE_PREFLIGHT_TIMEOUT: Duration = Duration::from_secs(5);
const CORE_DECISION_TIMEOUT: Duration = Duration::from_secs(15);
const CORE_IO_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const CORE_KILL_REAP_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
pub struct PinnedCoreTerminalBridge {
    path: PathBuf,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreTerminalBridgeError {
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct PinnedScheduledCoreTerminalBridge {
    inner: PinnedCoreTerminalBridge,
}

#[derive(Clone, Debug)]
pub struct PinnedScheduledGraphReconcileBridge {
    inner: PinnedCoreTerminalBridge,
}

impl PinnedCoreTerminalBridge {
    /// Validates and handshakes one explicitly pinned Core executable.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for an unsafe path, digest mismatch, failed
    /// protocol handshake, timeout, or bounded child-process I/O failure.
    pub fn new(path: PathBuf, sha256: String) -> Result<Self, CoreTerminalBridgeError> {
        validate_digest(&sha256)?;
        let bridge = Self { path, sha256 };
        let output = bridge.invoke(
            &["graph-node-terminal-receipt", "--protocol-version"],
            b"",
            CORE_PREFLIGHT_TIMEOUT,
        )?;
        if output.as_slice() != b"1" {
            return Err(invalid("Core terminal protocol handshake failed"));
        }
        Ok(bridge)
    }

    /// Runs the pinned, effect-free Core terminal decision over exact control bytes.
    ///
    /// # Errors
    ///
    /// Returns a redacted error if the executable changed or the bounded Core
    /// invocation does not produce one clean successful stdout artifact.
    pub fn decide_json(&self, control: &[u8]) -> Result<Vec<u8>, CoreTerminalBridgeError> {
        if !(1..=MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES).contains(&control.len()) {
            return Err(invalid("Core terminal control is outside its byte bound"));
        }
        self.invoke(
            &["graph-node-terminal-receipt", "--control", "-"],
            control,
            CORE_DECISION_TIMEOUT,
        )
    }

    fn prepare_binary(&self) -> Result<PinnedCoreExecutable, CoreTerminalBridgeError> {
        if !self.path.is_absolute() || self.path.as_os_str().is_empty() {
            return Err(invalid("Core executable path is invalid"));
        }
        let canonical = self
            .path
            .canonicalize()
            .map_err(|_| invalid("Core executable is unavailable"))?;
        if canonical != self.path {
            return Err(invalid("Core executable path is not canonical"));
        }
        let link = std::fs::symlink_metadata(&self.path)
            .map_err(|_| invalid("Core executable is unavailable"))?;
        if link.file_type().is_symlink() || !link.is_file() || link.len() > MAX_CORE_BINARY_BYTES {
            return Err(invalid("Core executable type or size is invalid"));
        }
        verify_executable_mode(&link)?;
        let file =
            File::open(&self.path).map_err(|_| invalid("Core executable cannot be opened"))?;
        let opened = file
            .metadata()
            .map_err(|_| invalid("Core executable metadata is unavailable"))?;
        if !same_file(&link, &opened) {
            return Err(invalid("Core executable changed during verification"));
        }
        PinnedCoreExecutable::prepare(file, opened.len(), &self.sha256)
    }

    fn invoke(
        &self,
        args: &[&str],
        input: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, CoreTerminalBridgeError> {
        let deadline = Instant::now() + timeout;
        let executable = self.prepare_binary_bounded(deadline)?;
        let mut child = core_command(&executable, &self.path, args)
            .spawn()
            .map_err(|_| invalid("Core terminal process could not start"))?;
        let stdin = child.stdin.take().ok_or_else(process_io)?;
        let stdout = child.stdout.take().ok_or_else(process_io)?;
        let stderr = child.stderr.take().ok_or_else(process_io)?;
        let io = CoreIo::spawn(stdin, stdout, stderr, input.to_vec());
        let status = wait_bounded(&mut child, deadline)?;
        let drain_deadline = deadline.min(Instant::now() + CORE_IO_DRAIN_TIMEOUT);
        let (wrote, stdout, stderr) = match io.collect(drain_deadline) {
            Ok(collected) => collected,
            Err(error) => {
                terminate_process_tree(&mut child);
                return Err(error);
            }
        };
        if !wrote.bytes.is_empty()
            || wrote.overflow
            || !status.success()
            || stdout.overflow
            || stderr.overflow
            || !stderr.bytes.is_empty()
            || stdout.bytes.is_empty()
        {
            return Err(invalid("Core terminal process failed"));
        }
        Ok(stdout.bytes)
    }

    fn prepare_binary_bounded(
        &self,
        deadline: Instant,
    ) -> Result<PinnedCoreExecutable, CoreTerminalBridgeError> {
        let bridge = self.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let _ = sender.send(bridge.prepare_binary());
        });
        let remaining = deadline.saturating_duration_since(Instant::now());
        receiver
            .recv_timeout(remaining)
            .map_err(|_| invalid("Core terminal process timed out"))?
    }
}

impl PinnedScheduledCoreTerminalBridge {
    /// Validates and handshakes the scheduled-specific Core receipt command.
    pub fn new(path: PathBuf, sha256: String) -> Result<Self, CoreTerminalBridgeError> {
        let bridge = PinnedCoreTerminalBridge { path, sha256 };
        let output = bridge.invoke(
            &[
                "graph-scheduled-node-terminal-receipt",
                "--protocol-version",
            ],
            b"",
            CORE_PREFLIGHT_TIMEOUT,
        )?;
        if output.as_slice() != b"1" {
            return Err(invalid("Core scheduled terminal protocol handshake failed"));
        }
        Ok(Self { inner: bridge })
    }

    pub fn decide_json(&self, control: &[u8]) -> Result<Vec<u8>, CoreTerminalBridgeError> {
        if !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_CONTROL_BYTES).contains(&control.len()) {
            return Err(invalid(
                "Core scheduled terminal control is outside its byte bound",
            ));
        }
        self.inner.invoke(
            &["graph-scheduled-node-terminal-receipt", "--control", "-"],
            control,
            CORE_DECISION_TIMEOUT,
        )
    }
}

impl PinnedScheduledGraphReconcileBridge {
    /// Records an explicitly pinned reconcile executable without starting it.
    pub fn new(path: PathBuf, sha256: String) -> Result<Self, CoreTerminalBridgeError> {
        validate_digest(&sha256)?;
        Ok(Self {
            inner: PinnedCoreTerminalBridge { path, sha256 },
        })
    }

    pub fn decide_json(&self, snapshot: &[u8]) -> Result<Vec<u8>, CoreTerminalBridgeError> {
        if !(1..=MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES).contains(&snapshot.len()) {
            return Err(invalid(
                "scheduled progress snapshot is outside its byte bound",
            ));
        }
        let protocol = self.inner.invoke(
            &["graph-scheduled-reconcile", "--protocol-version"],
            b"",
            CORE_PREFLIGHT_TIMEOUT,
        )?;
        if protocol.as_slice() != b"1" {
            return Err(invalid("Core scheduled reconcile handshake failed"));
        }
        let output = self.inner.invoke(
            &["graph-scheduled-reconcile", "--snapshot", "-"],
            snapshot,
            CORE_DECISION_TIMEOUT,
        )?;
        if output.len() > MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES {
            return Err(invalid(
                "scheduled reconcile decision exceeds its byte bound",
            ));
        }
        Ok(output)
    }
}

impl GroupAgentNodeCoreTerminalReceiptPort for PinnedCoreTerminalBridge {
    fn decide(
        &self,
        control: &GroupAgentNodeTerminalControl,
    ) -> Result<GroupAgentNodeCoreTerminalReceiptEnvelope, GroupAgentNodeCoreTerminalReceiptPortError>
    {
        control.validate().map_err(|_| core_port_error())?;
        let control_json = control.canonical_json().map_err(|_| core_port_error())?;
        let bytes = self
            .decide_json(control_json.as_bytes())
            .map_err(|_| core_port_error())?;
        let receipt =
            GroupAgentNodeTerminalReceipt::decode_exact(&bytes).map_err(|_| core_port_error())?;
        let receipt_json = String::from_utf8(bytes).map_err(|_| core_port_error())?;
        let envelope = GroupAgentNodeCoreTerminalReceiptEnvelope {
            receipt,
            receipt_json,
        };
        envelope
            .validate_against_control(control)
            .map_err(|_| core_port_error())?;
        Ok(envelope)
    }
}

fn scheduled_core_port_error()
-> crate::runtime_domain::GroupAgentScheduledNodeTerminalReceiptPortError {
    crate::runtime_domain::GroupAgentScheduledNodeTerminalReceiptPortError {
        message: "scheduled Core terminal receipt is invalid".into(),
    }
}

impl GroupAgentScheduledNodeTerminalReceiptPort for PinnedScheduledCoreTerminalBridge {
    fn decide(
        &self,
        control: &GroupAgentScheduledNodeTerminalControl,
    ) -> Result<
        GroupAgentScheduledNodeCoreTerminalReceiptEnvelope,
        crate::runtime_domain::GroupAgentScheduledNodeTerminalReceiptPortError,
    > {
        control
            .validate()
            .map_err(|_| scheduled_core_port_error())?;
        let control_json = control
            .canonical_json()
            .map_err(|_| scheduled_core_port_error())?;
        let bytes = self
            .decide_json(control_json.as_bytes())
            .map_err(|_| scheduled_core_port_error())?;
        if bytes.len() > MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES {
            return Err(scheduled_core_port_error());
        }
        let receipt = GroupAgentScheduledNodeTerminalReceipt::decode_exact(&bytes)
            .map_err(|_| scheduled_core_port_error())?;
        let receipt_json = String::from_utf8(bytes).map_err(|_| scheduled_core_port_error())?;
        receipt
            .validate_against_control(control)
            .map_err(|_| scheduled_core_port_error())?;
        Ok(GroupAgentScheduledNodeCoreTerminalReceiptEnvelope {
            receipt,
            receipt_json,
        })
    }
}

impl ScheduledGraphReconcilePort for PinnedScheduledGraphReconcileBridge {
    fn decide(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        snapshot
            .validate()
            .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)?;
        let snapshot_json = snapshot
            .canonical_json()
            .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)?;
        let output = self
            .decide_json(snapshot_json.as_bytes())
            .map_err(|_| ScheduledGraphReconcilePortError::Unavailable)?;
        let decision = ScheduledGraphReconcileDecision::decode_exact_bytes(&output)
            .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)?;
        decision
            .validate_against_snapshot(snapshot)
            .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)?;
        Ok(decision)
    }
}

fn hash_bounded(file: &mut File, size: u64) -> Result<String, CoreTerminalBridgeError> {
    if size == 0 || size > MAX_CORE_BINARY_BYTES {
        return Err(invalid("Core executable size is invalid"));
    }
    let mut hasher = Sha256::new();
    let copied = std::io::copy(&mut file.take(MAX_CORE_BINARY_BYTES + 1), &mut hasher)
        .map_err(|_| invalid("Core executable cannot be hashed"))?;
    if copied != size || copied > MAX_CORE_BINARY_BYTES {
        return Err(invalid("Core executable changed during hashing"));
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_digest(value: &str) -> Result<(), CoreTerminalBridgeError> {
    (value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(())
    .ok_or_else(|| invalid("Core executable digest is invalid"))
}

#[cfg(unix)]
fn verify_executable_mode(metadata: &Metadata) -> Result<(), CoreTerminalBridgeError> {
    use std::os::unix::fs::PermissionsExt;
    (metadata.permissions().mode() & 0o111 != 0)
        .then_some(())
        .ok_or_else(|| invalid("Core executable is not executable"))
}

#[cfg(not(unix))]
fn verify_executable_mode(_metadata: &Metadata) -> Result<(), CoreTerminalBridgeError> {
    Ok(())
}

#[cfg(unix)]
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(not(unix))]
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn invalid(message: &str) -> CoreTerminalBridgeError {
    CoreTerminalBridgeError {
        message: message.into(),
    }
}

fn core_port_error() -> GroupAgentNodeCoreTerminalReceiptPortError {
    GroupAgentNodeCoreTerminalReceiptPortError {
        message: "pinned Core terminal receipt decision failed".into(),
    }
}

impl std::fmt::Display for CoreTerminalBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CoreTerminalBridgeError {}
