use std::{fs, path::Path, time::Duration};

use crate::runtime_domain::{
    AgentTool, Cancellation, Capability, TOOL_EFFECT_UNCERTAIN_CODE, ToolContext, ToolError,
    ToolOutput, WorkspaceReadFactory as _,
};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::{
    ExecCommandTool, MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, MAX_TIMEOUT_MS, blocking_task_failure,
    is_allowed_environment_name, is_sensitive_environment_name, parse_input,
};
use crate::CapStdWorkspaceFactory;

fn context(root: &Path, max_output_bytes: usize) -> ToolContext {
    ToolContext {
        workspace: CapStdWorkspaceFactory
            .open(root)
            .expect("workspace capability"),
        cancellation: Cancellation::default(),
        max_output_bytes,
    }
}

async fn run(
    root: &Path,
    arguments: Value,
    max_output_bytes: usize,
) -> Result<ToolOutput, ToolError> {
    ExecCommandTool::new(root)
        .expect("tool")
        .execute(arguments, context(root, max_output_bytes))
        .await
}

#[test]
fn advertises_the_process_capability_and_direct_argv_shape() {
    let root = TempDir::new().expect("temporary workspace");
    let spec = ExecCommandTool::new(root.path()).expect("tool").spec();
    assert_eq!(spec.name, "exec_command");
    assert_eq!(spec.capability, Capability::Process);
    assert!(spec.input_schema.to_string().contains("argv"));
}

#[test]
fn blocking_worker_failure_is_effect_uncertain() {
    let error = blocking_task_failure("join failed");
    assert_eq!(error.code, TOOL_EFFECT_UNCERTAIN_CODE);
}

#[test]
fn environment_allowlist_rejects_sensitive_names() {
    assert!(is_allowed_environment_name("PATH"));
    assert!(is_allowed_environment_name("cargo_home"));
    for name in [
        "OPENAI_API_KEY",
        "ACCESS_TOKEN",
        "CLIENT_SECRET",
        "DB_PASSWORD",
        "AWS_CREDENTIAL_FILE",
        "HTTP_AUTHORIZATION",
    ] {
        assert!(is_sensitive_environment_name(name));
        assert!(!is_allowed_environment_name(name));
    }
}

#[test]
fn argv_and_timeout_boundaries_fail_closed() {
    let maximum_argv = vec![""; MAX_ARGUMENTS];
    parse_input(json!({
        "program": "true",
        "argv": maximum_argv,
        "timeout_ms": MAX_TIMEOUT_MS
    }))
    .expect("inclusive argument-count and timeout maxima");
    parse_input(json!({
        "program": "true",
        "argv": ["x".repeat(MAX_ARGUMENT_BYTES)],
        "timeout_ms": 1
    }))
    .expect("inclusive aggregate-byte and timeout minima");

    assert_invalid(
        json!({"program": "true", "argv": vec![""; MAX_ARGUMENTS + 1]}),
        "too many",
    );
    assert_invalid(
        json!({"program": "true", "argv": ["x".repeat(MAX_ARGUMENT_BYTES), "y"]}),
        "aggregate",
    );
    assert_invalid(json!({"program": "true", "argv": ["a\0b"]}), "NUL-free");
    assert_invalid(
        json!({"program": "true", "argv": [], "timeout_ms": 0}),
        "1..=300000",
    );
    assert_invalid(
        json!({"program": "true", "argv": [], "timeout_ms": MAX_TIMEOUT_MS + 1}),
        "1..=300000",
    );
}

fn assert_invalid(arguments: Value, expected: &str) {
    let error = parse_input(arguments).expect_err("invalid process arguments");
    assert_eq!(error.code, "invalid_arguments");
    assert!(error.message.contains(expected), "{}", error.message);
}

#[cfg(unix)]
#[tokio::test]
async fn captures_exit_code_stdout_and_stderr() {
    let root = TempDir::new().expect("temporary workspace");
    let output = run(
        root.path(),
        json!({
            "program": "sh",
            "argv": ["-c", "printf out; printf err >&2; exit 7"]
        }),
        1024,
    )
    .await
    .expect("command completes");
    assert!(output.content.contains("exit_code: 7"));
    assert!(output.content.contains("stdout:\nout"));
    assert!(output.content.contains("stderr:\nerr"));
}

