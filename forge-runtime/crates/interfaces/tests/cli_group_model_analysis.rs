use std::{fs, path::Path, process::Command};

use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatch, ClaimGroupModelAnalysisDispatchResult,
    GROUP_MODEL_ANALYSIS_CONSENT_VERSION, GROUP_MODEL_ANALYSIS_VERSION, GroupModelAnalysisStore,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

const FROZEN_PROMPT: &str = "frontend, backend, and SSO share an issuer contract";
const LATER_PROMPT: &str = "later prompt outside the frozen dossier";
const ANALYSIS_KEY: &str = "stable-analysis-key";

fn invoke(state: &Path, cwd: &Path, json: bool, arguments: &[&str]) -> std::process::Output {
    invoke_with_api_key(state, cwd, json, arguments, None)
}

fn invoke_with_api_key(
    state: &Path,
    cwd: &Path,
    json: bool,
    arguments: &[&str],
    api_key: Option<&str>,
) -> std::process::Output {
    invoke_with_openai_env(state, cwd, json, arguments, api_key, None)
}

fn invoke_with_openai_env(
    state: &Path,
    cwd: &Path,
    json: bool,
    arguments: &[&str],
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .arg("--state-dir")
        .arg(state);
    if let Some(api_key) = api_key {
        command.env("OPENAI_API_KEY", api_key);
    }
    if let Some(base_url) = base_url {
        command.env("OPENAI_BASE_URL", base_url);
    }
    if json {
        command.arg("--json");
    }
    command.args(arguments).output().expect("run forge-runtime")
}

fn run_json(state: &Path, cwd: &Path, arguments: &[&str]) -> Value {
    parse_json_output(&invoke(state, cwd, true, arguments))
}

fn run_json_with_base_url(state: &Path, cwd: &Path, arguments: &[&str], base_url: &str) -> Value {
    parse_json_output(&invoke_with_openai_env(
        state,
        cwd,
        true,
        arguments,
        None,
        Some(base_url),
    ))
}

fn parse_json_output(output: &std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI JSON")
}

struct Fixture {
    state: TempDir,
    project: TempDir,
    cwd: TempDir,
    session_id: String,
    group_run_id: String,
}

impl Fixture {
    fn new() -> Self {
        let state = TempDir::new().expect("state directory");
        let project = TempDir::new().expect("project directory");
        let cwd = TempDir::new().expect("unrelated working directory");
        fs::write(
            project.path().join("private.txt"),
            "workspace must not be read",
        )
        .expect("workspace fixture");
        let group_id = create_group(state.path(), cwd.path());
        add_project(state.path(), cwd.path(), &group_id, project.path());
        let session_id = create_session(state.path(), cwd.path(), &group_id);
        add_prompt(state.path(), cwd.path(), &session_id, FROZEN_PROMPT);
        let group_run_id = prepare_group_run(state.path(), cwd.path(), &group_id);
        Self {
            state,
            project,
            cwd,
            session_id,
            group_run_id,
        }
    }

    fn prepare_analysis(&self) -> Value {
        run_json(
            self.state.path(),
            self.cwd.path(),
            &[
                "group",
                "analysis",
                "prepare",
                &self.group_run_id,
                "--model",
                "test-model",
                "--max-output-tokens",
                "1024",
                "--idempotency-key",
                ANALYSIS_KEY,
            ],
        )
    }

    fn show(&self, analysis_id: &str) -> Value {
        run_json(
            self.state.path(),
            self.cwd.path(),
            &["group", "analysis", "show", analysis_id],
        )
    }
}

#[test]
fn prepare_is_local_exact_idempotent_and_private_by_default() {
    let fixture = Fixture::new();
    let first = fixture.prepare_analysis();
    assert_prepared(&first, "created", &fixture.group_run_id);
    add_prompt(
        fixture.state.path(),
        fixture.cwd.path(),
        &fixture.session_id,
        LATER_PROMPT,
    );
    fs::remove_dir_all(fixture.project.path()).expect("remove source workspace");

    let replay = fixture.prepare_analysis();
    assert_prepared(&replay, "replayed", &fixture.group_run_id);
    assert_eq!(replay["inspection"], first["inspection"]);
    let analysis_id = analysis_id(&first);
    assert_eq!(fixture.show(analysis_id)["inspection"], first["inspection"]);
    assert_list(&fixture, analysis_id);
    assert_request_contract(&fixture, analysis_id);
    assert_private(&first);
    assert_private(&replay);
}

#[test]
fn prepare_normalizes_a_trailing_slash_self_hosted_base() {
    let fixture = Fixture::new();
    let prepared = run_json_with_base_url(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "analysis",
            "prepare",
            &fixture.group_run_id,
            "--model",
            "test-model",
            "--max-output-tokens",
            "1024",
            "--idempotency-key",
            "trailing-base-analysis-key",
        ],
        "https://llm.internal.example/v1/",
    );

    assert_eq!(
        prepared["inspection"]["analysis"]["config"]["endpoint"],
        "https://llm.internal.example/v1/responses"
    );
}

