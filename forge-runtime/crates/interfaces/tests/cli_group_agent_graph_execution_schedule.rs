use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use forge_runtime_domain::MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES;
use rusqlite::{Connection, types::Value as SqlValue};
use serde_json::Value;
use tempfile::TempDir;

mod group_agent_graph_run_support;
mod group_agent_graph_support;
#[allow(dead_code)]
mod group_agent_node_dispatch_authorization_support;
use group_agent_graph_run_support::{
    Fixture, TASK_SECRET, WORKSPACE_SECRET, command, human_command, invoke_with_stdin, run_json,
};
use group_agent_graph_support::{successful_json, text};

const ADMISSION_KEY: &str = "real-go-rust-schedule-key";

#[derive(Debug, Eq, PartialEq)]
struct DurableSchedule {
    id: String,
    graph_run_id: String,
    blob: Vec<u8>,
    schedule_sha256: Vec<u8>,
    created_at_ms: i64,
}

#[test]
fn real_go_to_rust_schedule_pipeline_is_passive_private_and_replay_safe() {
    let fixture = Fixture::new();
    let graph_run_id = prepare_run(&fixture);
    let control = export_control(&fixture, &graph_run_id);
    let schedule = build_schedule_with_real_go(&control);
    assert_schedule_shape(&schedule, &graph_run_id);
    let main_before = main_graph_state(fixture.state.path());
    let unrelated_before = unrelated_counts(fixture.state.path());

    let created = successful_json(&invoke_with_stdin(
        fixture.state.path(),
        fixture.cwd.path(),
        &admit_args(&graph_run_id, ADMISSION_KEY),
        &schedule,
    ));
    let schedule_id = assert_created_is_private(&created, &graph_run_id);
    let durable = durable_schedule(fixture.state.path());

    assert_schedule_views(&fixture, &schedule_id, &schedule);
    assert_replay(&fixture, &graph_run_id, &schedule, &created);
    assert_eq!(durable_schedule(fixture.state.path()), durable);
    assert_conflicting_key_rejects(&fixture, &graph_run_id, &schedule);
    assert_eq!(main_graph_state(fixture.state.path()), main_before);
    assert_eq!(unrelated_counts(fixture.state.path()), unrelated_before);
    fixture.assert_workspace_unchanged();
    assert_historical_view_is_lifecycle_scoped(&fixture, &graph_run_id, &control, &schedule_id);
}

#[test]
fn invalid_schedule_fails_before_hub_creation() {
    reject_pre_hub(br#"{"v":"PRIVATE-SCHEDULE-INPUT"}"#, "noncanonical");
    reject_pre_hub(&[0xff, 0xfe], "noncanonical");

    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/group-agent-graph-execution-schedule-v1.json"
    ))
    .expect("shared schedule fixture");
    let mut noncanonical = text(&fixture["canonical_execution_schedule_json"]).into_bytes();
    noncanonical.push(b'\n');
    reject_pre_hub(&noncanonical, "noncanonical");

    let oversized = vec![b' '; MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES + 1];
    reject_pre_hub(&oversized, "byte limit");
}

fn prepare_run(fixture: &Fixture) -> String {
    let prepared = successful_json(&fixture.prepare(&fixture.plan(), "schedule-source-run"));
    assert_eq!(prepared["disposition"], "created");
    text(&prepared["inspection"]["run"]["graph_run_id"])
}

fn export_control(fixture: &Fixture, graph_run_id: &str) -> Vec<u8> {
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "control", "export", graph_run_id],
    )
    .output()
    .expect("export control");
    assert_success(&output);
    assert!(!output.stdout.ends_with(b"\n"));
    assert!(String::from_utf8_lossy(&output.stdout).contains(TASK_SECRET));
    output.stdout
}

fn build_schedule_with_real_go(control: &[u8]) -> Vec<u8> {
    let mut child = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "run",
            "./cmd/forge",
            "graph-execution-schedule",
            "--control",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn real forge-core");
    child
        .stdin
        .take()
        .expect("Go stdin")
        .write_all(control)
        .expect("write exact control");
    let output = child.wait_with_output().expect("wait for forge-core");
    assert_success(&output);
    assert!(!output.stdout.ends_with(b"\n"));
    output.stdout
}

