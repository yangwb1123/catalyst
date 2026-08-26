use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::OnceLock,
};

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_CORE_PLAN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphCorePlan, GroupAgentGraphEdge,
};
use rusqlite::{Connection, types::Value as SqlValue};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::{
    group_agent_graph_run_support::{command, invoke_with_stdin, run_json},
    group_agent_graph_support::{path_text, successful_json, text},
};

pub(super) const CREDENTIAL_POISON: &str =
    "scheduled-reconcile-credential-must-not-be-read\r\nx-private-header: rejected";
pub(super) const TASK_SECRET: &str = "scheduled-reconcile-task-secret";
pub(super) const WORKSPACE_SECRET: &str = "scheduled-reconcile-workspace-secret";

pub(super) struct PinnedGoCore {
    _directory: TempDir,
    pub(super) path: PathBuf,
    pub(super) sha256: String,
}

impl PinnedGoCore {
    fn build() -> Self {
        let directory = TempDir::new().expect("Go Core build directory");
        let output = directory.path().join("forge");
        let status = Command::new("go")
            .args(["build", "-trimpath", "-o"])
            .arg(&output)
            .arg("./cmd/forge")
            .current_dir(repository_root().join("forge-core"))
            .env("GOPROXY", "off")
            .env("GOSUMDB", "off")
            .env("GOTOOLCHAIN", "local")
            .status()
            .expect("build real Go Core");
        assert!(status.success(), "real Go Core build failed");
        let path = output.canonicalize().expect("canonical Go Core path");
        let sha256 = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
        Self {
            _directory: directory,
            path,
            sha256,
        }
    }
}

pub(super) fn shared_core() -> &'static PinnedGoCore {
    static CORE: OnceLock<PinnedGoCore> = OnceLock::new();
    CORE.get_or_init(PinnedGoCore::build)
}

pub(super) struct ReconcileFixture {
    pub(super) state: TempDir,
    pub(super) cwd: TempDir,
    _projects: TempDir,
    sentinels: BTreeMap<PathBuf, Vec<u8>>,
    pub(super) graph_run_id: String,
}

impl ReconcileFixture {
    pub(super) fn new(core: &PinnedGoCore) -> Self {
        let state = TempDir::new().expect("state directory");
        let cwd = TempDir::new().expect("unrelated cwd");
        let projects = TempDir::new().expect("project directories");
        let (group_run_id, project_ids, sentinels) =
            create_group_source(state.path(), cwd.path(), projects.path());
        let graph = prepare_graph(state.path(), cwd.path(), &group_run_id, &project_ids);
        let graph_run_id = prepare_run(state.path(), cwd.path(), &graph);
        let control = export_control(state.path(), cwd.path(), &graph_run_id);
        let schedule = build_schedule(core, &control);
        admit_schedule(state.path(), cwd.path(), &graph_run_id, &schedule);
        Self {
            state,
            cwd,
            _projects: projects,
            sentinels,
            graph_run_id,
        }
    }

    pub(super) fn reconcile(&self, core: &PinnedGoCore, digest: &str, endpoint: &str) -> Output {
        command(
            self.state.path(),
            self.cwd.path(),
            &[
                "group",
                "graph",
                "run",
                "reconcile",
                &self.graph_run_id,
                "--core-bin",
                path_text(&core.path),
                "--core-bin-sha256",
                digest,
            ],
        )
        .env("OPENAI_API_KEY", CREDENTIAL_POISON)
        .env("OPENAI_BASE_URL", endpoint)
        .output()
        .expect("run scheduled reconcile CLI")
    }

    pub(super) fn assert_workspace_unchanged(&self) {
        for (path, expected) in &self.sentinels {
            assert_eq!(&fs::read(path).expect("workspace sentinel"), expected);
        }
    }

    pub(super) fn hub_state(&self) -> BTreeMap<String, Vec<Vec<SqlValue>>> {
        snapshot_hub(self.state.path())
    }
}

pub(super) fn loopback_sentinel() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback sentinel");
    listener
        .set_nonblocking(true)
        .expect("nonblocking sentinel");
    let port = listener.local_addr().expect("sentinel address").port();
    (listener, format!("https://127.0.0.1:{port}/v1/responses"))
}

fn create_group_source(
    state: &Path,
    cwd: &Path,
    projects: &Path,
) -> (
    String,
    BTreeMap<&'static str, String>,
    BTreeMap<PathBuf, Vec<u8>>,
) {
    let group = run_json(state, cwd, &["group", "create", "Reconcile fixture"]);
    let group_id = text(&group["group"]["id"]);
    let (project_ids, sentinels) = link_projects(state, cwd, projects, &group_id);
    let prepared = run_json(
        state,
        cwd,
        &[
            "group",
            "run",
            "prepare",
            &group_id,
            "--idempotency-key",
            "scheduled-reconcile-group-run",
        ],
    );
    (
        text(&prepared["snapshot"]["run"]["run_id"]),
        project_ids,
        sentinels,
    )
}

