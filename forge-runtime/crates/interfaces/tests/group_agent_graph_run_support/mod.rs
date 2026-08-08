// Support module shared by CLI integration tests; each test file uses
// a subset of its helpers, so dead_code is expected here.
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_CORE_PLAN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphCorePlan, GroupAgentGraphEdge,
};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::group_agent_graph_support::{path_text, successful_json, text};

pub(super) const TASK_SECRET: &str = "private-task-must-not-leak-from-passive-run";
pub(super) const WORKSPACE_SECRET: &str = "workspace-secret-must-not-be-read";

#[allow(dead_code)]
pub(super) struct Fixture {
    pub(super) state: TempDir,
    projects: TempDir,
    pub(super) cwd: TempDir,
    graph: Value,
    fan_out: bool,
    sentinels: BTreeMap<&'static str, Vec<u8>>,
}

#[allow(dead_code)]
impl Fixture {
    pub(super) fn new() -> Self {
        let state = TempDir::new().expect("state directory");
        let projects = TempDir::new().expect("projects directory");
        let cwd = TempDir::new().expect("unrelated current directory");
        let (group_run_id, project_ids, sentinels) =
            setup_group_source(state.path(), projects.path(), cwd.path());
        let graph = prepare_graph(state.path(), cwd.path(), &group_run_id, &project_ids);
        Self {
            state,
            projects,
            cwd,
            graph,
            fan_out: false,
            sentinels,
        }
    }

    /// Fan-out topology (frontend -> backend, sso): both successors become
    /// ready once frontend is consumed — the two-ready-node wave shape
    /// (review Finding 6).
    pub(super) fn fan_out() -> Self {
        let state = TempDir::new().expect("state directory");
        let projects = TempDir::new().expect("projects directory");
        let cwd = TempDir::new().expect("unrelated current directory");
        let (group_run_id, project_ids, sentinels) =
            setup_group_source(state.path(), projects.path(), cwd.path());
        let graph = prepare_graph_fan_out(state.path(), cwd.path(), &group_run_id, &project_ids);
        Self {
            state,
            projects,
            cwd,
            graph,
            fan_out: true,
            sentinels,
        }
    }

    pub(super) fn graph_id(&self) -> String {
        text(&self.graph["inspection"]["graph"]["graph_id"])
    }

    pub(super) fn plan(&self) -> GroupAgentGraphCorePlan {
        let mut plan = plan_for(&self.graph);
        if self.fan_out {
            plan.edges = vec![edge("frontend", "backend"), edge("frontend", "sso")];
            plan.waves = vec![vec!["frontend".into()], vec![
                "backend".into(),
                "sso".into(),
            ]];
            plan.plan_sha256 = plan.expected_sha256().expect("fan-out plan digest");
        }
        plan
    }

    pub(super) fn prepare(&self, plan: &GroupAgentGraphCorePlan, key: &str) -> Output {
        invoke_with_stdin(
            self.state.path(),
            self.cwd.path(),
            &[
                "group",
                "graph",
                "run",
                "prepare",
                &self.graph_id(),
                "--plan",
                "-",
                "--idempotency-key",
                key,
            ],
            plan.canonical_json().expect("canonical plan").as_bytes(),
        )
    }

    pub(super) fn assert_workspace_unchanged(&self) {
        for (role, expected) in &self.sentinels {
            let actual =
                fs::read(self.projects.path().join(role).join("private.txt")).expect("sentinel");
            assert_eq!(&actual, expected);
        }
    }

    #[allow(dead_code)] // This support module is compiled independently by several CLI test crates.
    pub(super) fn remove_member_workspaces(&self) {
        for role in ["frontend", "backend", "sso"] {
            fs::remove_dir_all(self.projects.path().join(role)).expect("remove member workspace");
        }
    }
}

fn setup_group_source(
    state: &Path,
    projects: &Path,
    cwd: &Path,
) -> (
    String,
    BTreeMap<&'static str, String>,
    BTreeMap<&'static str, Vec<u8>>,
) {
    let group = run_json(state, cwd, &["group", "create", "Graph Run fixture"]);
    let group_id = text(&group["group"]["id"]);
    let (project_ids, sentinels) = link_projects(state, projects, cwd, &group_id);
    let frozen = run_json(state, cwd, &[
        "group",
        "run",
        "prepare",
        &group_id,
        "--idempotency-key",
        "graph-run-source",
    ]);
    (
        text(&frozen["snapshot"]["run"]["run_id"]),
        project_ids,
        sentinels,
    )
}

