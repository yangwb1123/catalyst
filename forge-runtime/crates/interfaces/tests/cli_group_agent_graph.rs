use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use forge_runtime_domain::MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES;
use rusqlite::Connection;
use serde_json::{Value, json};
use tempfile::TempDir;

mod group_agent_graph_support;
use group_agent_graph_support::{path_text, successful_json, text};

const GRAPH_KEY: &str = "stable-group-agent-graph-key";
const MANAGER_INSTRUCTION: &str =
    "Coordinate the frontend, backend, and SSO contracts.\nReport only accepted work.";
const FRONTEND_TASK: &str = "Implement the browser flow.\nPreserve the shared issuer.";
const WORKSPACE_SECRET: &str = "workspace-secret-must-not-enter-graph-output";
const STRICT_UNKNOWN_SPEC: &[u8] = br#"{"v":1,"manager":{"agent_profile":"manager","instruction":"plan"},"nodes":[{"node_id":"node","project_id":"project","member_role":"frontend","agent_profile":"implementer","task":"work","acceptance":"done"}],"edges":[],"capabilities":["network"]}"#;
const VERSION_2_SPEC: &[u8] = br#"{"v":2,"manager":{},"nodes":[],"edges":[]}"#;
const SECRET_INVALID_SPEC: &[u8] =
    br#"{"v":"TOP-SECRET-SPEC-VALUE","manager":{},"nodes":[],"edges":[]}"#;

struct Fixture {
    state: TempDir,
    projects: TempDir,
    cwd: TempDir,
    group_run_id: String,
    project_ids: BTreeMap<&'static str, String>,
}

impl Fixture {
    fn new() -> Self {
        let state = TempDir::new().expect("state directory");
        let projects = TempDir::new().expect("projects directory");
        let cwd = TempDir::new().expect("unrelated current directory");
        fs::write(cwd.path().join("private.txt"), WORKSPACE_SECRET).expect("workspace fixture");
        let group = run_json(
            state.path(),
            cwd.path(),
            &["group", "create", "Frontend backend SSO"],
        );
        let group_id = text(&group["group"]["id"]);
        let mut project_ids = BTreeMap::new();
        for role in ["frontend", "backend", "sso"] {
            let directory = projects.path().join(role);
            fs::create_dir(&directory).expect("project directory");
            let linked = run_json(
                state.path(),
                cwd.path(),
                &[
                    "group",
                    "add",
                    &group_id,
                    path_text(&directory),
                    "--role",
                    role,
                ],
            );
            project_ids.insert(role, text(&linked["member"]["project_id"]));
        }
        let prepared = run_json(
            state.path(),
            cwd.path(),
            &[
                "group",
                "run",
                "prepare",
                &group_id,
                "--idempotency-key",
                "graph-source-run",
            ],
        );
        Self {
            state,
            projects,
            cwd,
            group_run_id: text(&prepared["snapshot"]["run"]["run_id"]),
            project_ids,
        }
    }

    fn spec(&self) -> Value {
        json!({
            "v": 1,
            "manager": {
                "agent_profile": "integration-manager",
                "instruction": MANAGER_INSTRUCTION
            },
            "nodes": [
                {
                    "node_id": "frontend",
                    "project_id": self.project_ids["frontend"],
                    "member_role": "frontend",
                    "agent_profile": "implementer",
                    "task": FRONTEND_TASK,
                    "acceptance": "Browser uses the shared issuer."
                },
                {
                    "node_id": "backend",
                    "project_id": self.project_ids["backend"],
                    "member_role": "backend",
                    "agent_profile": "implementer",
                    "task": "Implement token verification.",
                    "acceptance": "API validates the shared issuer."
                },
                {
                    "node_id": "sso",
                    "project_id": self.project_ids["sso"],
                    "member_role": "sso",
                    "agent_profile": "reviewer",
                    "task": "Review both relying parties.",
                    "acceptance": "Both contracts agree before sign-off."
                }
            ],
            "edges": [
                {"from_node_id": "frontend", "to_node_id": "sso"},
                {"from_node_id": "backend", "to_node_id": "sso"}
            ]
        })
    }

    fn prepare(&self, spec: &Value, key: &str) -> Output {
        invoke_with_stdin(
            self.state.path(),
            self.cwd.path(),
            &[
                "group",
                "graph",
                "prepare",
                &self.group_run_id,
                "--spec",
                "-",
                "--idempotency-key",
                key,
            ],
            &serde_json::to_vec(spec).expect("spec JSON"),
        )
    }
}

#[test]
fn graph_cli_prepares_replays_inspects_and_lists_without_execution() {
    let fixture = Fixture::new();
    let spec = fixture.spec();
    let graph_id = assert_prepare_and_replay(&fixture, &spec);
    assert_show_views(&fixture, &graph_id);
    assert_list_view(&fixture, &graph_id);
}