fn assert_schedule_shape(schedule: &[u8], graph_run_id: &str) {
    let value: Value = serde_json::from_slice(schedule).expect("schedule JSON");
    assert_eq!(value["graph_run_id"], graph_run_id);
    assert_eq!(value["node_count"], 3);
    assert_eq!(value["wave_count"], 2);
    assert_eq!(value["execution_mode"], "serial");
    assert_eq!(value["max_in_flight_nodes"], 1);
    assert_eq!(
        value["initial_frontier"],
        serde_json::json!(["frontend", "backend"])
    );
    assert_eq!(value["initial_node"], "frontend");
    assert_eq!(
        value["nodes"][2]["direct_predecessor_node_ids"],
        serde_json::json!(["frontend", "backend"])
    );
    assert_eq!(value["execution_contract_present"], false);
    assert_eq!(value["dispatch_authority_released"], false);
    assert_eq!(value["progress_observed"], false);
    assert_eq!(value["successor_advanced"], false);
    let encoded = String::from_utf8_lossy(schedule);
    assert!(!encoded.contains(TASK_SECRET));
    assert!(!encoded.contains(WORKSPACE_SECRET));
}

fn admit_args<'a>(graph_run_id: &'a str, key: &'a str) -> [&'a str; 10] {
    [
        "group",
        "graph",
        "run",
        "schedule",
        "admit",
        graph_run_id,
        "--schedule",
        "-",
        "--idempotency-key",
        key,
    ]
}

fn assert_created_is_private(created: &Value, graph_run_id: &str) -> String {
    assert_eq!(
        created["type"],
        "group_agent_graph_execution_schedule_admitted"
    );
    assert_eq!(created["disposition"], "created");
    let inspection = &created["inspection"];
    assert_eq!(inspection["record"]["graph_run_id"], graph_run_id);
    assert_eq!(inspection["explicit_schedule_file_read"], false);
    assert_schedule_boundaries(inspection);
    assert_private(created);
    text(&inspection["record"]["schedule_id"])
}

fn assert_hidden_human_view(fixture: &Fixture, schedule_id: &str) {
    let human = human_command(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "schedule", "show", schedule_id],
    );
    assert_success(&human);
    let output = String::from_utf8(human.stdout).expect("human output");
    assert!(output.contains("passive scheduling artifact only"));
    assert!(output.contains("schedule hidden"));
    assert!(output.contains("current Run lifecycle is not reported"));
}

fn assert_historical_view_is_lifecycle_scoped(
    fixture: &Fixture,
    graph_run_id: &str,
    control: &[u8],
    schedule_id: &str,
) {
    let contract = group_agent_node_dispatch_authorization_support::build_contract_with_real_core(
        control,
        "https://api.openai.com/v1/responses",
    );
    group_agent_node_dispatch_authorization_support::admit_contract(
        fixture,
        graph_run_id,
        &contract,
    );
    let run = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "show", graph_run_id],
    );
    assert_eq!(run["inspection"]["run"]["execution_contract_present"], true);
    let schedule = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "schedule", "show", schedule_id],
    );
    assert_eq!(
        schedule["inspection"]["artifact_execution_contract_present"],
        false
    );
    assert_eq!(
        schedule["inspection"]["current_run_lifecycle_included"],
        false
    );
    assert!(
        schedule["inspection"]
            .get("execution_contract_present")
            .is_none()
    );
    assert_hidden_human_view(fixture, schedule_id);
}

fn assert_schedule_views(fixture: &Fixture, schedule_id: &str, schedule: &[u8]) {
    let hidden = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "schedule", "show", schedule_id],
    );
    assert_eq!(hidden["inspection"]["schedule_included"], false);
    assert_private(&hidden);

    let shown = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "schedule",
            "show",
            schedule_id,
            "--include-schedule",
        ],
    );
    let expected: Value = serde_json::from_slice(schedule).expect("expected schedule");
    assert_eq!(shown["inspection"]["schedule"], expected);
    assert_eq!(shown["inspection"]["schedule_included"], true);
    assert!(
        !serde_json::to_string(&shown)
            .expect("shown JSON")
            .contains(TASK_SECRET)
    );

    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "schedule", "list"],
    );
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["schedule_included"], false);
    assert_eq!(listed["artifact_execution_contract_present"], false);
    assert_eq!(listed["current_run_lifecycle_included"], false);
    assert!(listed.get("execution_contract_present").is_none());
    assert_eq!(listed["schedules"][0]["schedule_id"], schedule_id);
    assert_private(&listed);
    assert_hidden_human_view(fixture, schedule_id);
}

