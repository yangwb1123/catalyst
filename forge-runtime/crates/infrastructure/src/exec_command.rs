use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

use crate::runtime_domain::{
    AgentTool, Cancellation, Capability, TOOL_EFFECT_UNCERTAIN_CODE, ToolContext, ToolError,
    ToolFuture, ToolOutput, ToolSpec,
};
use cap_std::{ambient_authority, fs::Dir};
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const MAX_PROGRAM_BYTES: usize = 4_096;
const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;
const MAX_CWD_BYTES: usize = 4_096;
const MAX_CAPTURE_BYTES: usize = 1024 * 1024;
const OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const SAFE_ENVIRONMENT_NAMES: &[&str] = &[
    "PATH",
    "HOME",
    "TMPDIR",
    "TEMP",
    "TMP",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "GOPATH",
    "GOMODCACHE",
    "GOCACHE",
    "PNPM_HOME",
    "LANG",
    "LC_ALL",
];
const SENSITIVE_ENVIRONMENT_MARKERS: &[&str] =
    &["KEY", "TOKEN", "SECRET", "PASSWORD", "CREDENTIAL", "AUTH"];

#[path = "exec_command_cleanup.rs"]
mod cleanup;
use cleanup::{cancelled_error, cleanup_after_capture_error, cleanup_after_error, wait_for_child};

#[derive(Clone)]
pub struct ExecCommandTool {
    #[cfg(not(unix))]
    workspace_root: PathBuf,
    #[cfg(unix)]
    workspace_dir: Arc<Dir>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExecCommandInput {
    program: String,
    argv: Vec<String>,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Debug)]
struct BoundedCapture {
    bytes: Vec<u8>,
    truncated: bool,
}

struct CaptureReaders {
    stdout: Receiver<io::Result<BoundedCapture>>,
    stderr: Receiver<io::Result<BoundedCapture>>,
}

impl ExecCommandTool {
    /// Bind command execution to one existing workspace directory.
    ///
    /// # Errors
    ///
    /// Returns `invalid_workspace` when the path cannot be canonicalized or is
    /// not a directory.
    pub fn new(workspace_root: impl AsRef<Path>) -> Result<Self, ToolError> {
        let root = fs::canonicalize(workspace_root.as_ref())
            .map_err(|error| ToolError::new("invalid_workspace", error.to_string()))?;
        let metadata = fs::metadata(&root)
            .map_err(|error| ToolError::new("invalid_workspace", error.to_string()))?;
        if !metadata.is_dir() {
            return Err(ToolError::new(
                "invalid_workspace",
                "workspace root is not a directory",
            ));
        }
        let workspace_dir = Arc::new(
            Dir::open_ambient_dir(&root, ambient_authority())
                .map_err(|error| ToolError::new("invalid_workspace", error.to_string()))?,
        );
        Ok(Self::from_anchored(workspace_dir, root))
    }

    #[cfg(unix)]
    pub(crate) fn from_anchored(workspace_dir: Arc<Dir>, _workspace_root: PathBuf) -> Self {
        Self { workspace_dir }
    }

