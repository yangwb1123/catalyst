use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use super::{
    CORE_KILL_REAP_TIMEOUT, CoreTerminalBridgeError, MAX_CORE_STDERR_BYTES, PinnedCoreExecutable,
    invalid,
};

pub(super) fn core_command(
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

pub(super) struct BoundedRead {
    pub(super) bytes: Vec<u8>,
    pub(super) overflow: bool,
}

pub(super) struct CoreIo {
    writer: Receiver<std::io::Result<BoundedRead>>,
    stdout: Receiver<std::io::Result<BoundedRead>>,
    stderr: Receiver<std::io::Result<BoundedRead>>,
}

impl CoreIo {
    pub(super) fn spawn(
        stdin: impl Write + Send + 'static,
        stdout: impl Read + Send + 'static,
        stderr: impl Read + Send + 'static,
        input: Vec<u8>,
        stdout_limit: usize,
    ) -> Self {
        Self {
            writer: spawn_io(move || write_input(stdin, &input)),
            stdout: spawn_io(move || read_bounded(stdout, stdout_limit)),
            stderr: spawn_io(move || read_bounded(stderr, MAX_CORE_STDERR_BYTES)),
        }
    }

    pub(super) fn collect(
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

pub(super) fn wait_bounded(
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

pub(super) fn terminate_process_tree(child: &mut std::process::Child) {
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

pub(super) fn process_io() -> CoreTerminalBridgeError {
    invalid("Core terminal process I/O failed")
}
