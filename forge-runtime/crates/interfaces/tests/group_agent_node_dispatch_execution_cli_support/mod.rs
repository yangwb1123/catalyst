use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_CORE_PLAN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphCorePlan,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::group_agent_graph_support::{path_text, successful_json, text};

mod multi_node;
mod quarantine;

pub(super) const CREDENTIAL_SECRET: &str = "credential-must-not-leak\r\nx-private-header: rejected";
pub(super) const TASK_SECRET: &str = "single-node-private-task-must-not-leak";
const MODEL: &str = "private-dispatch-execution-model";
const OFFICIAL_ENDPOINT: &str = "https://api.openai.com/v1/responses";
const PRICING_RATE: &str = "2000000";

pub(super) struct Fixture {
    pub(super) state: TempDir,
    pub(super) cwd: TempDir,
    project: TempDir,
    sentinel: Vec<u8>,
    pub(super) graph_run_id: String,
    authorization_path: PathBuf,
    pricing_path: PathBuf,
    core_bin: PathBuf,
    core_sha256: String,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let state = TempDir::new().expect("state directory");
        let cwd = TempDir::new().expect("current directory");
        let project = TempDir::new().expect("project directory");
        let sentinel = b"workspace-must-not-be-read-or-written".to_vec();
        fs::write(project.path().join("private.txt"), &sentinel).expect("workspace sentinel");
        let (graph_id, manifest_sha256) =
            prepare_single_node_graph(state.path(), cwd.path(), project.path());
        let graph_run_id = prepare_graph_run(state.path(), cwd.path(), &graph_id, &manifest_sha256);
        let core_bin = build_go_core(cwd.path());
        let core_sha256 = file_sha256(&core_bin);
        let (authorization, pricing) =
            prepare_authority(state.path(), cwd.path(), &graph_run_id, &core_bin);
        let authorization_path = cwd.path().join("authorization.json");
        let pricing_path = cwd.path().join("pricing.json");
        fs::write(&authorization_path, authorization).expect("authorization fixture");
        fs::write(&pricing_path, pricing).expect("pricing fixture");
        Self {
            state,
            cwd,
            project,
            sentinel,
            graph_run_id,
            authorization_path,
            pricing_path,
            core_bin,
            core_sha256,
        }
    }

    pub(super) fn execute(&self, consent: bool, credential: Option<&str>) -> Output {
        self.execute_with_result_visibility(consent, credential, false)
    }

    pub(super) fn execute_include_result(&self, consent: bool, credential: Option<&str>) -> Output {
        self.execute_with_result_visibility(consent, credential, true)
    }

    fn execute_with_result_visibility(
        &self,
        consent: bool,
        credential: Option<&str>,
        include_result: bool,
    ) -> Output {
        let mut args = vec![
            "group",
            "graph",
            "run",
            "dispatch",
            "execute",
            &self.graph_run_id,
            "--authorization",
            path_text(&self.authorization_path),
            "--pricing",
            path_text(&self.pricing_path),
            "--core-bin",
            path_text(&self.core_bin),
            "--core-bin-sha256",
            &self.core_sha256,
        ];
        if consent {
            args.push("--confirm-off-machine");
        }
        if include_result {
            args.push("--include-result");
        }
        let mut process = runtime_command(self.state.path(), self.cwd.path());
        process.args(args);
        if let Some(value) = credential {
            process.env("OPENAI_API_KEY", value);
        }
        process.output().expect("execute dispatch CLI")
    }

    pub(super) fn replace_authorization(&self, bytes: &[u8]) {
        fs::write(&self.authorization_path, bytes).expect("replace authorization fixture");
    }

    pub(super) fn replace_pricing(&self, bytes: &[u8]) {
        fs::write(&self.pricing_path, bytes).expect("replace pricing fixture");
    }

    pub(super) fn state_bytes(&self) -> BTreeMap<String, Vec<u8>> {
        state_bytes(self.state.path())
    }

    pub(super) fn assert_workspace_unchanged(&self) {
        assert_eq!(
            fs::read(self.project.path().join("private.txt")).expect("workspace sentinel"),
            self.sentinel
        );
    }
}