fn assert_replay(fixture: &Fixture, graph_run_id: &str, schedule: &[u8], created: &Value) {
    let replayed = successful_json(&invoke_with_stdin(
        fixture.state.path(),
        fixture.cwd.path(),
        &admit_args(graph_run_id, ADMISSION_KEY),
        schedule,
    ));
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(
        replayed["inspection"]["record"],
        created["inspection"]["record"]
    );
    assert_private(&replayed);
}

fn assert_conflicting_key_rejects(fixture: &Fixture, graph_run_id: &str, schedule: &[u8]) {
    let output = invoke_with_stdin(
        fixture.state.path(),
        fixture.cwd.path(),
        &admit_args(graph_run_id, "different-schedule-key"),
        schedule,
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("conflicts"));
}

fn assert_schedule_boundaries(inspection: &Value) {
    for field in [
        "schedule_admitted",
        "source_graph_validated",
        "control_snapshot_validated",
        "schedule_validated",
        "passive_policy_only",
    ] {
        assert_eq!(inspection[field], true, "{field} must be true");
    }
    for field in [
        "artifact_execution_contract_present",
        "artifact_dispatch_authority_released",
        "artifact_progress_observed",
        "artifact_successor_advanced",
        "current_run_lifecycle_included",
        "credential_read",
        "model_used",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "task_results_produced",
        "conversation_or_prompt_written",
        "memory_written",
        "writeback_performed",
        "schedule_included",
        "control_snapshot_included",
    ] {
        assert_eq!(inspection[field], false, "{field} must be false");
    }
}

fn assert_private(value: &Value) {
    let encoded = serde_json::to_string(value).expect("output JSON");
    for secret in [
        TASK_SECRET,
        WORKSPACE_SECRET,
        ADMISSION_KEY,
        "project_lane_sha256",
        "direct_predecessor_node_ids",
    ] {
        assert!(!encoded.contains(secret), "default output leaked {secret}");
    }
    for node_id in ["frontend", "backend", "sso"] {
        assert!(
            !encoded.contains(&format!("\"{node_id}\"")),
            "default output leaked node identity {node_id}"
        );
    }
    assert!(!encoded.contains("\"schedule\":"));
}

fn reject_pre_hub(input: &[u8], expected_error: &str) {
    let state = TempDir::new().expect("isolated state");
    let cwd = TempDir::new().expect("isolated cwd");
    let output = invoke_with_stdin(
        state.path(),
        cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "schedule",
            "admit",
            "missing-run",
            "--schedule",
            "-",
        ],
        input,
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected_error), "stderr was {stderr:?}");
    assert!(!stderr.contains("PRIVATE-SCHEDULE-INPUT"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn durable_schedule(state: &Path) -> DurableSchedule {
    Connection::open(state.join("hub.sqlite3"))
        .expect("open Hub")
        .query_row(
            "SELECT id,graph_run_id,schedule_blob,schedule_sha256,created_at_ms
             FROM group_agent_graph_execution_schedules",
            [],
            |row| {
                Ok(DurableSchedule {
                    id: row.get(0)?,
                    graph_run_id: row.get(1)?,
                    blob: row.get(2)?,
                    schedule_sha256: row.get(3)?,
                    created_at_ms: row.get(4)?,
                })
            },
        )
        .expect("durable schedule")
}

fn main_graph_state(state: &Path) -> (Vec<Vec<SqlValue>>, Vec<Vec<SqlValue>>) {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    (
        query_rows(
            &connection,
            "SELECT * FROM group_agent_graph_runs ORDER BY id",
        ),
        query_rows(
            &connection,
            "SELECT * FROM group_agent_graph_run_events ORDER BY graph_run_id,seq",
        ),
    )
}

fn query_rows(connection: &Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut statement = connection.prepare(sql).expect("prepare snapshot query");
    let columns = statement.column_count();
    statement
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get(index))
                .collect::<Result<_, _>>()
        })
        .expect("query snapshot")
        .collect::<Result<_, _>>()
        .expect("collect snapshot")
}

fn unrelated_counts(state: &Path) -> (i64, i64, i64) {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    (
        table_count(&connection, "runs"),
        table_count(&connection, "run_events"),
        table_count(&connection, "prompts"),
    )
}

fn table_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count table")
}

fn forge_core_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("forge-core")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