#[cfg(unix)]
#[tokio::test]
async fn passes_arguments_without_implicit_shell_expansion() {
    let root = TempDir::new().expect("temporary workspace");
    let output = run(
        root.path(),
        json!({ "program": "printf", "argv": ["%s", "$(printf injected)"] }),
        1024,
    )
    .await
    .expect("printf completes");
    assert!(output.content.contains("$(printf injected)"));
}

#[cfg(unix)]
#[tokio::test]
async fn confines_cwd_and_copies_only_safe_environment() {
    let root = TempDir::new().expect("temporary workspace");
    fs::create_dir(root.path().join("nested")).expect("nested directory");
    let cwd = run(
        root.path(),
        json!({ "program": "pwd", "argv": [], "cwd": "nested" }),
        1024,
    )
    .await
    .expect("pwd completes");
    let environment = run(
        root.path(),
        json!({ "program": "env", "argv": [] }),
        64 * 1024,
    )
    .await
    .expect("env completes");
    assert!(
        cwd.content
            .contains(&root.path().join("nested").display().to_string())
    );
    assert!(environment.content.contains("PATH="));
    assert!(environment.content.contains("HOME="));
    assert!(!environment.content.contains("OPENAI_API_KEY="));
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_a_cwd_symlink_that_escapes_the_workspace() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("temporary workspace");
    let outside = TempDir::new().expect("outside directory");
    symlink(outside.path(), root.path().join("escape")).expect("symlink fixture");
    let error = run(
        root.path(),
        json!({ "program": "pwd", "argv": [], "cwd": "escape" }),
        1024,
    )
    .await
    .expect_err("escaping cwd is rejected");
    assert_eq!(error.code, "cwd_outside_workspace");
}