fn link_projects(
    state: &Path,
    projects: &Path,
    cwd: &Path,
    group_id: &str,
) -> (
    BTreeMap<&'static str, String>,
    BTreeMap<&'static str, Vec<u8>>,
) {
    let mut project_ids = BTreeMap::new();
    let mut sentinels = BTreeMap::new();
    for role in ["frontend", "backend", "sso"] {
        let directory = projects.join(role);
        fs::create_dir(&directory).expect("project directory");
        let bytes = format!("{WORKSPACE_SECRET}-{role}").into_bytes();
        fs::write(directory.join("private.txt"), &bytes).expect("workspace sentinel");
        let linked = run_json(state, cwd, &[
            "group",
            "add",
            group_id,
            path_text(&directory),
            "--role",
            role,
        ]);
        project_ids.insert(role, text(&linked["member"]["project_id"]));
        sentinels.insert(role, bytes);
    }
    (project_ids, sentinels)
}

fn prepare_graph(
    state: &Path,
    cwd: &Path,
    group_run_id: &str,
    project_ids: &BTreeMap<&'static str, String>,
) -> Value {
    successful_json(&invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "prepare",
            group_run_id,
            "--spec",
            "-",
            "--idempotency-key",
            "graph-run-graph",
        ],
        &serde_json::to_vec(&graph_spec(project_ids)).expect("spec JSON"),
    ))
}

fn prepare_graph_fan_out(
    state: &Path,
    cwd: &Path,
    group_run_id: &str,
    project_ids: &BTreeMap<&'static str, String>,
) -> Value {
    successful_json(&invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "prepare",
            group_run_id,
            "--spec",
            "-",
            "--idempotency-key",
            "graph-run-graph-fan-out",
        ],
        &serde_json::to_vec(&graph_spec_fan_out(project_ids)).expect("fan-out spec JSON"),
    ))
}

fn plan_for(graph_output: &Value) -> GroupAgentGraphCorePlan {
    let graph = &graph_output["inspection"]["graph"];
    let mut plan = GroupAgentGraphCorePlan {
        v: GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        graph_version: 1,
        graph_id: text(&graph["graph_id"]),
        graph_manifest_sha256: text(&graph["manifest_sha256"]),
        authored_node_ids: vec!["frontend".into(), "backend".into(), "sso".into()],
        edges: vec![edge("backend", "sso"), edge("frontend", "sso")],
        waves: vec![vec!["frontend".into(), "backend".into()], vec![
            "sso".into(),
        ]],
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan_sha256: "0".repeat(64),
    };
    plan.plan_sha256 = plan.expected_sha256().expect("plan digest");
    plan
}

fn graph_spec_fan_out(projects: &BTreeMap<&'static str, String>) -> Value {
    json!({
        "v": 1,
        "manager": {
            "agent_profile": "integration-manager",
            "instruction": "coordinate without executing"
        },
        "nodes": [
            node("frontend", &projects["frontend"], "frontend"),
            node("backend", &projects["backend"], "backend"),
            node("sso", &projects["sso"], "sso")
        ],
        "edges": [
            {"from_node_id": "frontend", "to_node_id": "backend"},
            {"from_node_id": "frontend", "to_node_id": "sso"}
        ]
    })
}

fn graph_spec(projects: &BTreeMap<&'static str, String>) -> Value {
    json!({
        "v": 1,
        "manager": {
            "agent_profile": "integration-manager",
            "instruction": "coordinate without executing"
        },
        "nodes": [
            node("frontend", &projects["frontend"], "frontend"),
            node("backend", &projects["backend"], "backend"),
            node("sso", &projects["sso"], "sso")
        ],
        "edges": [
            {"from_node_id": "frontend", "to_node_id": "sso"},
            {"from_node_id": "backend", "to_node_id": "sso"}
        ]
    })
}

fn node(node_id: &str, project_id: &str, role: &str) -> Value {
    json!({
        "node_id": node_id,
        "project_id": project_id,
        "member_role": role,
        "agent_profile": "implementer",
        "task": format!("{TASK_SECRET}-{node_id}"),
        "acceptance": format!("{node_id} contract passes")
    })
}

fn edge(from: &str, to: &str) -> GroupAgentGraphEdge {
    GroupAgentGraphEdge {
        from_node_id: from.into(),
        to_node_id: to.into(),
    }
}

pub(super) fn run_json(state: &Path, cwd: &Path, args: &[&str]) -> Value {
    successful_json(&command(state, cwd, args).output().expect("run CLI"))
}

pub(super) fn invoke_with_stdin(state: &Path, cwd: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = command(state, cwd, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    child.wait_with_output().expect("wait for CLI")
}

pub(super) fn command(state: &Path, cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args(["--state-dir", path_text(state), "--json"])
        .args(args);
    command
}

#[allow(dead_code)]
pub(super) fn human_command(state: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args(["--state-dir", path_text(state)])
        .args(args)
        .output()
        .expect("run human CLI")
}
