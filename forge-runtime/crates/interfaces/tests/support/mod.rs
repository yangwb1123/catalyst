use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use forge_runtime_infrastructure::SqliteHubStore;
use serde_json::Value;
use tempfile::TempDir;

pub(crate) struct RunFixture {
    pub(crate) state: TempDir,
    pub(crate) project: TempDir,
    pub(crate) conversation_id: String,
    pub(crate) prompt_id: String,
}

pub(crate) fn invoke(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .output()
        .expect("run forge-runtime")
}

pub(crate) fn invoke_without_openai_key(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .env_remove("OPENAI_API_KEY")
        .output()
        .expect("run forge-runtime without an OpenAI key")
}

pub(crate) fn run_json(arguments: &[&str]) -> Value {
    let output = invoke(arguments);
    assert_success(&output);
    serde_json::from_slice(&output.stdout).expect("CLI emits JSON")
}

pub(crate) fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn path_text(path: &Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

pub(crate) fn fixture() -> RunFixture {
    let state = TempDir::new().expect("state directory");
    let project = TempDir::new().expect("project directory");
    fs::write(project.path().join("README.md"), "# Durable run\n").expect("workspace fixture");
    let created = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "-C",
        path_text(project.path()),
        "session",
        "new",
        "--title",
        "Durable execution",
    ]);
    let conversation_id = text_field(&created, &["session", "id"]);
    let prompt = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "prompt",
        "add",
        &conversation_id,
        "inspect the durable workspace",
    ]);
    let prompt_id = text_field(&prompt, &["prompt", "id"]);
    RunFixture {
        state,
        project,
        conversation_id,
        prompt_id,
    }
}

pub(crate) fn text_field(value: &Value, path: &[&str]) -> String {
    let mut current = value;
    for segment in path {
        current = &current[*segment];
    }
    current.as_str().expect("string field").to_owned()
}

pub(crate) fn start_arguments<'a>(fixture: &'a RunFixture, key: &'a str) -> Vec<&'a str> {
    vec![
        "--state-dir",
        path_text(fixture.state.path()),
        "--idempotency-key",
        key,
        "-C",
        path_text(fixture.project.path()),
        "run",
        "start",
        &fixture.conversation_id,
        &fixture.prompt_id,
        "--read",
        "README.md",
    ]
}

pub(crate) fn parse_jsonl(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("runtime event is JSON"))
        .collect()
}

#[allow(dead_code)]
pub(crate) fn assert_prompt_writeback(fixture: &RunFixture) {
    let prompts = run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "prompt",
        "list",
        &fixture.conversation_id,
        "--limit",
        "10",
    ]);
    let assistant: Vec<_> = prompts["prompts"]
        .as_array()
        .expect("prompt array")
        .iter()
        .filter(|prompt| prompt["role"] == "assistant")
        .collect();
    assert_eq!(assistant.len(), 1);
    assert!(
        assistant[0]["content"]
            .as_str()
            .is_some_and(|answer| answer.contains("Read README.md successfully"))
    );
}

pub(crate) fn fixture_store(fixture: &RunFixture) -> SqliteHubStore {
    SqliteHubStore::open(fixture.state.path().join("hub.sqlite3")).expect("open Hub fixture")
}
