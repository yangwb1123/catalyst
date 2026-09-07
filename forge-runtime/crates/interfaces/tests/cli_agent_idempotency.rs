#[path = "cli_agent_e2e_support/mod.rs"]
#[allow(dead_code)]
mod e2e_support;
#[allow(dead_code)]
mod support;

use std::{process::Command, sync::Arc, thread, time::Duration};

use forge_runtime_application::{HubService, RunService};
use forge_runtime_domain::{Conversation, ConversationScope, PromptRecord, RunRecord};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

use e2e_support::{LocalResponses, final_stream, invoke_keyed_agent, invoke_keyed_agent_json};
use support::{assert_success, fixture, invoke, start_arguments};

const CLOSED_LOOPBACK: &str = "http://127.0.0.1:9/v1";

#[derive(Debug, Eq, PartialEq)]
struct PersistedAgentState {
    sessions: Vec<Conversation>,
    prompts: Vec<PromptRecord>,
    runs: Vec<RunRecord>,
}

struct DriftCase<'a> {
    project: &'a std::path::Path,
    options: &'a [&'a str],
    prompt: &'a str,
}

#[test]
fn legacy_run_key_conflict_does_not_create_agent_session_or_prompt() {
    let fixture = fixture();
    let key = "shared-legacy-agent-key";
    let started = invoke(&start_arguments(&fixture, key));
    assert_success(&started);
    let before = persisted_agent_state(fixture.state.path());

    let conflict = invoke_keyed_agent(
        CLOSED_LOOPBACK,
        fixture.state.path(),
        fixture.project.path(),
        key,
        &[],
        "conflicting prompt",
    );

    assert_idempotency_conflict(&conflict);
    assert_eq!(persisted_agent_state(fixture.state.path()), before);
}

#[cfg(unix)]
#[test]
fn agent_key_replays_exactly_without_additional_mutation_or_model_access() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let key = "stable-agent-key";
    let server = LocalResponses::start(vec![final_stream("original answer")]);
    let endpoint = server.endpoint().to_owned();
    let options = ["--model", "offline-test-model"];

    let first = invoke_keyed_agent(
        &endpoint,
        state.path(),
        project.path(),
        key,
        &options,
        "original prompt",
    );
    assert_success(&first);
    let after_first = persisted_agent_state(state.path());
    let replay = invoke_keyed_agent(
        &endpoint,
        state.path(),
        project.path(),
        key,
        &options,
        "original prompt",
    );
    assert_success(&replay);
    let json_replay = invoke_keyed_agent_json(
        &endpoint,
        state.path(),
        project.path(),
        key,
        &options,
        "original prompt",
    );
    assert_success(&json_replay);
    assert_eq!(server.finish().len(), 1);
    assert_eq!(persisted_agent_state(state.path()), after_first);
    let human = String::from_utf8(replay.stdout).expect("human replay output");
    assert!(human.contains("[assistant] original answer"), "{human}");
    assert!(human.contains("[run] completed"), "{human}");
    let events: Vec<serde_json::Value> = String::from_utf8(json_replay.stdout)
        .expect("JSON replay output")
        .lines()
        .map(|line| serde_json::from_str(line).expect("replayed event JSON"))
        .collect();
    assert_eq!(events.first().unwrap()["type"], "run_started");
    assert_eq!(events.last().unwrap()["type"], "run_finished");
}

#[cfg(unix)]
#[test]
fn different_key_lock_loser_announces_its_durable_run_for_explicit_resume() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let server =
        LocalResponses::start_delayed(vec![final_stream("first answer")], Duration::from_secs(2));
    let first = spawn_keyed_agent(
        server.endpoint(),
        state.path(),
        project.path(),
        "first-different-key",
    );
    thread::sleep(Duration::from_millis(250));

    let loser = invoke_keyed_agent(
        server.endpoint(),
        state.path(),
        project.path(),
        "second-different-key",
        &["--model", "offline-test-model"],
        "second prompt",
    );
    assert!(!loser.status.success());
    let stderr = String::from_utf8(loser.stderr).expect("loser stderr");
    assert!(stderr.contains("run: run-"), "{stderr}");
    assert!(stderr.contains("explicit run resume"), "{stderr}");

    let first = first.wait_with_output().expect("first Agent exits");
    assert_success(&first);
    assert_eq!(server.finish().len(), 1);
    let runs = persisted_agent_state(state.path()).runs;
    assert_eq!(runs.len(), 2);
}

