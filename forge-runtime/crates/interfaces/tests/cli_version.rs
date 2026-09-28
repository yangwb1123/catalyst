use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

fn invoke_version(root: &Path, arguments: &[&str], explicit_state: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command
        .current_dir(root)
        .env("FORGE_RUNTIME_HOME", root.join("default-state"))
        .env("OPENAI_BASE_URL", "invalid URL")
        .env("FORGE_API_URL", "invalid URL")
        .env_remove("OPENAI_API_KEY")
        .env_remove("FORGE_ACCESS_TOKEN");
    if let Some(state) = explicit_state {
        command.arg("--state-dir").arg(state);
    }
    command.args(arguments).output().expect("run version query")
}

fn assert_version_output(output: &Output, json: bool) {
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    assert!(output.stderr.is_empty());
    if json {
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!({"name": "forge-runtime", "version": env!("CARGO_PKG_VERSION")})
        );
    } else {
        assert_eq!(
            output.stdout,
            format!("forge-runtime {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
    }
}

#[test]
fn version_flags_and_json_need_no_model_configuration_or_default_state() {
    let root = TempDir::new().unwrap();
    for (arguments, json) in [
        (vec!["--version"], false),
        (vec!["-V"], false),
        (vec!["--json", "--version"], true),
        (vec!["--version", "--json"], true),
        (vec!["--json", "-V"], true),
        (vec!["-V", "--json"], true),
    ] {
        let output = invoke_version(root.path(), &arguments, None);
        assert_version_output(&output, json);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[test]
fn version_does_not_create_explicit_state_or_inspect_the_project() {
    let root = TempDir::new().unwrap();
    let state = root.path().join("missing-state");
    let output = invoke_version(
        root.path(),
        &["-C", "missing-project", "--json", "--version"],
        Some(&state),
    );
    assert_version_output(&output, true);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn version_does_not_open_or_migrate_an_existing_hub() {
    let root = TempDir::new().unwrap();
    let database = root.path().join("hub.sqlite3");
    let sentinel = b"this is deliberately not a SQLite database";
    fs::write(&database, sentinel).unwrap();
    let output = invoke_version(root.path(), &["-V"], Some(root.path()));
    assert_version_output(&output, false);
    assert_eq!(fs::read(&database).unwrap(), sentinel);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
