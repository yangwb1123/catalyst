use std::{fs, path::Path, process::Output};

use forge_runtime_domain::{GroupAgentGraphCorePlan, MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES};
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

mod group_agent_graph_run_support;
mod group_agent_graph_support;
use group_agent_graph_run_support::{
    Fixture, TASK_SECRET, WORKSPACE_SECRET, command, human_command, invoke_with_stdin, run_json,
};
use group_agent_graph_support::{path_text, successful_json, text};

#[test]
fn cli_prepares_replays_shows_and_lists_one_passive_core_plan() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let before = unrelated_counts(fixture.state.path());
    let graph_run_id = assert_prepare_and_replay(&fixture, &plan);
    assert_show_and_list(&fixture, &plan, &graph_run_id);
    assert_eq!(unrelated_counts(fixture.state.path()), before);
    assert_run_rows(fixture.state.path(), 1, 1);
    fixture.assert_workspace_unchanged();
}

fn assert_prepare_and_replay(fixture: &Fixture, plan: &GroupAgentGraphCorePlan) -> String {
    let created = successful_json(&fixture.prepare(plan, "passive-run-key"));
    assert_eq!(created["type"], "group_agent_graph_run_prepared");
    assert_eq!(created["disposition"], "created");
    assert_safe_inspection(&created["inspection"], false);
    assert_private(&created);
    let graph_run_id = text(&created["inspection"]["run"]["graph_run_id"]);
    let created_at = created["inspection"]["run"]["created_at_ms"].clone();

    let replayed = successful_json(&fixture.prepare(plan, "passive-run-key"));
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(replayed["inspection"]["run"]["graph_run_id"], graph_run_id);
    assert_eq!(replayed["inspection"]["run"]["created_at_ms"], created_at);
    graph_run_id
}

fn assert_show_and_list(fixture: &Fixture, plan: &GroupAgentGraphCorePlan, graph_run_id: &str) {
    let shown = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "show",
            graph_run_id,
            "--include-plan",
        ],
    );
    assert_eq!(shown["type"], "group_agent_graph_run");
    assert_safe_inspection(&shown["inspection"], true);
    assert_eq!(shown["inspection"]["plan"]["plan_sha256"], plan.plan_sha256);

    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "list", &fixture.graph_id()],
    );
    assert_eq!(listed["type"], "group_agent_graph_runs");
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["plan_and_journal_validated"], false);
    assert_eq!(listed["source_graph_validated"], false);
    assert_eq!(listed["runs"][0]["graph_run_id"], graph_run_id);
    assert_eq!(listed["dispatch_authority_released"], false);
    assert_eq!(listed["explicit_plan_file_read"], false);
    assert_eq!(listed["plan_included"], false);
    assert_all_effects_false(&listed);
}

#[test]
fn divergent_or_noncanonical_plan_never_creates_a_run() {
    let fixture = Fixture::new();
    let mut divergent = fixture.plan();
    divergent.graph_manifest_sha256 = "a".repeat(64);
    divergent.plan_sha256 = divergent.expected_sha256().expect("divergent digest");
    let output = fixture.prepare(&divergent, "divergent-plan-key");
    assert_rejected(&output, "failed");
    assert_run_rows(fixture.state.path(), 0, 0);

    let mut bytes = fixture
        .plan()
        .canonical_json()
        .expect("canonical plan")
        .into_bytes();
    bytes.push(b'\n');
    let output = invoke_with_stdin(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "prepare",
            &fixture.graph_id(),
            "--plan",
            "-",
            "--idempotency-key",
            "noncanonical-key",
        ],
        &bytes,
    );
    assert_rejected(&output, "not canonical");
    assert_run_rows(fixture.state.path(), 0, 0);
}