#[test]
fn consent_and_credentials_fail_before_the_dispatch_claim() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare_analysis();
    let analysis_id = analysis_id(&prepared);

    let no_consent = invoke(
        fixture.state.path(),
        fixture.cwd.path(),
        true,
        &["group", "analysis", "send", analysis_id],
    );
    assert!(!no_consent.status.success());
    assert!(String::from_utf8_lossy(&no_consent.stderr).contains("--confirm-off-machine"));
    assert_eq!(
        fixture.show(analysis_id)["inspection"]["analysis"]["status"],
        "awaiting_consent"
    );

    let no_key = invoke(
        fixture.state.path(),
        fixture.cwd.path(),
        true,
        &[
            "group",
            "analysis",
            "send",
            analysis_id,
            "--confirm-off-machine",
        ],
    );
    assert!(!no_key.status.success());
    assert!(String::from_utf8_lossy(&no_key.stderr).contains("OPENAI_API_KEY"));
    assert_eq!(
        fixture.show(analysis_id)["inspection"]["analysis"]["status"],
        "awaiting_consent"
    );
}

#[test]
fn a_header_unsafe_credential_fails_before_the_dispatch_claim() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare_analysis();
    let analysis_id = analysis_id(&prepared);

    let malformed_key = invoke_with_api_key(
        fixture.state.path(),
        fixture.cwd.path(),
        true,
        &[
            "group",
            "analysis",
            "send",
            analysis_id,
            "--confirm-off-machine",
        ],
        Some("credential\nmust-not-enter-a-header"),
    );

    assert!(!malformed_key.status.success());
    assert!(!String::from_utf8_lossy(&malformed_key.stderr).contains("must-not-enter-a-header"));
    let inspection = fixture.show(analysis_id);
    assert_eq!(
        inspection["inspection"]["analysis"]["status"],
        "awaiting_consent"
    );
    assert!(inspection["inspection"]["dispatch"].is_null());

    let connection =
        Connection::open(fixture.state.path().join("hub.sqlite3")).expect("open SQLite");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM group_model_analysis_events WHERE analysis_id=?1",
            [analysis_id],
            |row| row.get(0),
        )
        .expect("count analysis events");
    assert_eq!(event_count, 1);
}

#[test]
fn an_existing_dispatch_claim_is_never_sent_again() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare_analysis();
    let analysis_id = analysis_id(&prepared);
    let store = SqliteHubStore::open(fixture.state.path().join("hub.sqlite3")).expect("open store");
    let claimed = store
        .claim_group_model_analysis_dispatch(&ClaimGroupModelAnalysisDispatch {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            analysis_id: analysis_id.into(),
            dispatch_id: "simulated-crash-dispatch".into(),
            consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
            released_at_ms: 9_999_999_999_999,
        })
        .expect("commit claim before simulated crash");
    assert!(matches!(
        claimed,
        ClaimGroupModelAnalysisDispatchResult::Claimed { .. }
    ));

    let recovered = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "analysis", "send", analysis_id],
    );
    assert_eq!(recovered["disposition"], "already_claimed");
    assert_eq!(
        recovered["inspection"]["recovery"]["status"],
        "dispatch_unknown"
    );
    assert_eq!(
        recovered["inspection"]["dispatch"]["dispatch_id"],
        "simulated-crash-dispatch"
    );
    assert_private(&recovered);
}

fn assert_prepared(output: &Value, disposition: &str, group_run_id: &str) {
    assert_eq!(output["type"], "group_model_analysis_prepared");
    assert_eq!(output["disposition"], disposition);
    assert_eq!(
        output["inspection"]["analysis"]["group_run_id"],
        group_run_id
    );
    assert_eq!(
        output["inspection"]["analysis"]["status"],
        "awaiting_consent"
    );
    assert_eq!(
        output["inspection"]["recovery"]["status"],
        "awaiting_consent"
    );
    assert_eq!(
        output["inspection"]["analysis"]["config"]["model"],
        "test-model"
    );
}

fn assert_list(fixture: &Fixture, analysis_id: &str) {
    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "analysis",
            "list",
            &fixture.group_run_id,
            "--limit",
            "5",
        ],
    );
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["source_and_journal_validated"], false);
    assert_eq!(listed["inspect_with"], "group analysis show ANALYSIS_ID");
    assert_eq!(listed["analyses"][0]["analysis_id"], analysis_id);
    assert_private(&listed);
}

fn assert_request_contract(fixture: &Fixture, analysis_id: &str) {
    let connection =
        Connection::open(fixture.state.path().join("hub.sqlite3")).expect("open SQLite");
    let (body, config): (Vec<u8>, String) = connection
        .query_row(
            "SELECT request_body,config_json FROM group_model_analyses WHERE id=?1",
            [analysis_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored preparation");
    let body: Value = serde_json::from_slice(&body).expect("request JSON");
    assert_eq!(body["model"], "test-model");
    assert_eq!(body["tools"], serde_json::json!([]));
    assert_eq!(body["store"], false);
    assert_eq!(body["stream"], true);
    assert!(
        body["input"][0]["content"]
            .as_str()
            .is_some_and(|context| context.contains(FROZEN_PROMPT))
    );
    assert!(config.contains("untrusted context"));
}

fn assert_private(output: &Value) {
    let json = output.to_string();
    for forbidden in [
        FROZEN_PROMPT,
        LATER_PROMPT,
        ANALYSIS_KEY,
        "workspace must not be read",
        "untrusted context",
        "\"events\"",
        "\"request_body\"",
        "\"config_json\"",
        "\"result\"",
    ] {
        assert!(!json.contains(forbidden), "output leaked {forbidden}");
    }
}

fn analysis_id(output: &Value) -> &str {
    output["inspection"]["analysis"]["analysis_id"]
        .as_str()
        .expect("analysis ID")
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
