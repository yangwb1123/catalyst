use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::{Command, Output},
};

use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatch, ClaimGroupModelAnalysisDispatchResult,
    ClaimGroupPanelSynthesisDispatch, ClaimGroupPanelSynthesisDispatchResult,
    CompleteGroupModelAnalysis, GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
    GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_RESULT_VERSION,
    GROUP_MODEL_ANALYSIS_VERSION, GROUP_PANEL_SYNTHESIS_CONSENT_VERSION,
    GROUP_PANEL_SYNTHESIS_VERSION, GroupModelAnalysisOutcome, GroupModelAnalysisResult,
    GroupModelAnalysisResultArtifact, GroupModelAnalysisStore, GroupPanelSynthesisStore, Usage,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

pub const FROZEN_PROMPT: &str = "frontend, backend, and SSO share one frozen issuer contract";
pub const FIRST_ANSWER: &str = "frontend preserves the shared issuer.";
pub const SECOND_ANSWER: &str = "backend and SSO validate that issuer.";
pub const WORKSPACE_SECRET: &str = "workspace secret must not enter synthesis";
pub const SYNTHESIS_KEY: &str = "stable-synthesis-idempotency-key";

pub struct Fixture {
    pub state: TempDir,
    pub project: TempDir,
    pub cwd: TempDir,
    pub panel_id: String,
}

impl Fixture {
    pub fn new() -> Self {
        let state = TempDir::new().expect("state directory");
        let project = TempDir::new().expect("project directory");
        let cwd = TempDir::new().expect("unrelated working directory");
        fs::write(project.path().join("private.txt"), WORKSPACE_SECRET).expect("workspace fixture");
        let group_id = create_group(state.path(), cwd.path());
        add_project(state.path(), cwd.path(), &group_id, project.path());
        let session_id = create_session(state.path(), cwd.path(), &group_id);
        add_prompt(state.path(), cwd.path(), &session_id, FROZEN_PROMPT);
        let group_run_id = prepare_group_run(state.path(), cwd.path(), &group_id);
        let first = prepare_analysis(state.path(), cwd.path(), &group_run_id, "analysis-key-a");
        let second = prepare_analysis(state.path(), cwd.path(), &group_run_id, "analysis-key-b");
        complete_analysis(
            state.path(),
            &first,
            "dispatch-a",
            FIRST_ANSWER,
            9_999_999_999_990,
        );
        complete_analysis(
            state.path(),
            &second,
            "dispatch-b",
            SECOND_ANSWER,
            9_999_999_999_991,
        );
        let panel_id = prepare_panel(state.path(), cwd.path(), &group_run_id, [&first, &second]);
        Self {
            state,
            project,
            cwd,
            panel_id,
        }
    }

    pub fn prepare_synthesis(&self) -> Value {
        run_json(
            self.state.path(),
            self.cwd.path(),
            &[
                "group",
                "synthesis",
                "prepare",
                &self.panel_id,
                "--model",
                "test-model",
                "--max-output-tokens",
                "1024",
                "--idempotency-key",
                SYNTHESIS_KEY,
            ],
        )
    }

    pub fn show(&self, synthesis_id: &str) -> Value {
        run_json(
            self.state.path(),
            self.cwd.path(),
            &["group", "synthesis", "show", synthesis_id],
        )
    }

    pub fn invoke(&self, arguments: &[&str], api_key: Option<&str>) -> Output {
        invoke(self.state.path(), self.cwd.path(), arguments, api_key)
    }

    pub fn claim_synthesis(&self, synthesis_id: &str) {
        let store = SqliteHubStore::open(self.state.path().join("hub.sqlite3"))
            .expect("open synthesis store");
        let result = store
            .claim_group_panel_synthesis_dispatch(&ClaimGroupPanelSynthesisDispatch {
                v: GROUP_PANEL_SYNTHESIS_VERSION,
                synthesis_id: synthesis_id.into(),
                dispatch_id: "simulated-crash-dispatch".into(),
                consent_version: GROUP_PANEL_SYNTHESIS_CONSENT_VERSION,
                released_at_ms: 9_999_999_999_999,
            })
            .expect("claim synthesis");
        assert!(matches!(
            result,
            ClaimGroupPanelSynthesisDispatchResult::Claimed { .. }
        ));
    }

