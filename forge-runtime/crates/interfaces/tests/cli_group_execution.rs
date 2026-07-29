use std::{fs, path::Path, process::Command};

use forge_runtime_domain::{
    BeginGroupExecution, BeginGroupExecutionDisposition, GROUP_EXECUTION_PROTOCOL_VERSION,
    GROUP_EXECUTION_VERSION, GroupExecutionEvent, GroupExecutionEventKind, GroupExecutionMode,
    GroupExecutionOutcome, GroupExecutionReceipt, GroupExecutionRecovery, GroupExecutionStore,
    GroupRunSnapshot,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

const API_KEY_SENTINEL: &str = "provider-must-not-be-invoked";
const FROZEN_PROMPT: &str = "frozen cross-project prompt";
const LATER_PROMPT: &str = "newer prompt outside the frozen snapshot";
const EXECUTION_KEY: &str = "recover-this-execution";

fn invoke(state: &Path, cwd: &Path, json: bool, arguments: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command
        .current_dir(cwd)
        .env("OPENAI_API_KEY", API_KEY_SENTINEL)
        .arg("--state-dir")
        .arg(state);
    if json {
        command.arg("--json");
    }
    command.args(arguments).output().expect("run forge-runtime")
}

fn run_json(state: &Path, cwd: &Path, arguments: &[&str]) -> Value {
    let output = invoke(state, cwd, true, arguments);
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI emits JSON")
}

#[test]
fn start_without_an_explicit_key_fails_before_creating_hub_state() {
    let state = TempDir::new().expect("state directory");
    let cwd = TempDir::new().expect("working directory");

    let output = invoke(
        state.path(),
        cwd.path(),
        true,
        &["group", "execution", "start", "group-run-1"],
    );

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires an explicit --idempotency-key for durable recovery")
    );
    assert!(!state.path().join("hub.sqlite3").exists());
}

struct Fixture {
    state: TempDir,
    project: TempDir,
    unrelated_cwd: TempDir,
    group_id: String,
    session_id: String,
}

impl Fixture {
    fn new() -> Self {
        let state = TempDir::new().expect("state directory");
        let project = TempDir::new().expect("project directory");
        let unrelated_cwd = TempDir::new().expect("unrelated current directory");
        fs::write(project.path().join("must-not-be-read"), "workspace secret")
            .expect("workspace fixture");
        let group_id = create_group(state.path(), unrelated_cwd.path());
        add_project(
            state.path(),
            unrelated_cwd.path(),
            &group_id,
            project.path(),
        );
        let session_id = create_session(state.path(), unrelated_cwd.path(), &group_id);
        let fixture = Self {
            state,
            project,
            unrelated_cwd,
            group_id,
            session_id,
        };
        fixture.add_prompt(FROZEN_PROMPT);
        fixture
    }

    fn add_prompt(&self, content: &str) {
        run_json(
            self.state.path(),
            self.unrelated_cwd.path(),
            &["prompt", "add", &self.session_id, content],
        );
    }

    fn prepare(&self) -> String {
        let output = run_json(
            self.state.path(),
            self.unrelated_cwd.path(),
            &[
                "group",
                "run",
                "prepare",
                &self.group_id,
                "--idempotency-key",
                "frozen-source",
            ],
        );
        output["snapshot"]["run"]["run_id"]
            .as_str()
            .expect("Group Run ID")
            .to_owned()
    }

    fn start(&self, group_run_id: &str) -> Value {
        run_json(
            self.state.path(),
            self.unrelated_cwd.path(),
            &[
                "group",
                "execution",
                "start",
                group_run_id,
                "--idempotency-key",
                EXECUTION_KEY,
            ],
        )
    }
}

