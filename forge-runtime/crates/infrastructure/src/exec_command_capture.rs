use std::{
    io::{self, Read},
    process::{Child, ExitStatus},
    sync::mpsc::{self, Receiver},
    thread,
    time::Instant,
};

use super::{BoundedCapture, ToolError, ToolOutput};

pub(super) struct CaptureReaders {
    stdout: Receiver<io::Result<BoundedCapture>>,
    stderr: Receiver<io::Result<BoundedCapture>>,
}

impl CaptureReaders {
    pub(super) fn from_child(child: &mut Child, limit: usize) -> Result<Self, ToolError> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ToolError::new("capture_failed", "child stdout pipe was unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ToolError::new("capture_failed", "child stderr pipe was unavailable"))?;
        Ok(Self {
            stdout: spawn_reader("forge-command-stdout", stdout, limit)?,
            stderr: spawn_reader("forge-command-stderr", stderr, limit)?,
        })
    }

    pub(super) fn collect(
        self,
        deadline: Instant,
    ) -> Result<(BoundedCapture, BoundedCapture), ToolError> {
        let stdout = receive_capture(&self.stdout, deadline)?;
        let stderr = receive_capture(&self.stderr, deadline)?;
        Ok((stdout, stderr))
    }
}

fn spawn_reader(
    name: &str,
    reader: impl Read + Send + 'static,
    limit: usize,
) -> Result<Receiver<io::Result<BoundedCapture>>, ToolError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let _ = sender.send(read_bounded(reader, limit));
        })
        .map_err(|error| ToolError::new("capture_failed", error.to_string()))?;
    Ok(receiver)
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<BoundedCapture> {
    let mut bytes = Vec::with_capacity(limit.min(8 * 1024));
    let mut truncated = false;
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let retained = count.min(limit.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&chunk[..retained]);
        truncated |= retained < count;
    }
    Ok(BoundedCapture { bytes, truncated })
}

fn receive_capture(
    receiver: &Receiver<io::Result<BoundedCapture>>,
    deadline: Instant,
) -> Result<BoundedCapture, ToolError> {
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| ToolError::new("capture_failed", "command output did not close in time"))?
        .map_err(|error| ToolError::new("capture_failed", error.to_string()))
}

pub(super) fn render_output(
    status: ExitStatus,
    stdout: &BoundedCapture,
    stderr: &BoundedCapture,
    limit: usize,
) -> ToolOutput {
    let exit_code = status
        .code()
        .map_or_else(|| "signal".into(), |code| code.to_string());
    let mut content = format!(
        "exit_code: {exit_code}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&stdout.bytes),
        String::from_utf8_lossy(&stderr.bytes)
    );
    let truncated = stdout.truncated || stderr.truncated || content.len() > limit;
    truncate_utf8(&mut content, limit);
    ToolOutput { content, truncated }
}

fn truncate_utf8(value: &mut String, maximum: usize) {
    if value.len() <= maximum {
        return;
    }
    let mut boundary = maximum;
    while !value.is_char_boundary(boundary) {
        boundary = boundary.saturating_sub(1);
    }
    value.truncate(boundary);
}
