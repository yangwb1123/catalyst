use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::{Command, Output},
};

use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatch, ClaimGroupModelAnalysisDispatchResult,
    CompleteGroupModelAnalysis, GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
    GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_RESULT_VERSION,
    GROUP_MODEL_ANALYSIS_VERSION, GroupModelAnalysisOutcome, GroupModelAnalysisResult,
    GroupModelAnalysisResultArtifact, GroupModelAnalysisStore, Usage,
};
use forge_runtime_infrastructure::SqliteHubStore;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const FROZEN_PROMPT: &str = "frontend, backend, and SSO must share one issuer contract";
const LATER_PROMPT: &str = "an unrelated prompt added after panel preparation";
const WORKSPACE_SECRET: &str = "workspace content must never enter panel output";
const PANEL_KEY: &str = "stable-panel-idempotency-key";
const FIRST_ANSWER: &str = "frontend preserves the shared issuer.";
const SECOND_ANSWER: &str = "backend and SSO validate the same issuer.";

struct Fixture {
    state: TempDir,
    project: TempDir,
    cwd: TempDir,
    session_id: String,
    group_run_id: String,
    analysis_ids: [String; 2],
}

impl Fixture {
    fn new() -> Self {
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
        Self {
            state,
            project,
            cwd,
            session_id,
            group_run_id,
            analysis_ids: [first, second],
        }
    }

    fn prepare_panel(&self) -> Value {
        run_json(
            self.state.path(),
            self.cwd.path(),
            &[
                "group",
                "panel",
                "prepare",
                &self.group_run_id,
                "--analysis",
                &self.analysis_ids[0],
                "--analysis",
                &self.analysis_ids[1],
                "--idempotency-key",
                PANEL_KEY,
            ],
        )
    }
}

#[test]
fn panel_cli_is_ordered_redacted_metadata_only_and_exactly_replayable() {
    let fixture = Fixture::new();
    let created = fixture.prepare_panel();
    assert_eq!(created["type"], "group_analysis_panel_prepared");
    assert_eq!(created["disposition"], "created");
    assert_order_and_redaction(&created["panel"], &fixture.analysis_ids);
    assert_private(&created, &fixture, false);

    let panel_id = created["panel"]["panel"]["panel_id"]
        .as_str()
        .expect("panel ID");
    let shown = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "panel", "show", panel_id],
    );
    assert_order_and_redaction(&shown["panel"], &fixture.analysis_ids);
    assert_private(&shown, &fixture, false);
    assert_included_results(&fixture, panel_id);
    assert_metadata_list(&fixture, panel_id);

    add_prompt(
        fixture.state.path(),
        fixture.cwd.path(),
        &fixture.session_id,
        LATER_PROMPT,
    );
    let replayed = fixture.prepare_panel();
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(replayed["panel"], created["panel"]);
    assert_private(&replayed, &fixture, false);
}

#[test]
fn panel_cli_rejects_duplicate_and_too_few_analysis_ids() {
    let fixture = Fixture::new();
    let duplicate = invoke(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "panel",
            "prepare",
            &fixture.group_run_id,
            "--analysis",
            &fixture.analysis_ids[0],
            "--analysis",
            &fixture.analysis_ids[0],
        ],
    );
    assert_rejected(&duplicate, "does not allow duplicate analysis IDs");
    let too_few = invoke(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "panel",
            "prepare",
            &fixture.group_run_id,
            "--analysis",
            &fixture.analysis_ids[0],
        ],
    );
    assert_rejected(&too_few, "requires between 2 and 8 --analysis values");
}

fn assert_included_results(fixture: &Fixture, panel_id: &str) {
    let included = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "panel", "show", panel_id, "--include-results"],
    );
    let contributions = included["panel"]["contributions"]
        .as_array()
        .expect("panel contributions");
    assert_eq!(included["panel"]["assembly_only"], true);
    assert_eq!(included["panel"]["synthesis_performed"], false);
    assert_eq!(included["panel"]["results_included"], true);
    assert_eq!(contributions[0]["result"]["answer"], FIRST_ANSWER);
    assert_eq!(contributions[1]["result"]["answer"], SECOND_ANSWER);
    assert_private(&included, fixture, true);
}

fn assert_metadata_list(fixture: &Fixture, panel_id: &str) {
    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "panel",
            "list",
            &fixture.group_run_id,
            "--limit",
            "5",
        ],
    );
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["source_and_results_validated"], false);
    assert_eq!(listed["inspect_with"], "group panel show PANEL_ID");
    assert_eq!(listed["panels"][0]["panel_id"], panel_id);
    assert!(listed["panels"][0].get("manifest").is_none());
    assert_private(&listed, fixture, false);
}

fn assert_order_and_redaction(panel: &Value, ids: &[String; 2]) {
    assert_eq!(panel["assembly_only"], true);
    assert_eq!(panel["synthesis_performed"], false);
    assert_eq!(panel["results_included"], false);
    let contributions = panel["contributions"]
        .as_array()
        .expect("panel contributions");
    assert_eq!(contributions.len(), 2);
    assert_eq!(contributions[0]["analysis"]["analysis_id"], ids[0]);
    assert_eq!(contributions[1]["analysis"]["analysis_id"], ids[1]);
    assert_eq!(contributions[0]["outcome"], "completed");
    assert_eq!(contributions[1]["outcome"], "completed");
    assert!(
        contributions
            .iter()
            .all(|item| item.get("result").is_none())
    );
}

fn assert_private(output: &Value, fixture: &Fixture, answers_allowed: bool) {
    let encoded = output.to_string();
    for forbidden in [
        FROZEN_PROMPT,
        LATER_PROMPT,
        WORKSPACE_SECRET,
        PANEL_KEY,
        "analysis-key-a",
        "analysis-key-b",
        "private.txt",
    ] {
        assert!(!encoded.contains(forbidden), "output leaked {forbidden}");
    }
    let workspace = fixture.project.path().to_string_lossy();
    assert!(
        !encoded.contains(workspace.as_ref()),
        "output leaked workspace"
    );
    if !answers_allowed {
        assert!(!encoded.contains(FIRST_ANSWER));
        assert!(!encoded.contains(SECOND_ANSWER));
    }
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
        .expect("claim analysis locally");
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
        .expect("complete analysis locally");
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

fn invoke(state: &Path, cwd: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .arg("--state-dir")
        .arg(state)
        .arg("--json")
        .args(arguments)
        .output()
        .expect("run forge-runtime")
}

fn run_json(state: &Path, cwd: &Path, arguments: &[&str]) -> Value {
    let output = invoke(state, cwd, arguments);
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI JSON")
}

fn assert_rejected(output: &Output, message: &str) {
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    assert!(output.stdout.is_empty());
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