#[test]
fn local_execution_replays_frozen_receipt_without_runtime_side_effects() {
    let fixture = Fixture::new();
    let group_run_id = fixture.prepare();
    fixture.add_prompt(LATER_PROMPT);
    fs::remove_dir_all(fixture.project.path()).expect("remove source workspace");

    let first = fixture.start(&group_run_id);
    assert_started(&first, "created", &group_run_id);
    let replay = fixture.start(&group_run_id);
    assert_started(&replay, "replayed", &group_run_id);
    assert_eq!(replay["inspection"], first["inspection"]);

    let execution_id = first["inspection"]["execution"]["execution_id"]
        .as_str()
        .expect("execution ID");
    let shown = fixture.show(execution_id);
    assert_eq!(shown["inspection"], first["inspection"]);
    let listed = fixture.list(&group_run_id);
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["source_and_journal_validated"], false);
    assert_eq!(listed["inspect_with"], "group execution show EXECUTION_ID");
    assert_eq!(listed["executions"].as_array().map(Vec::len), Some(1));
    assert_eq!(listed["executions"][0], first["inspection"]["execution"]);

    assert_private_output(&first, &fixture);
    assert_private_output(&shown, &fixture);
    assert_human_boundaries(&fixture, execution_id);
    assert_no_runtime_side_effects(fixture.state.path());
}

#[test]
fn explicit_key_recovers_a_persisted_prefix_across_cli_processes() {
    let fixture = Fixture::new();
    let group_run_id = fixture.prepare();
    let database = fixture.state.path().join("hub.sqlite3");
    let store = SqliteHubStore::open(&database).expect("reopen Hub");
    let begun = store
        .begin_group_execution(&BeginGroupExecution {
            v: GROUP_EXECUTION_VERSION,
            execution_id: "interrupted-execution".into(),
            group_run_id: group_run_id.clone(),
            mode: GroupExecutionMode::OfflineSnapshotValidation,
            idempotency_key: EXECUTION_KEY.into(),
            created_at_ms: 10,
        })
        .expect("persist incomplete intent");
    assert_eq!(begun.disposition, BeginGroupExecutionDisposition::Created);
    let events = expected_execution_events(&begun.execution, &begun.snapshot);
    for event in &events[..2] {
        store
            .append_group_execution_event(event)
            .expect("persist deterministic prefix");
    }
    let prefix = store
        .inspect_group_execution("interrupted-execution")
        .expect("inspect durable prefix");
    assert_eq!(prefix.events.len(), 2);
    assert!(matches!(
        prefix.recovery,
        GroupExecutionRecovery::Incomplete
    ));
    drop(store);

    let recovered = fixture.start(&group_run_id);
    assert_started(&recovered, "replayed", &group_run_id);
    assert_eq!(
        recovered["inspection"]["execution"]["execution_id"],
        "interrupted-execution"
    );
    assert_no_runtime_side_effects(fixture.state.path());
}

impl Fixture {
    fn show(&self, execution_id: &str) -> Value {
        run_json(
            self.state.path(),
            self.unrelated_cwd.path(),
            &["group", "execution", "show", execution_id],
        )
    }

    fn list(&self, group_run_id: &str) -> Value {
        run_json(
            self.state.path(),
            self.unrelated_cwd.path(),
            &["group", "execution", "list", group_run_id, "--limit", "5"],
        )
    }
}

fn create_group(state: &Path, cwd: &Path) -> String {
    let output = run_json(state, cwd, &["group", "create", "Frontend backend SSO"]);
    output["group"]["id"].as_str().expect("group ID").to_owned()
}

fn add_project(state: &Path, cwd: &Path, group_id: &str, project: &Path) {
    run_json(
        state,
        cwd,
        &[
            "group",
            "add",
            group_id,
            project.to_str().expect("UTF-8 project path"),
            "--role",
            "sso",
        ],
    );
}

fn create_session(state: &Path, cwd: &Path, group_id: &str) -> String {
    let output = run_json(
        state,
        cwd,
        &[
            "--group",
            group_id,
            "session",
            "new",
            "--title",
            "Contract discussion",
        ],
    );
    output["session"]["id"]
        .as_str()
        .expect("session ID")
        .to_owned()
}

fn assert_started(output: &Value, disposition: &str, group_run_id: &str) {
    assert_eq!(output["type"], "group_execution_started");
    assert_eq!(output["disposition"], disposition);
    assert_eq!(
        output["inspection"]["execution"]["group_run_id"],
        group_run_id
    );
    assert_eq!(output["inspection"]["execution"]["status"], "completed");
    assert_eq!(output["inspection"]["event_count"], 3);
    assert_eq!(output["inspection"]["recovery"]["status"], "terminal");
    assert_eq!(
        output["inspection"]["recovery"]["outcome"],
        "snapshot_validated"
    );
    assert!(output["inspection"]["receipt"].is_object());
}

