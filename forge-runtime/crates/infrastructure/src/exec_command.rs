use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::runtime_domain::{
    AgentTool, Cancellation, Capability, TOOL_EFFECT_UNCERTAIN_CODE, ToolContext, ToolError,
    ToolExecutionFuture, ToolExecutionResult, ToolFuture, ToolOutput, ToolSpec,
    execution::fabric::{
        EXECUTION_FABRIC_ABI_VERSION, EffectClassification, ExecutionAttempt, ExecutionEvidence,
        ExecutionEvidenceSource, ExecutionTarget, ExecutionTargetScope, ExecutionTargetStatus,
        LOCAL_EXECUTION_ADAPTER_ID, LOCAL_EXECUTION_ADAPTER_VERSION, LocalProcessObservation,
        Mobility, ToolInvocationRef,
    },
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

#[path = "exec_command_cleanup.rs"]
mod cleanup;
use cleanup::{cancelled_error, cleanup_after_capture_error, cleanup_after_error, wait_for_child};
#[path = "exec_command_environment.rs"]
mod environment;
use environment::{SafeEnvironment, is_valid_environment_digest};
#[path = "exec_command_capture.rs"]
mod capture;
use capture::{CaptureReaders, render_output};

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
        let execution = self.execute_inner(arguments, context, None);
        Box::pin(async move { execution.await.map(|result| result.output) })
    }

    fn execute_with_invocation(
        &self,
        arguments: Value,
        context: ToolContext,
        invocation: ToolInvocationRef,
    ) -> ToolFuture<'_> {
        let execution = self.execute_inner(arguments, context, Some(invocation));
        Box::pin(async move { execution.await.map(|result| result.output) })
    }

    fn execute_with_invocation_evidence(
        &self,
        arguments: Value,
        context: ToolContext,
        invocation: ToolInvocationRef,
    ) -> ToolExecutionFuture<'_> {
        self.execute_inner(arguments, context, Some(invocation))
    }
}

impl ExecCommandTool {
    fn execute_inner(
        &self,
        arguments: Value,
        context: ToolContext,
        invocation: Option<ToolInvocationRef>,
    ) -> ToolExecutionFuture<'_> {
        let workspace = self.clone();
        Box::pin(async move {
            let input = parse_input(arguments)?;
            let output_limit = context.max_output_bytes.min(MAX_CAPTURE_BYTES);
            tokio::task::spawn_blocking(move || {
                LocalExecutionTarget::new(workspace).execute(
                    &input,
                    invocation,
                    &context.cancellation,
                    output_limit,
                )
            })
            .await
            .map_err(blocking_task_failure)?
            .map(|(output, execution_evidence)| ToolExecutionResult {
                output,
                execution_evidence,
            })
        })
    }
}

struct LocalExecutionTarget {
    workspace: ExecCommandTool,
    descriptor: ExecutionTarget,
}

impl LocalExecutionTarget {
    fn new(workspace: ExecCommandTool) -> Self {
        Self {
            workspace,
            descriptor: ExecutionTarget::local(),
        }
    }

    fn execute(
        &self,
        input: &ExecCommandInput,
        invocation: Option<ToolInvocationRef>,
        cancellation: &Cancellation,
        output_limit: usize,
    ) -> Result<(ToolOutput, Option<ExecutionEvidence>), ToolError> {
        let environment = SafeEnvironment::capture();
        let attempt = invocation.map(|invocation| {
            ExecutionAttempt::local_process_with_environment(invocation, environment.digest())
        });
        if let Some(attempt) = &attempt {
            self.validate_attempt(attempt)?;
        }
        let (output, exit_code) = run_command(
            &self.workspace,
            input,
            &environment,
            cancellation,
            output_limit,
        )?;
        let evidence = attempt.map(|attempt| ExecutionEvidence {
            v: EXECUTION_FABRIC_ABI_VERSION,
            attempt_ref: attempt.attempt_ref,
            target_ref: attempt.target_ref,
            source: ExecutionEvidenceSource::LocalProcessObservation,
            observation: LocalProcessObservation {
                exit_code,
                rendered_output_bytes: u64::try_from(output.content.len()).unwrap_or(u64::MAX),
                output_truncated: output.truncated,
            },
        });
        Ok((output, evidence))
    }

    fn validate_attempt(&self, attempt: &ExecutionAttempt) -> Result<(), ToolError> {
        let target_ref = &self.descriptor.target_ref;
        if self.descriptor.v != EXECUTION_FABRIC_ABI_VERSION
            || self.descriptor.target_ref.scope != ExecutionTargetScope::CurrentRuntimeOnly
            || self.descriptor.adapter_id != LOCAL_EXECUTION_ADAPTER_ID
            || self.descriptor.adapter_version != LOCAL_EXECUTION_ADAPTER_VERSION
            || self.descriptor.status != ExecutionTargetStatus::Ready
            || attempt.target_ref != *target_ref
            || attempt.placement_constraint.target_required != *target_ref
        {
            return Err(ToolError::new(
                "unsupported_execution_target",
                "the local adapter accepts only the current local target",
            ));
        }
        if attempt.v != EXECUTION_FABRIC_ABI_VERSION
            || attempt.attempt_ref.session_id != attempt.tool_invocation.session_id
            || attempt.attempt_ref.run_id != attempt.tool_invocation.run_id
            || attempt.attempt_ref.tool_started_sequence
                != attempt.tool_invocation.tool_started_sequence
            || attempt.effect.capability != Capability::Process
            || attempt.effect.classification != EffectClassification::PotentiallySideEffecting
            || attempt.mobility != Mobility::Pinned
            || !attempt.input_artifacts.is_empty()
            || !attempt.output_artifacts.is_empty()
            || !is_valid_environment_digest(&attempt.environment_digest)
        {
            return Err(ToolError::new(
                "invalid_execution_attempt",
                "attempt metadata is incompatible with the local process adapter",
            ));
        }
        Ok(())
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
    environment: &SafeEnvironment,
    cancellation: &Cancellation,
    output_limit: usize,
) -> Result<(ToolOutput, Option<i32>), ToolError> {
    if cancellation.is_cancelled() {
        return Err(cancelled_error());
    }
    let cwd = resolve_cwd(workspace, input.cwd.as_deref())?;
    let timeout = Duration::from_millis(input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));
    let mut command = command_for(input, &cwd, environment);
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
    let exit_code = status.code();
    Ok((
        render_output(status, &stdout, &stderr, output_limit),
        exit_code,
    ))
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
fn command_for(input: &ExecCommandInput, cwd: &Dir, environment: &SafeEnvironment) -> Command {
    let mut command = Command::new(&input.program);
    command
        .args(&input.argv)
        .current_dir(descriptor_cwd_path(cwd))
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    environment.apply(&mut command);
    configure_process_group(&mut command);
    command
}

#[cfg(not(unix))]
fn command_for(input: &ExecCommandInput, cwd: &Path, environment: &SafeEnvironment) -> Command {
    let mut command = Command::new(&input.program);
    command
        .args(&input.argv)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    environment.apply(&mut command);
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

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(test)]
#[path = "exec_command_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "exec_command_basic_tests.rs"]
mod basic_tests;

#[cfg(test)]
#[path = "exec_command_fabric_tests.rs"]
mod fabric_tests;