fn assert_prepare_and_replay(fixture: &Fixture, spec: &Value) -> String {
    let created = successful_json(&fixture.prepare(spec, GRAPH_KEY));
    assert_eq!(created["type"], "group_agent_graph_prepared");
    assert_eq!(created["disposition"], "created");
    assert_safe_redacted_inspection(&created["inspection"]);
    assert_private(&created, fixture);
    let graph_id = text(&created["inspection"]["graph"]["graph_id"]);
    let created_at = created["inspection"]["graph"]["created_at_ms"].clone();
    let replayed = successful_json(&fixture.prepare(spec, GRAPH_KEY));
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(replayed["inspection"]["graph"]["graph_id"], graph_id);
    assert_eq!(replayed["inspection"]["graph"]["created_at_ms"], created_at);
    assert_safe_redacted_inspection(&replayed["inspection"]);
    graph_id
}

fn assert_show_views(fixture: &Fixture, graph_id: &str) {
    let hidden = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "show", graph_id],
    );
    assert_safe_redacted_inspection(&hidden["inspection"]);
    assert_private(&hidden, fixture);
    let revealed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "show", graph_id, "--include-spec"],
    );
    assert_eq!(revealed["inspection"]["spec_included"], true);
    assert_eq!(
        revealed["inspection"]["manifest"]["manager"]["instruction"],
        MANAGER_INSTRUCTION
    );
    assert_eq!(
        revealed["inspection"]["manifest"]["nodes"][0]["task"],
        FRONTEND_TASK
    );
    assert_eq!(
        revealed["inspection"]["manifest"]["edges"],
        json!([
            {"from_node_id": "backend", "to_node_id": "sso"},
            {"from_node_id": "frontend", "to_node_id": "sso"}
        ])
    );
    assert_eq!(
        revealed["inspection"]["manifest"]["waves"],
        json!([["frontend", "backend"], ["sso"]])
    );
    assert_no_execution(&revealed["inspection"]);
    assert_graph_honesty(&revealed["inspection"]);
}

fn assert_list_view(fixture: &Fixture, graph_id: &str) {
    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "list",
            &fixture.group_run_id,
            "--limit",
            "5",
        ],
    );
    assert_eq!(listed["type"], "group_agent_graphs");
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["source_and_manifest_validated"], false);
    assert_eq!(listed["graphs_are_prepared_only"], true);
    assert_eq!(listed["execution_performed"], false);
    assert_eq!(listed["manager_execution_performed"], false);
    assert_eq!(listed["node_execution_performed"], false);
    assert_eq!(listed["profiles_are_labels"], true);
    assert_eq!(listed["model_selected"], false);
    assert_eq!(listed["model_used"], false);
    assert_eq!(listed["capabilities_granted"], false);
    assert_eq!(listed["task_results_produced"], false);
    assert_eq!(listed["memory_written"], false);
    assert_eq!(listed["provider_used"], false);
    assert_eq!(listed["network_accessed"], false);
    assert_eq!(listed["workspace_scanned"], false);
    assert_eq!(listed["explicit_spec_file_read"], false);
    assert_eq!(listed["tools_used"], false);
    assert_eq!(listed["writeback_performed"], false);
    assert_eq!(listed["graphs"][0]["graph_id"], graph_id);
    assert!(listed["graphs"][0].get("manifest").is_none());
    assert_private(&listed, fixture);
}

#[test]
fn graph_cli_rejects_cycles_member_mismatch_and_key_conflicts_atomically() {
    let fixture = Fixture::new();
    let valid = fixture.spec();
    let created = successful_json(&fixture.prepare(&valid, GRAPH_KEY));
    let original_id = text(&created["inspection"]["graph"]["graph_id"]);

    let mut conflict = valid.clone();
    conflict["manager"]["instruction"] = json!("a different graph");
    assert_rejected(&fixture.prepare(&conflict, GRAPH_KEY), "idempotency key");

    let mut cycle = valid.clone();
    cycle["edges"]
        .as_array_mut()
        .expect("edges")
        .push(json!({"from_node_id": "sso", "to_node_id": "frontend"}));
    assert_rejected(&fixture.prepare(&cycle, "cycle-key"), "input is invalid");

    let mut wrong_role = valid;
    wrong_role["nodes"][0]["member_role"] = json!("sso");
    assert_rejected(
        &fixture.prepare(&wrong_role, "wrong-member-key"),
        "input is invalid",
    );

    let connection =
        Connection::open(fixture.state.path().join("hub.sqlite3")).expect("open Hub database");
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM group_agent_graphs", [], |row| {
            row.get(0)
        })
        .expect("count graphs");
    assert_eq!(rows, 1);
    let stored: String = connection
        .query_row("SELECT id FROM group_agent_graphs", [], |row| row.get(0))
        .expect("stored graph");
    assert_eq!(stored, original_id);
}