    pub fn request_and_config(&self, synthesis_id: &str) -> (Vec<u8>, String) {
        let connection = self.connection();
        connection
            .query_row(
                "SELECT request_body,config_json FROM group_panel_syntheses WHERE id=?1",
                [synthesis_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("stored synthesis")
    }

    pub fn synthesis_event_count(&self, synthesis_id: &str) -> i64 {
        self.connection()
            .query_row(
                "SELECT COUNT(*) FROM group_panel_synthesis_events WHERE synthesis_id=?1",
                [synthesis_id],
                |row| row.get(0),
            )
            .expect("synthesis event count")
    }

    pub fn side_effect_counts(&self) -> Vec<i64> {
        let connection = self.connection();
        [
            "conversations",
            "prompts",
            "runs",
            "run_events",
            "run_assistant_prompts",
            "group_executions",
        ]
        .iter()
        .map(|table| {
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("side-effect count")
        })
        .collect()
    }

    fn connection(&self) -> Connection {
        Connection::open(self.state.path().join("hub.sqlite3")).expect("open SQLite")
    }
}

pub fn run_json(state: &Path, cwd: &Path, arguments: &[&str]) -> Value {
    let output = invoke(state, cwd, arguments, None);
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI JSON")
}

fn invoke(state: &Path, cwd: &Path, arguments: &[&str], api_key: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .arg("--state-dir")
        .arg(state)
        .arg("--json");
    if let Some(api_key) = api_key {
        command.env("OPENAI_API_KEY", api_key);
    }
    command.args(arguments).output().expect("run forge-runtime")
}

fn complete_analysis(state: &Path, analysis_id: &str, dispatch_id: &str, answer: &str, at: u64) {
    let store = SqliteHubStore::open(state.join("hub.sqlite3")).expect("open Hub");
    let claimed = store
        .claim_group_model_analysis_dispatch(&ClaimGroupModelAnalysisDispatch {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            analysis_id: analysis_id.into(),
            dispatch_id: dispatch_id.into(),
            consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
            released_at_ms: at - 1,
        })
        .expect("claim analysis");
    assert!(matches!(
        claimed,
        ClaimGroupModelAnalysisDispatchResult::Claimed { .. }
    ));
    let inspection = store
        .inspect_group_model_analysis(analysis_id)
        .expect("inspect claimed analysis");
    store
        .complete_group_model_analysis(&CompleteGroupModelAnalysis {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            artifact: result_artifact(&inspection, answer, at),
        })
        .expect("complete analysis");
}

fn result_artifact(
    inspection: &forge_runtime_domain::GroupModelAnalysisInspection,
    answer: &str,
    created_at_ms: u64,
) -> GroupModelAnalysisResultArtifact {
    let result = GroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_RESULT_VERSION,
        analysis_id: inspection.analysis.analysis_id.clone(),
        dispatch_id: inspection
            .dispatch
            .as_ref()
            .expect("dispatch")
            .dispatch_id
            .clone(),
        request_sha256: inspection.analysis.request_sha256.clone(),
        outcome: GroupModelAnalysisOutcome::Completed,
        answer: answer.into(),
        usage: Usage {
            input_tokens: 11,
            output_tokens: 7,
        },
    };
    let bytes = canonical_json_bytes(&result);
    GroupModelAnalysisResultArtifact {
        result,
        result_sha256: digest(GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, &bytes),
        result_bytes: bytes.len(),
        created_at_ms,
    }
}

fn prepare_panel(state: &Path, cwd: &Path, group_run_id: &str, ids: [&str; 2]) -> String {
    run_json(
        state,
        cwd,
        &[
            "group",
            "panel",
            "prepare",
            group_run_id,
            "--analysis",
            ids[0],
            "--analysis",
            ids[1],
            "--idempotency-key",
            "panel-key",
        ],
    )["panel"]["panel"]["panel_id"]
        .as_str()
        .expect("panel ID")
        .into()
}

fn prepare_analysis(state: &Path, cwd: &Path, group_run_id: &str, key: &str) -> String {
    run_json(
        state,
        cwd,
        &[
            "group",
            "analysis",
            "prepare",
            group_run_id,
            "--model",
            "test-model",
            "--max-output-tokens",
            "1024",
            "--idempotency-key",
            key,
        ],
    )["inspection"]["analysis"]["analysis_id"]
        .as_str()
        .expect("analysis ID")
        .into()
}

fn create_group(state: &Path, cwd: &Path) -> String {
    run_json(state, cwd, &["group", "create", "Frontend backend SSO"])["group"]["id"]
        .as_str()
        .expect("group ID")
        .into()
}

fn add_project(state: &Path, cwd: &Path, group_id: &str, project: &Path) {
    run_json(
        state,
        cwd,
        &[
            "group",
            "add",
            group_id,
            project.to_str().expect("UTF-8 path"),
            "--role",
            "sso",
        ],
    );
}

fn create_session(state: &Path, cwd: &Path, group_id: &str) -> String {
    run_json(
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
    )["session"]["id"]
        .as_str()
        .expect("session ID")
        .into()
}

fn add_prompt(state: &Path, cwd: &Path, session_id: &str, content: &str) {
    run_json(state, cwd, &["prompt", "add", session_id, content]);
}

fn prepare_group_run(state: &Path, cwd: &Path, group_id: &str) -> String {
    run_json(
        state,
        cwd,
        &[
            "group",
            "run",
            "prepare",
            group_id,
            "--idempotency-key",
            "frozen-group-run",
        ],
    )["snapshot"]["run"]["run_id"]
        .as_str()
        .expect("Group Run ID")
        .into()
}

fn canonical_json_bytes(value: &impl Serialize) -> Vec<u8> {
    let value = serde_json::to_value(value).expect("JSON value");
    serde_json::to_vec(&sort_json(value)).expect("canonical JSON")
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => {
            let sorted = items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        other => other,
    }
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}