#[cfg(unix)]
#[tokio::test]
async fn default_cwd_remains_anchored_when_workspace_path_is_replaced() {
    let base = TempDir::new().expect("temporary base");
    let workspace = base.path().join("workspace");
    let original = base.path().join("original");
    fs::create_dir(&workspace).expect("workspace fixture");
    let tool = ExecCommandTool::new(&workspace).expect("anchored tool");
    fs::rename(&workspace, &original).expect("move original workspace");
    fs::create_dir(&workspace).expect("replacement workspace");

    tool.execute(
        json!({ "program": "sh", "argv": ["-c", "printf anchored > marker"] }),
        context(&workspace, 1024),
    )
    .await
    .expect("command runs in anchored directory");

    assert_eq!(
        fs::read_to_string(original.join("marker")).unwrap(),
        "anchored"
    );
    assert!(!workspace.join("marker").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn bounds_combined_rendered_output() {
    let root = TempDir::new().expect("temporary workspace");
    let output = run(
        root.path(),
        json!({
            "program": "sh",
            "argv": ["-c", "printf 123456789; printf abcdefghi >&2"]
        }),
        24,
    )
    .await
    .expect("command completes");
    assert!(output.truncated);
    assert!(output.content.len() <= 24);
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_terminates_the_process_group() {
    let root = TempDir::new().expect("temporary workspace");
    let error = run(
        root.path(),
        json!({
            "program": "sh",
            "argv": ["-c", "sleep 30"],
            "timeout_ms": 200
        }),
        1024,
    )
    .await
    .expect_err("command times out");
    assert_eq!(error.code, "tool_effect_uncertain");
}

#[cfg(unix)]
#[tokio::test]
async fn inherited_output_pipe_timeout_avoids_a_post_reap_group_kill() {
    let root = TempDir::new().expect("temporary workspace");
    let error = run(
        root.path(),
        json!({
            "program": "sh",
            "argv": ["-c", "sleep 30 & echo $! > background.pid"]
        }),
        1024,
    )
    .await
    .expect_err("an inherited output pipe cannot outlive the capture deadline");
    let raw_pid: i32 = fs::read_to_string(root.path().join("background.pid"))
        .expect("background process records its pid")
        .trim()
        .parse()
        .expect("recorded pid is numeric");
    let pid = rustix::process::Pid::from_raw(raw_pid).expect("recorded pid is positive");
    let descendant_was_alive = rustix::process::test_kill_process(pid).is_ok();
    let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);

    assert_eq!(error.code, "tool_effect_uncertain");
    assert!(
        descendant_was_alive,
        "post-reap capture failure must not blindly signal a reusable PGID"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn escaped_descendant_with_an_open_pipe_is_reported_as_uncertain() {
    let root = TempDir::new().expect("temporary workspace");
    let error = run(
        root.path(),
        json!({
            "program": "sh",
            "argv": [
                "-c",
                "setsid sh -c 'echo $$ > escaped.pid; exec sleep 30' &"
            ]
        }),
        1024,
    )
    .await
    .expect_err("an escaped pipe owner cannot be reported as cleaned up");

    let raw_pid: i32 = fs::read_to_string(root.path().join("escaped.pid"))
        .expect("escaped descendant records its pid")
        .trim()
        .parse()
        .expect("recorded pid is numeric");
    let pid = rustix::process::Pid::from_raw(raw_pid).expect("recorded pid is positive");
    let descendant_was_alive = rustix::process::test_kill_process_group(pid).is_ok();
    let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);

    assert_eq!(error.code, "tool_effect_uncertain");
    assert!(
        descendant_was_alive,
        "the fixture must prove why group cleanup could not be claimed"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn timed_out_parent_with_an_escaped_pipe_owner_is_uncertain() {
    let root = TempDir::new().expect("temporary workspace");
    let error = run(
        root.path(),
        json!({
            "program": "sh",
            "argv": [
                "-c",
                "setsid sh -c 'echo $$ > escaped-timeout.pid; exec sleep 30' & sleep 30"
            ],
            "timeout_ms": 200
        }),
        1024,
    )
    .await
    .expect_err("timeout cannot certify cleanup of an escaped pipe owner");

    let raw_pid: i32 = fs::read_to_string(root.path().join("escaped-timeout.pid"))
        .expect("escaped descendant records its pid")
        .trim()
        .parse()
        .expect("recorded pid is numeric");
    let pid = rustix::process::Pid::from_raw(raw_pid).expect("recorded pid is positive");
    let descendant_was_alive = rustix::process::test_kill_process_group(pid).is_ok();
    let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);

    assert_eq!(error.code, "tool_effect_uncertain");
    assert!(descendant_was_alive, "escaped fixture must still be alive");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn timed_out_parent_with_a_silent_escaped_descendant_is_uncertain() {
    let root = TempDir::new().expect("temporary workspace");
    let error = run(
        root.path(),
        json!({
            "program": "sh",
            "argv": [
                "-c",
                "setsid sh -c 'echo $$ > escaped-silent.pid; exec >/dev/null 2>&1; sleep 2; touch escaped-silent-marker' & sleep 30"
            ],
            "timeout_ms": 200
        }),
        1024,
    )
    .await
    .expect_err("post-spawn timeout cannot certify cleanup of a silent escapee");

    let raw_pid: i32 = fs::read_to_string(root.path().join("escaped-silent.pid"))
        .expect("escaped descendant records its pid")
        .trim()
        .parse()
        .expect("recorded pid is numeric");
    let pid = rustix::process::Pid::from_raw(raw_pid).expect("recorded pid is positive");
    let descendant_was_alive = rustix::process::test_kill_process_group(pid).is_ok();
    let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);

    assert_eq!(error.code, "tool_effect_uncertain");
    assert!(
        descendant_was_alive,
        "the silent escaped fixture must prove why cleanup is uncertain"
    );
    assert!(
        !root.path().join("escaped-silent-marker").exists(),
        "test cleanup must stop the escaped fixture before its delayed write"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_terminates_the_process_group() {
    let root = TempDir::new().expect("temporary workspace");
    let tool = ExecCommandTool::new(root.path()).expect("tool");
    let run_context = context(root.path(), 1024);
    let cancellation = run_context.cancellation.clone();
    let execution = tool.execute(
        json!({ "program": "sh", "argv": ["-c", "sleep 30"] }),
        run_context,
    );
    let cancel = async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation.cancel();
    };
    let (result, ()) = tokio::join!(execution, cancel);
    assert_eq!(
        result
            .expect_err("spawned command cancellation is uncertain")
            .code,
        "tool_effect_uncertain"
    );
}