fn prepare_single_node_graph(state: &Path, cwd: &Path, project: &Path) -> (String, String) {
    let (group_run_id, project_id) = prepare_source(state, cwd, project);
    let spec = single_node_spec(&project_id);
    let graph = successful_json(&invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "prepare",
            &group_run_id,
            "--spec",
            "-",
            "--idempotency-key",
            "single-node-graph",
        ],
        &serde_json::to_vec(&spec).unwrap(),
    ));
    (
        text(&graph["inspection"]["graph"]["graph_id"]),
        text(&graph["inspection"]["graph"]["manifest_sha256"]),
    )
}

fn prepare_source(state: &Path, cwd: &Path, project: &Path) -> (String, String) {
    let group = run_json(state, cwd, &["group", "create", "single-node-dispatch"]);
    let group_id = text(&group["group"]["id"]);
    let member = run_json(
        state,
        cwd,
        &[
            "group",
            "add",
            &group_id,
            path_text(project),
            "--role",
            "worker",
        ],
    );
    let project_id = text(&member["member"]["project_id"]);
    let source = run_json(
        state,
        cwd,
        &[
            "group",
            "run",
            "prepare",
            &group_id,
            "--idempotency-key",
            "single-node-source",
        ],
    );
    let group_run_id = text(&source["snapshot"]["run"]["run_id"]);
    (group_run_id, project_id)
}

fn single_node_spec(project_id: &str) -> Value {
    json!({
        "v": 1,
        "manager": {
            "agent_profile": "single-node-manager",
            "instruction": "Coordinate exactly one bounded node."
        },
        "nodes": [{
            "node_id": "worker",
            "project_id": project_id,
            "member_role": "worker",
            "agent_profile": "implementer",
            "task": TASK_SECRET,
            "acceptance": "return one bounded result"
        }],
        "edges": []
    })
}

fn prepare_graph_run(state: &Path, cwd: &Path, graph_id: &str, manifest: &str) -> String {
    let mut plan = GroupAgentGraphCorePlan {
        v: GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        graph_version: 1,
        graph_id: graph_id.into(),
        graph_manifest_sha256: manifest.into(),
        authored_node_ids: vec!["worker".into()],
        edges: Vec::new(),
        waves: vec![vec!["worker".into()]],
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan_sha256: "0".repeat(64),
    };
    plan.plan_sha256 = plan.expected_sha256().expect("plan digest");
    let output = successful_json(&invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "run",
            "prepare",
            graph_id,
            "--plan",
            "-",
            "--idempotency-key",
            "single-node-run",
        ],
        plan.canonical_json().unwrap().as_bytes(),
    ));
    text(&output["inspection"]["run"]["graph_run_id"])
}

fn prepare_authority(
    state: &Path,
    cwd: &Path,
    graph_run_id: &str,
    core: &Path,
) -> (Vec<u8>, Vec<u8>) {
    let control = raw_runtime(
        state,
        cwd,
        &["group", "graph", "run", "control", "export", graph_run_id],
        None,
    );
    let (contract, pricing) = build_priced_contract(core, &control.stdout);
    admit_contract(state, cwd, graph_run_id, &contract);
    prepare_dispatch(state, cwd, graph_run_id);
    let authorization = export_authorization(state, cwd, graph_run_id, core);
    (authorization, pricing)
}

fn build_priced_contract(core: &Path, control: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let pricing = core_output(core, pricing_args(), None);
    let pricing_json: Value = serde_json::from_slice(&pricing).expect("pricing JSON");
    let pricing_sha256 = text(&pricing_json["pricing_snapshot_sha256"]);
    let contract = core_output(core, contract_args(&pricing_sha256), Some(control));
    (contract, pricing)
}