#[test]
fn invalid_spec_is_rejected_before_hub_creation() {
    for (bytes, forbidden) in [
        (VERSION_2_SPEC, "manager"),
        (br#"{"v":1,"manager":"#.as_slice(), r#"{"v":1"#),
        (SECRET_INVALID_SPEC, "TOP-SECRET-SPEC-VALUE"),
    ] {
        let state = TempDir::new().expect("parse-only state");
        let cwd = TempDir::new().expect("current directory");
        let output = invoke_with_stdin(
            state.path(),
            cwd.path(),
            &["group", "graph", "prepare", "unopened-run", "--spec", "-"],
            bytes,
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains(forbidden));
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

#[test]
fn graph_cli_accepts_an_explicit_spec_file() {
    let fixture = Fixture::new();
    let spec_path = fixture.cwd.path().join("graph-spec.json");
    fs::write(
        &spec_path,
        serde_json::to_vec(&fixture.spec()).expect("spec JSON"),
    )
    .expect("write graph spec");
    let output = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "prepare",
            &fixture.group_run_id,
            "--spec",
            path_text(&spec_path),
        ],
    );
    assert_safe_redacted_inspection(&output["inspection"]);
    assert_eq!(output["inspection"]["explicit_spec_file_read"], true);
}

#[test]
fn file_specs_are_bounded_utf8_and_strict_json() {
    assert_file_spec_rejected(STRICT_UNKNOWN_SPEC, "invalid Group Agent Graph spec JSON");
    assert_file_spec_rejected(&[0xff, 0xfe], "must be UTF-8");
    let oversized = vec![b' '; MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES + 1];
    assert_file_spec_rejected(&oversized, "byte limit");
}

#[test]
fn graph_management_rejects_space_selectors_before_hub_open() {
    for arguments in [
        vec!["-C", ".", "group", "graph", "list"],
        vec!["--group", "group-1", "group", "graph", "show", "graph-1"],
    ] {
        let state = TempDir::new().expect("selector state");
        let cwd = TempDir::new().expect("selector cwd");
        let output = command(state.path(), cwd.path(), &arguments)
            .output()
            .expect("run forge-runtime");
        assert_rejected(&output, "selectors are not valid");
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

#[test]
fn human_output_states_the_non_execution_boundary() {
    let fixture = Fixture::new();
    let created = successful_json(&fixture.prepare(&fixture.spec(), "human-output-key"));
    let graph_id = text(&created["inspection"]["graph"]["graph_id"]);
    let output = human_command(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "show", &graph_id],
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(text.contains("manager/node Agents not executed"));
    assert!(text.contains("descriptive labels only"));
    assert!(text.contains("no capabilities"));
    assert!(text.contains("task results: none"));
    assert!(text.contains("persistent memory: not written"));
    assert!(!text.contains(MANAGER_INSTRUCTION));
}

fn assert_safe_redacted_inspection(inspection: &Value) {
    assert_eq!(inspection["graph_prepared"], true);
    assert_eq!(inspection["source_and_manifest_validated"], true);
    assert_eq!(inspection["spec_included"], false);
    assert!(inspection.get("manifest").is_none());
    assert_no_execution(inspection);
    assert_graph_honesty(inspection);
}

fn assert_no_execution(inspection: &Value) {
    for field in [
        "execution_performed",
        "manager_execution_performed",
        "node_execution_performed",
        "model_selected",
        "model_used",
        "capabilities_granted",
        "task_results_produced",
        "memory_written",
        "provider_used",
        "network_accessed",
        "workspace_scanned",
        "tools_used",
        "writeback_performed",
    ] {
        assert_eq!(inspection[field], false, "{field} must remain false");
    }
}

fn assert_graph_honesty(inspection: &Value) {
    assert_eq!(inspection["profiles_are_labels"], true);
}

fn assert_private(output: &Value, fixture: &Fixture) {
    let encoded = output.to_string();
    for forbidden in [
        MANAGER_INSTRUCTION,
        FRONTEND_TASK,
        WORKSPACE_SECRET,
        GRAPH_KEY,
        "private.txt",
        path_text(fixture.projects.path()),
        path_text(fixture.cwd.path()),
    ] {
        assert!(!encoded.contains(forbidden), "output leaked {forbidden}");
    }
}

fn invoke_with_stdin(state: &Path, cwd: &Path, arguments: &[&str], input: &[u8]) -> Output {
    let mut child = command(state, cwd, arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge-runtime");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input)
        .expect("write graph spec");
    child.wait_with_output().expect("wait for forge-runtime")
}

fn assert_file_spec_rejected(bytes: &[u8], expected: &str) {
    let state = TempDir::new().expect("file-spec state");
    let cwd = TempDir::new().expect("file-spec cwd");
    let path = cwd.path().join("graph-spec.json");
    fs::write(&path, bytes).expect("write graph spec");
    let output = command(
        state.path(),
        cwd.path(),
        &[
            "group",
            "graph",
            "prepare",
            "missing-run",
            "--spec",
            path_text(&path),
        ],
    )
    .output()
    .expect("run forge-runtime");
    assert_rejected(&output, expected);
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn run_json(state: &Path, cwd: &Path, arguments: &[&str]) -> Value {
    successful_json(
        &command(state, cwd, arguments)
            .output()
            .expect("run forge-runtime"),
    )
}

fn command(state: &Path, cwd: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .arg("--state-dir")
        .arg(state)
        .arg("--json")
        .args(arguments);
    command
}

fn human_command(state: &Path, cwd: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .arg("--state-dir")
        .arg(state)
        .args(arguments)
        .output()
        .expect("run forge-runtime")
}

fn assert_rejected(output: &Output, message: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(message), "unexpected error: {stderr}");
}