    #[cfg(not(unix))]
    pub(crate) fn from_anchored(_workspace_dir: Arc<Dir>, workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl AgentTool for ExecCommandTool {
    fn spec(&self) -> ToolSpec {
        let schema = serde_json::to_value(schema_for!(ExecCommandInput))
            .expect("generated exec-command schema is serializable");
        ToolSpec {
            name: "exec_command".into(),
            description: "Execute one program directly with an argv array inside the workspace."
                .into(),
            input_schema: schema,
            capability: Capability::Process,
        }
    }

    fn execute(&self, arguments: Value, context: ToolContext) -> ToolFuture<'_> {
        let workspace = self.clone();
        Box::pin(async move {
            let input = parse_input(arguments)?;
            let output_limit = context.max_output_bytes.min(MAX_CAPTURE_BYTES);
            tokio::task::spawn_blocking(move || {
                run_command(&workspace, &input, &context.cancellation, output_limit)
            })
            .await
            .map_err(blocking_task_failure)?
        })
    }
}

fn blocking_task_failure(error: impl std::fmt::Display) -> ToolError {
    ToolError::new(
        TOOL_EFFECT_UNCERTAIN_CODE,
        format!("process worker ended abnormally after effect ownership transferred: {error}"),
    )
}

fn parse_input(arguments: Value) -> Result<ExecCommandInput, ToolError> {
    let input: ExecCommandInput = serde_json::from_value(arguments)
        .map_err(|error| ToolError::new("invalid_arguments", error.to_string()))?;
    validate_text(&input.program, MAX_PROGRAM_BYTES, "program")?;
    if input.argv.len() > MAX_ARGUMENTS {
        return Err(invalid_arguments("argv contains too many entries"));
    }
    let mut total = 0_usize;
    for argument in &input.argv {
        validate_argument(argument)?;
        total = total
            .checked_add(argument.len())
            .ok_or_else(|| invalid_arguments("argv byte count overflowed"))?;
    }
    if total > MAX_ARGUMENT_BYTES {
        return Err(invalid_arguments("argv exceeds its aggregate byte limit"));
    }
    validate_optional_fields(&input)?;
    Ok(input)
}

fn validate_optional_fields(input: &ExecCommandInput) -> Result<(), ToolError> {
    if let Some(cwd) = &input.cwd {
        validate_text(cwd, MAX_CWD_BYTES, "cwd")?;
    }
    if input
        .timeout_ms
        .is_some_and(|value| value == 0 || value > MAX_TIMEOUT_MS)
    {
        return Err(invalid_arguments("timeout_ms is outside 1..=300000"));
    }
    Ok(())
}

fn validate_text(value: &str, maximum: usize, label: &str) -> Result<(), ToolError> {
    if value.is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(invalid_arguments(format!(
            "{label} must be nonempty, NUL-free, and at most {maximum} bytes"
        )));
    }
    Ok(())
}

fn validate_argument(value: &str) -> Result<(), ToolError> {
    if value.len() > MAX_ARGUMENT_BYTES || value.contains('\0') {
        return Err(invalid_arguments(
            "argv entries must be NUL-free and within the byte limit",
        ));
    }
    Ok(())
}

fn invalid_arguments(message: impl Into<String>) -> ToolError {
    ToolError::new("invalid_arguments", message)
}

fn run_command(
    workspace: &ExecCommandTool,
    input: &ExecCommandInput,
    cancellation: &Cancellation,
    output_limit: usize,
) -> Result<ToolOutput, ToolError> {
    if cancellation.is_cancelled() {
        return Err(cancelled_error());
    }
    let cwd = resolve_cwd(workspace, input.cwd.as_deref())?;
    let timeout = Duration::from_millis(input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));
    let mut command = command_for(input, &cwd);
    let mut child = command
        .spawn()
        .map_err(|error| ToolError::new("spawn_failed", error.to_string()))?;
    let readers = match CaptureReaders::from_child(&mut child, output_limit) {
        Ok(readers) => readers,
        Err(error) => {
            return Err(cleanup_after_error(&mut child, error));
        }
    };
    let status = match wait_for_child(&mut child, cancellation, Instant::now() + timeout) {
        Ok(status) => status,
        Err(error) if error.code == forge_runtime_domain::TOOL_EFFECT_UNCERTAIN_CODE => {
            return Err(error);
        }
        Err(error) => {
            return match readers.collect(Instant::now() + OUTPUT_DRAIN_TIMEOUT) {
                Ok(_) => Err(error),
                Err(capture_error) => Err(cleanup_after_capture_error(&mut child, capture_error)),
            };
        }
    };
    let (stdout, stderr) = match readers.collect(Instant::now() + OUTPUT_DRAIN_TIMEOUT) {
        Ok(output) => output,
        Err(error) => {
            return Err(cleanup_after_capture_error(&mut child, error));
        }
    };
    Ok(render_output(status, &stdout, &stderr, output_limit))
}

