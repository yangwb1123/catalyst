use std::{
    fs::{File, Metadata},
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

#[path = "core_terminal_bridge/pinned_executable.rs"]
mod pinned_executable;

use pinned_executable::PinnedCoreExecutable;

use crate::runtime_domain::{
    GroupAgentNodeCoreTerminalReceiptEnvelope, GroupAgentNodeCoreTerminalReceiptPort,
    GroupAgentNodeCoreTerminalReceiptPortError, GroupAgentNodeTerminalControl,
    GroupAgentNodeTerminalReceipt, MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES,
    MAX_GROUP_AGENT_NODE_TERMINAL_RECEIPT_BYTES,
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

fn core_command(
    executable: &PinnedCoreExecutable,
    display_path: &PathBuf,
    args: &[&str],
) -> Command {
    let mut command = Command::new(executable.command_path());
    configure_argv_zero(&mut command, display_path);
    command
        .args(args)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_tree(&mut command);
    command
}

#[cfg(unix)]
fn configure_argv_zero(command: &mut Command, display_path: &PathBuf) {
    use std::os::unix::process::CommandExt;
    command.arg0(display_path);
}

#[cfg(not(unix))]
fn configure_argv_zero(_command: &mut Command, _display_path: &PathBuf) {}

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

struct BoundedRead {
    bytes: Vec<u8>,
    overflow: bool,
}

struct CoreIo {
    writer: Receiver<std::io::Result<BoundedRead>>,
    stdout: Receiver<std::io::Result<BoundedRead>>,
    stderr: Receiver<std::io::Result<BoundedRead>>,
}

impl CoreIo {
    fn spawn(
        stdin: impl Write + Send + 'static,
        stdout: impl Read + Send + 'static,
        stderr: impl Read + Send + 'static,
        input: Vec<u8>,
    ) -> Self {
        Self {
            writer: spawn_io(move || write_input(stdin, &input)),
            stdout: spawn_io(move || read_bounded(stdout, MAX_CORE_STDOUT_BYTES)),
            stderr: spawn_io(move || read_bounded(stderr, MAX_CORE_STDERR_BYTES)),
        }
    }

    fn collect(
        self,
        deadline: Instant,
    ) -> Result<(BoundedRead, BoundedRead, BoundedRead), CoreTerminalBridgeError> {
        Ok((
            receive_io(&self.writer, deadline)?,
            receive_io(&self.stdout, deadline)?,
            receive_io(&self.stderr, deadline)?,
        ))
    }
}

fn write_input(mut stdin: impl Write, input: &[u8]) -> std::io::Result<BoundedRead> {
    stdin.write_all(input)?;
    Ok(BoundedRead {
        bytes: Vec::new(),
        overflow: false,
    })
}

fn read_bounded(mut reader: impl Read, limit: usize) -> std::io::Result<BoundedRead> {
    let mut bytes = Vec::with_capacity(limit.min(8192));
    let mut overflow = false;
    let mut chunk = [0_u8; 8192];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        bytes.extend_from_slice(&chunk[..read.min(remaining)]);
        overflow |= read > remaining;
    }
    Ok(BoundedRead { bytes, overflow })
}

fn wait_bounded(
    child: &mut std::process::Child,
    deadline: Instant,
) -> Result<std::process::ExitStatus, CoreTerminalBridgeError> {
    loop {
        if Instant::now() >= deadline {
            terminate_process_tree(child);
            return Err(invalid("Core terminal process timed out"));
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|_| invalid("Core terminal process wait failed"))?
        {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn spawn_io(
    task: impl FnOnce() -> std::io::Result<BoundedRead> + Send + 'static,
) -> Receiver<std::io::Result<BoundedRead>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(task());
    });
    receiver
}

fn receive_io(
    receiver: &Receiver<std::io::Result<BoundedRead>>,
    deadline: Instant,
) -> Result<BoundedRead, CoreTerminalBridgeError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    receiver
        .recv_timeout(remaining)
        .map_err(|_| process_io())?
        .map_err(|_| process_io())
}

#[cfg(unix)]
fn configure_process_tree(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_tree(_command: &mut Command) {}

fn terminate_process_tree(child: &mut std::process::Child) {
    terminate_process_group(child);
    let _ = child.kill();
    let deadline = Instant::now() + CORE_KILL_REAP_TIMEOUT;
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
fn terminate_process_group(child: &std::process::Child) {
    let group = rustix::process::Pid::from_child(child);
    let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
}

#[cfg(not(unix))]
fn terminate_process_group(_child: &std::process::Child) {}

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

fn process_io() -> CoreTerminalBridgeError {
    invalid("Core terminal process I/O failed")
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