#[cfg(unix)]
#[test]
fn agent_key_drift_is_rejected_without_session_prompt_or_run_mutation() {
    let project = TempDir::new().expect("project");
    let other_project = TempDir::new().expect("other project");
    let state = TempDir::new().expect("state");
    let key = "drift-agent-key";
    let server = LocalResponses::start(vec![final_stream("original answer")]);
    let endpoint = server.endpoint().to_owned();
    let options = ["--model", "offline-test-model"];
    assert_success(&invoke_keyed_agent(
        &endpoint,
        state.path(),
        project.path(),
        key,
        &options,
        "original prompt",
    ));
    server.finish();

    let alternate = create_alternate_session(state.path(), project.path());
    let baseline = persisted_agent_state(state.path());
    let changed_model = ["--model", "different-model"];
    let changed_session = ["--model", "offline-test-model", "--session", &alternate];
    let cases = [
        DriftCase {
            project: project.path(),
            options: &options,
            prompt: "changed prompt",
        },
        DriftCase {
            project: project.path(),
            options: &changed_model,
            prompt: "original prompt",
        },
        DriftCase {
            project: other_project.path(),
            options: &options,
            prompt: "original prompt",
        },
        DriftCase {
            project: project.path(),
            options: &changed_session,
            prompt: "original prompt",
        },
    ];
    for case in &cases {
        assert_drift_is_read_only(state.path(), key, &endpoint, &baseline, case);
    }
}

fn persisted_agent_state(state: &std::path::Path) -> PersistedAgentState {
    let store = Arc::new(SqliteHubStore::open(state.join("hub.sqlite3")).expect("open Agent Hub"));
    let hub = HubService::new(store.clone());
    PersistedAgentState {
        sessions: hub.global_snapshot().expect("Hub snapshot").conversations,
        prompts: hub.list_prompts(None, 1_000).expect("all prompts"),
        runs: RunService::new(store)
            .list_runs(None, 1_000)
            .expect("all Runs"),
    }
}

fn create_alternate_session(state: &std::path::Path, project: &std::path::Path) -> String {
    let store = Arc::new(SqliteHubStore::open(state.join("hub.sqlite3")).expect("open Agent Hub"));
    let hub = HubService::new(store);
    let project = hub
        .open_project(&project.canonicalize().expect("canonical Project"))
        .expect("persisted Project");
    hub.create_session(
        &ConversationScope::Project(project.id),
        "Alternate Agent session",
        "alternate-agent-session",
    )
    .expect("create alternate session")
    .id
}

fn assert_drift_is_read_only(
    state: &std::path::Path,
    key: &str,
    endpoint: &str,
    baseline: &PersistedAgentState,
    case: &DriftCase<'_>,
) {
    let output = invoke_keyed_agent(
        endpoint,
        state,
        case.project,
        key,
        case.options,
        case.prompt,
    );
    assert_idempotency_conflict(&output);
    assert_eq!(&persisted_agent_state(state), baseline);
}

fn assert_idempotency_conflict(output: &std::process::Output) {
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .to_lowercase()
            .contains("idempotency")
    );
}

#[cfg(unix)]
fn spawn_keyed_agent(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
    key: &str,
) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--state-dir",
            state.to_str().unwrap(),
            "--idempotency-key",
            key,
            "-C",
            project.to_str().unwrap(),
            "agent",
            "--model",
            "offline-test-model",
            "first prompt",
        ])
        .env("OPENAI_BASE_URL", endpoint)
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn keyed Agent")
}
