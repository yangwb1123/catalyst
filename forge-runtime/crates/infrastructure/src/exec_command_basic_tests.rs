use super::environment::{is_allowed_environment_name, is_sensitive_environment_name};
use super::{ExecCommandTool, blocking_task_failure};
use crate::runtime_domain::{AgentTool, Capability, TOOL_EFFECT_UNCERTAIN_CODE};
use tempfile::TempDir;

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