fn admit_contract(state: &Path, cwd: &Path, graph_run_id: &str, contract: &[u8]) {
    successful_json(&invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "run",
            "contract",
            "admit",
            graph_run_id,
            "--contract",
            "-",
            "--idempotency-key",
            "single-node-contract",
        ],
        contract,
    ));
}

fn prepare_dispatch(state: &Path, cwd: &Path, graph_run_id: &str) {
    successful_json(&raw_runtime(
        state,
        cwd,
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "prepare",
            graph_run_id,
            "--idempotency-key",
            "single-node-dispatch",
        ],
        None,
    ));
}

fn export_authorization(state: &Path, cwd: &Path, graph_run_id: &str, core: &Path) -> Vec<u8> {
    let release = raw_runtime(
        state,
        cwd,
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "release-control",
            "export",
            graph_run_id,
        ],
        None,
    );
    core_output(
        core,
        vec!["graph-node-dispatch-authorize", "--control", "-"],
        Some(&release.stdout),
    )
}

fn pricing_args() -> Vec<&'static str> {
    vec![
        "graph-node-pricing-snapshot",
        "--model",
        MODEL,
        "--input-usd-micros-per-token-unit",
        PRICING_RATE,
        "--output-usd-micros-per-token-unit",
        "10000000",
        "--max-input-tokens",
        "400000",
    ]
}

fn contract_args(pricing_sha256: &str) -> Vec<&str> {
    vec![
        "graph-node-contract",
        "--control",
        "-",
        "--endpoint",
        OFFICIAL_ENDPOINT,
        "--model",
        MODEL,
        "--max-output-tokens",
        "1024",
        "--max-model-output-bytes",
        "8192",
        "--max-model-events",
        "128",
        "--timeout-ms",
        "30000",
        "--max-cost-usd-micros",
        "1000000",
        "--pricing-snapshot-sha256",
        pricing_sha256,
        "--max-result-bytes",
        "16384",
    ]
}

fn build_go_core(directory: &Path) -> PathBuf {
    let output = directory.join("forge-core-test-bin");
    let status = Command::new("go")
        .current_dir(repository_root().join("forge-core"))
        .env("GOTOOLCHAIN", "local")
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args(["build", "-trimpath", "-o"])
        .arg(&output)
        .arg("./cmd/forge")
        .status()
        .expect("build real Go Core");
    assert!(status.success(), "real Go Core build failed");
    output.canonicalize().expect("canonical Go Core path")
}

fn core_output(core: &Path, args: Vec<&str>, input: Option<&[u8]>) -> Vec<u8> {
    let mut child = Command::new(core)
        .args(args)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn real Go Core");
    if let Some(bytes) = input {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    let output = child.wait_with_output().expect("wait for real Go Core");
    assert_success(&output);
    output.stdout
}

fn invoke_with_stdin(state: &Path, cwd: &Path, args: &[&str], input: &[u8]) -> Output {
    raw_runtime(state, cwd, args, Some(input))
}

fn raw_runtime(state: &Path, cwd: &Path, args: &[&str], input: Option<&[u8]>) -> Output {
    let mut child = runtime_command(state, cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn runtime CLI");
    if let Some(bytes) = input {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    let output = child.wait_with_output().expect("wait for runtime CLI");
    assert_success(&output);
    output
}

fn run_json(state: &Path, cwd: &Path, args: &[&str]) -> Value {
    successful_json(&raw_runtime(state, cwd, args, None))
}

fn runtime_command(state: &Path, cwd: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .args(["--state-dir", path_text(state), "--json"]);
    command
}

fn file_sha256(path: &Path) -> String {
    format!(
        "{:x}",
        Sha256::digest(fs::read(path).expect("read Go Core"))
    )
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root")
}

fn state_bytes(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .expect("state directory")
        .map(|entry| {
            let entry = entry.expect("state entry");
            let name = entry.file_name().into_string().expect("UTF-8 state name");
            (name, fs::read(entry.path()).expect("state file"))
        })
        .collect()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