fn link_projects(
    state: &Path,
    cwd: &Path,
    projects: &Path,
    group_id: &str,
) -> (BTreeMap<&'static str, String>, BTreeMap<PathBuf, Vec<u8>>) {
    let mut project_ids = BTreeMap::new();
    let mut sentinels = BTreeMap::new();
    for role in ["build", "verify"] {
        let directory = projects.join(role);
        fs::create_dir(&directory).expect("project directory");
        let path = directory.join("private.txt");
        let bytes = format!("{WORKSPACE_SECRET}-{role}").into_bytes();
        fs::write(&path, &bytes).expect("workspace sentinel");
        let linked = run_json(
            state,
            cwd,
            &[
                "group",
                "add",
                group_id,
                path_text(&directory),
                "--role",
                role,
            ],
        );
        project_ids.insert(role, text(&linked["member"]["project_id"]));
        sentinels.insert(path, bytes);
    }
    (project_ids, sentinels)
}

fn prepare_graph(
    state: &Path,
    cwd: &Path,
    group_run_id: &str,
    projects: &BTreeMap<&'static str, String>,
) -> Value {
    let spec = graph_spec(projects);
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
            "scheduled-reconcile-graph",
        ],
        &serde_json::to_vec(&spec).expect("graph spec JSON"),
    ))
}

fn prepare_run(state: &Path, cwd: &Path, graph: &Value) -> String {
    let plan = plan_for(graph);
    let output = invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "run",
            "prepare",
            &text(&graph["inspection"]["graph"]["graph_id"]),
            "--plan",
            "-",
            "--idempotency-key",
            "scheduled-reconcile-graph-run",
        ],
        plan.canonical_json().expect("canonical plan").as_bytes(),
    );
    text(&successful_json(&output)["inspection"]["run"]["graph_run_id"])
}

fn export_control(state: &Path, cwd: &Path, graph_run_id: &str) -> Vec<u8> {
    let output = command(
        state,
        cwd,
        &["group", "graph", "run", "control", "export", graph_run_id],
    )
    .output()
    .expect("export Graph control");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn build_schedule(core: &PinnedGoCore, control: &[u8]) -> Vec<u8> {
    let mut child = Command::new(&core.path)
        .args(["graph-execution-schedule", "--control", "-"])
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start real Go schedule Core");
    child.stdin.take().unwrap().write_all(control).unwrap();
    let output = child.wait_with_output().expect("wait for schedule Core");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).expect("schedule JSON");
    assert_eq!(value["node_count"], 2, "fixture must remain two-node");
    output.stdout
}

fn admit_schedule(state: &Path, cwd: &Path, graph_run_id: &str, schedule: &[u8]) {
    let output = invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "run",
            "schedule",
            "admit",
            graph_run_id,
            "--schedule",
            "-",
            "--idempotency-key",
            "scheduled-reconcile-schedule",
        ],
        schedule,
    );
    successful_json(&output);
}

fn plan_for(graph: &Value) -> GroupAgentGraphCorePlan {
    let graph = &graph["inspection"]["graph"];
    let mut plan = GroupAgentGraphCorePlan {
        v: GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        graph_version: 1,
        graph_id: text(&graph["graph_id"]),
        graph_manifest_sha256: text(&graph["manifest_sha256"]),
        authored_node_ids: vec!["build".into(), "verify".into()],
        edges: vec![GroupAgentGraphEdge {
            from_node_id: "build".into(),
            to_node_id: "verify".into(),
        }],
        waves: vec![vec!["build".into()], vec!["verify".into()]],
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan_sha256: "0".repeat(64),
    };
    plan.plan_sha256 = plan.expected_sha256().expect("plan digest");
    plan
}

fn graph_spec(projects: &BTreeMap<&'static str, String>) -> Value {
    json!({
        "v": 1,
        "manager": {"agent_profile": "manager", "instruction": "observe only"},
        "nodes": [
            graph_node("build", &projects["build"]),
            graph_node("verify", &projects["verify"])
        ],
        "edges": [{"from_node_id": "build", "to_node_id": "verify"}]
    })
}

fn graph_node(node_id: &str, project_id: &str) -> Value {
    json!({
        "node_id": node_id,
        "project_id": project_id,
        "member_role": node_id,
        "agent_profile": "implementer",
        "task": format!("{TASK_SECRET}-{node_id}"),
        "acceptance": format!("{node_id} accepted")
    })
}

fn snapshot_hub(state: &Path) -> BTreeMap<String, Vec<Vec<SqlValue>>> {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub snapshot");
    table_names(&connection)
        .into_iter()
        .map(|name| {
            let rows = snapshot_table(&connection, &name);
            (name, rows)
        })
        .collect()
}

fn table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' \
             ORDER BY name",
        )
        .expect("prepare Hub table inventory");
    statement
        .query_map([], |row| row.get(0))
        .expect("query Hub table inventory")
        .collect::<Result<_, _>>()
        .expect("collect Hub table inventory")
}

fn snapshot_table(connection: &Connection, table: &str) -> Vec<Vec<SqlValue>> {
    let quoted = format!("\"{}\"", table.replace('"', "\"\""));
    let columns = connection
        .prepare(&format!("SELECT * FROM {quoted} LIMIT 0"))
        .expect("prepare table shape")
        .column_count();
    let order = (1..=columns)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(",");
    query_rows(
        connection,
        &format!("SELECT * FROM {quoted} ORDER BY {order}"),
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

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root")
}