#[test]
fn malformed_plan_and_selectors_fail_before_hub_creation() {
    for bytes in [
        br#"{"v":2}"#.as_slice(),
        br#"{"v":"PLAN-SECRET","unknown":true}"#.as_slice(),
        &[0xff, 0xfe],
    ] {
        let state = TempDir::new().expect("isolated state");
        let cwd = TempDir::new().expect("isolated cwd");
        let output = invoke_with_stdin(
            state.path(),
            cwd.path(),
            &[
                "group",
                "graph",
                "run",
                "prepare",
                "missing-graph",
                "--plan",
                "-",
            ],
            bytes,
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("PLAN-SECRET"));
        assert!(!state.path().join("hub.sqlite3").exists());
    }
    let state = TempDir::new().expect("selector state");
    let cwd = TempDir::new().expect("selector cwd");
    let output = command(
        state.path(),
        cwd.path(),
        &["-C", ".", "group", "graph", "run", "list"],
    )
    .output()
    .expect("run selector rejection");
    assert_rejected(&output, "selectors are not valid");
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn oversized_plan_is_rejected_before_hub_creation() {
    let state = TempDir::new().expect("oversized state");
    let cwd = TempDir::new().expect("oversized cwd");
    let bytes = vec![b' '; MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES + 1];
    let output = invoke_with_stdin(
        state.path(),
        cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "prepare",
            "missing-graph",
            "--plan",
            "-",
        ],
        &bytes,
    );
    assert_rejected(&output, "byte limit");
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn explicit_plan_file_is_reported_without_workspace_discovery() {
    let fixture = Fixture::new();
    let plan_path = fixture.cwd.path().join("core-plan.json");
    fs::write(
        &plan_path,
        fixture.plan().canonical_json().expect("canonical plan"),
    )
    .expect("write explicit plan");
    let output = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "prepare",
            &fixture.graph_id(),
            "--plan",
            path_text(&plan_path),
            "--idempotency-key",
            "file-plan-key",
        ],
    );
    assert_eq!(output["inspection"]["explicit_plan_file_read"], true);
    assert_eq!(output["inspection"]["workspace_accessed"], false);
    fixture.assert_workspace_unchanged();
}

#[test]
fn human_plan_view_reveals_topology_and_every_no_effect_boundary() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let prepared = successful_json(&fixture.prepare(&plan, "human-plan-key"));
    let graph_run_id = text(&prepared["inspection"]["run"]["graph_run_id"]);
    let output = human_command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "show",
            &graph_run_id,
            "--include-plan",
        ],
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("human output is UTF-8");
    assert!(text.contains("authored nodes: frontend, backend, sso"));
    assert!(text.contains("edge backend -> sso"));
    assert!(text.contains("edge frontend -> sso"));
    assert!(text.contains("execution contract present=false"));
    assert!(text.contains("no Conversation/Prompt/memory/writeback operation occurred"));
    assert!(!text.contains(TASK_SECRET));
}

fn assert_safe_inspection(inspection: &Value, plan_included: bool) {
    assert_eq!(inspection["plan_admitted"], true);
    assert_eq!(inspection["source_graph_validated"], true);
    assert_eq!(inspection["plan_and_journal_validated"], true);
    assert_eq!(inspection["execution_contract_present"], false);
    assert_eq!(inspection["dispatch_authority_released"], false);
    assert_all_effects_false(inspection);
    assert_eq!(inspection["plan_included"], plan_included);
}

fn assert_all_effects_false(value: &Value) {
    for field in [
        "execution_performed",
        "manager_execution_performed",
        "node_execution_performed",
        "model_selected",
        "model_used",
        "capabilities_granted",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "task_results_produced",
        "conversation_or_prompt_written",
        "memory_written",
        "writeback_performed",
    ] {
        assert_eq!(value[field], false, "{field} must remain false");
    }
}

fn assert_private(value: &Value) {
    let encoded = serde_json::to_string(value).expect("output JSON");
    assert!(!encoded.contains(TASK_SECRET));
    assert!(!encoded.contains(WORKSPACE_SECRET));
    assert!(!encoded.contains("passive-run-key"));
}

fn assert_run_rows(state: &Path, runs: i64, events: i64) {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub database");
    assert_eq!(table_count(&connection, "group_agent_graph_runs"), runs);
    assert_eq!(
        table_count(&connection, "group_agent_graph_run_events"),
        events
    );
}

fn unrelated_counts(state: &Path) -> (i64, i64, i64, i64) {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub database");
    (
        table_count(&connection, "runs"),
        table_count(&connection, "run_events"),
        table_count(&connection, "prompts"),
        table_count(&connection, "group_agent_graphs"),
    )
}

fn table_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count table")
}

fn assert_rejected(output: &Output, message: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "stderr was {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