fn assert_private_output(output: &Value, fixture: &Fixture) {
    let encoded = output.to_string();
    for forbidden in [
        API_KEY_SENTINEL,
        EXECUTION_KEY,
        FROZEN_PROMPT,
        LATER_PROMPT,
        "workspace secret",
        "\"content\"",
        "\"excerpt\"",
        "\"context_json\"",
        "\"idempotency_key\"",
        "\"events\"",
    ] {
        assert!(!encoded.contains(forbidden), "output leaked {forbidden}");
    }
    assert!(!encoded.contains(fixture.project.path().to_string_lossy().as_ref()));
}

fn assert_human_boundaries(fixture: &Fixture, execution_id: &str) {
    let shown = human_output(fixture, &["group", "execution", "show", execution_id]);
    assert!(shown.contains("snapshot-validation receipt"));
    assert!(shown.contains("frozen snapshot integrity: validated"));
    assert_common_boundaries(&shown);

    let listed = human_output(fixture, &["group", "execution", "list"]);
    assert!(listed.contains("status is recorded metadata"));
    assert!(!listed.contains("frozen snapshot integrity: validated"));
    assert_common_boundaries(&listed);
}

fn human_output(fixture: &Fixture, arguments: &[&str]) -> String {
    let output = invoke(
        fixture.state.path(),
        fixture.unrelated_cwd.path(),
        false,
        arguments,
    );
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("human output is UTF-8")
}

fn assert_common_boundaries(text: &str) {
    for boundary in [
        "model/provider: not invoked",
        "analysis/discussion/task result: not produced",
        "workspace/tools/network: unavailable",
    ] {
        assert!(text.contains(boundary), "missing boundary: {boundary}");
    }
    assert!(!text.contains(FROZEN_PROMPT));
}

fn assert_no_runtime_side_effects(state: &Path) {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub fixture");
    assert_eq!(table_count(&connection, "group_executions"), 1);
    assert_eq!(table_count(&connection, "group_execution_events"), 3);
    assert_eq!(table_count(&connection, "runs"), 0);
    assert_eq!(table_count(&connection, "run_events"), 0);
    assert_eq!(table_count(&connection, "run_assistant_prompts"), 0);
    let assistants: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM prompts WHERE role = 'assistant'",
            [],
            |row| row.get(0),
        )
        .expect("count assistant Prompts");
    assert_eq!(assistants, 0);
}

fn expected_execution_events(
    execution: &forge_runtime_domain::GroupExecutionRecord,
    snapshot: &GroupRunSnapshot,
) -> [GroupExecutionEvent; 3] {
    let receipt = GroupExecutionReceipt {
        v: GROUP_EXECUTION_VERSION,
        execution_id: execution.execution_id.clone(),
        group_run_id: execution.group_run_id.clone(),
        group_id: snapshot.run.group_id.clone(),
        context_version: snapshot.run.context_version,
        context_slice_sha256: snapshot.run.context_slice_sha256.clone(),
        snapshot_sha256: snapshot.run.snapshot_sha256.clone(),
        snapshot_bytes: snapshot.run.snapshot_bytes,
        stats: snapshot.context.payload.stats.clone(),
    };
    [
        execution_event(
            execution,
            1,
            GroupExecutionEventKind::ExecutionStarted {
                group_run_id: execution.group_run_id.clone(),
                snapshot_sha256: execution.source_snapshot_sha256.clone(),
            },
        ),
        execution_event(
            execution,
            2,
            GroupExecutionEventKind::SnapshotVerified { receipt },
        ),
        execution_event(
            execution,
            3,
            GroupExecutionEventKind::ExecutionFinished {
                outcome: GroupExecutionOutcome::SnapshotValidated,
            },
        ),
    ]
}

fn execution_event(
    execution: &forge_runtime_domain::GroupExecutionRecord,
    seq: u64,
    kind: GroupExecutionEventKind,
) -> GroupExecutionEvent {
    GroupExecutionEvent {
        v: GROUP_EXECUTION_PROTOCOL_VERSION,
        execution_id: execution.execution_id.clone(),
        seq,
        kind,
    }
}

fn table_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count table rows")
}
