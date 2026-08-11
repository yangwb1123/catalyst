use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    process::{Command, Output},
};

use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

const GOLDEN: &str =
    include_str!("../../../../docs/contracts/fixtures/governance-evidence-claim-v1.json");

#[test]
fn semantic_reads_are_explicit_time_projection_only() {
    let state = initialized_semantic_state();
    let view = json(&journal_read(
        &state,
        &[
            "view",
            "KnowledgeClaim",
            "claim-harness-check",
            "--as-of-unix-ms",
            "1700000002000",
        ],
    ));
    assert_keys(
        &view,
        &[
            "api_version",
            "evaluated_at_unix_ms",
            "interpretation",
            "kind",
            "projection",
            "semantic_view_version",
            "temporal_state",
        ],
    );
    assert_eq!(view["api_version"], "forgeos.governance-semantic-view/v1");
    assert_eq!(view["kind"], "GovernanceSemanticAssessment");
    assert_eq!(
        view["interpretation"],
        "semantic_projection_only_no_truth_or_authority"
    );
    assert_eq!(view["temporal_state"], "fresh");
    assert_eq!(view["projection"]["head"]["record_id"], "kcr-0001");
    assert_empty_semantic_lists(&state);
}

#[test]
fn semantic_read_uses_live_v27_snapshot_while_ordinary_read_stays_immutable() {
    let state = initialized_semantic_state();
    let database = state.path().join("hub.sqlite3");
    let writer = hot_writer(&database);
    writer
        .execute(
            "INSERT INTO groups(id,name,idempotency_key,created_at_ms) VALUES(?1,?2,?3,?4)",
            ("cli-hot", "CLI hot WAL", "cli-hot-key", 1_i64),
        )
        .expect("commit unrelated row into hot WAL");

    assert!(!journal_read(&state, &["list"]).status.success());
    let view = json(&journal_read(
        &state,
        &[
            "view",
            "KnowledgeClaim",
            "claim-harness-check",
            "--as-of-unix-ms",
            "1700000002000",
        ],
    ));
    assert_eq!(view["projection"]["head"]["record_id"], "kcr-0001");
    drop(writer);
}

#[test]
fn semantic_read_without_explicit_time_fails_before_database_open() {
    let state = private_state();
    let output = journal_read(&state, &["view", "KnowledgeClaim", "claim-harness-check"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--as-of-unix-ms"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn assert_empty_semantic_lists(state: &TempDir) {
    let conflicts = json(&journal_read(
        state,
        &["conflicts", "--as-of-unix-ms", "1700000002000"],
    ));
    assert_eq!(conflicts["kind"], "GovernanceClaimConflictList");
    assert_eq!(conflicts["conflicts"], serde_json::json!([]));
    let jobs = json(&journal_read(
        state,
        &[
            "validation-jobs",
            "--as-of-unix-ms",
            "1700000002000",
            "--due-only",
        ],
    ));
    assert_eq!(jobs["kind"], "GovernanceValidationJobList");
    assert_eq!(jobs["jobs"], serde_json::json!([]));
}

fn initialized_semantic_state() -> TempDir {
    let state = private_state();
    let path = state.path().join("records.json");
    let fixture: Value = serde_json::from_str(GOLDEN).expect("golden fixture");
    let records = fixture["records"].as_array().expect("record fixtures");
    let canonical: Vec<_> = records
        .iter()
        .map(|record| {
            record["expected"]["canonical_record_json"]
                .as_str()
                .expect("canonical")
        })
        .collect();
    fs::write(&path, format!("[{}]", canonical.join(","))).expect("write record set");
    json(&invoke(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "--idempotency-key",
        "semantic-key",
        "governance",
        "journal",
        "append",
        "--file",
        path_text(&path),
    ]));
    state
}

fn hot_writer(database: &Path) -> Connection {
    let writer = Connection::open(database).expect("open WAL writer");
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA wal_autocheckpoint=0;")
        .expect("prepare hot WAL");
    writer
}

fn journal_read(state: &TempDir, suffix: &[&str]) -> Output {
    let mut arguments = vec![
        "--state-dir",
        path_text(state.path()),
        "--json",
        "governance",
        "journal",
    ];
    arguments.extend_from_slice(suffix);
    invoke(&arguments)
}

fn invoke(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .output()
        .expect("run forge-runtime")
}

fn json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON output")
}

fn private_state() -> TempDir {
    let state = TempDir::new().expect("state directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(state.path(), fs::Permissions::from_mode(0o700))
            .expect("secure state directory");
    }
    state
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("test path is UTF-8")
}

fn assert_keys(value: &Value, expected: &[&str]) {
    let actual = value
        .as_object()
        .expect("JSON object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected.iter().copied().collect());
}