#[cfg(unix)]
fn resolve_cwd(workspace: &ExecCommandTool, cwd: Option<&str>) -> Result<Dir, ToolError> {
    let Some(cwd) = cwd else {
        return workspace
            .workspace_dir
            .try_clone()
            .map_err(|error| ToolError::new("cwd_unavailable", error.to_string()));
    };
    let relative = validate_cwd(cwd)?;
    workspace
        .workspace_dir
        .open_dir(relative)
        .map_err(|error| ToolError::new("cwd_outside_workspace", error.to_string()))
}

#[cfg(not(unix))]
fn resolve_cwd(workspace: &ExecCommandTool, cwd: Option<&str>) -> Result<PathBuf, ToolError> {
    let Some(cwd) = cwd else {
        return Ok(workspace.workspace_root.clone());
    };
    let relative = validate_cwd(cwd)?;
    let resolved = fs::canonicalize(workspace.workspace_root.join(relative))
        .map_err(|error| ToolError::new("cwd_unavailable", error.to_string()))?;
    if !resolved.starts_with(&workspace.workspace_root) || !resolved.is_dir() {
        return Err(ToolError::new(
            "cwd_outside_workspace",
            "cwd does not resolve to a directory inside the workspace",
        ));
    }
    Ok(resolved)
}

fn validate_cwd(cwd: &str) -> Result<&Path, ToolError> {
    let relative = Path::new(cwd);
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(ToolError::new(
            "invalid_cwd",
            "cwd must be a relative path without parent traversal",
        ));
    }
    Ok(relative)
}

#[cfg(unix)]
fn command_for(input: &ExecCommandInput, cwd: &Dir) -> Command {
    let mut command = Command::new(&input.program);
    command
        .args(&input.argv)
        .current_dir(descriptor_cwd_path(cwd))
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_safe_environment(&mut command);
    configure_process_group(&mut command);
    command
}

#[cfg(not(unix))]
fn command_for(input: &ExecCommandInput, cwd: &Path) -> Command {
    let mut command = Command::new(&input.program);
    command
        .args(&input.argv)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_safe_environment(&mut command);
    command
}

#[cfg(unix)]
fn descriptor_cwd_path(directory: &Dir) -> PathBuf {
    use std::os::fd::AsRawFd as _;

    #[cfg(target_os = "linux")]
    let base = "/proc/self/fd";
    #[cfg(not(target_os = "linux"))]
    let base = "/dev/fd";
    Path::new(base).join(directory.as_raw_fd().to_string())
}

fn configure_safe_environment(command: &mut Command) {
    for (name, value) in std::env::vars_os() {
        let Some(name_text) = name.to_str() else {
            continue;
        };
        if is_allowed_environment_name(name_text) {
            command.env(name, value);
        }
    }
}

fn is_allowed_environment_name(name: &str) -> bool {
    SAFE_ENVIRONMENT_NAMES
        .iter()
        .any(|allowed| name.eq_ignore_ascii_case(allowed))
        && !is_sensitive_environment_name(name)
}

fn is_sensitive_environment_name(name: &str) -> bool {
    let uppercase = name.to_ascii_uppercase();
    SENSITIVE_ENVIRONMENT_MARKERS
        .iter()
        .any(|marker| uppercase.contains(marker))
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

impl CaptureReaders {
    fn from_child(child: &mut Child, limit: usize) -> Result<Self, ToolError> {
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

    fn collect(self, deadline: Instant) -> Result<(BoundedCapture, BoundedCapture), ToolError> {
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

fn render_output(
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

#[cfg(test)]
#[path = "exec_command_tests.rs"]
mod tests;
