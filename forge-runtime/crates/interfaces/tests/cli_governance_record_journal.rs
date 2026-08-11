use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::Duration,
};

use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

const GOLDEN: &str =
    include_str!("../../../../docs/contracts/fixtures/governance-evidence-claim-v1.json");
const RESTORE_HISTORICAL_ANALYSES_SQL: &str =
    include_str!("../../infrastructure/tests/restore_historical_analyses.sql");

fn invoke(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .output()
        .expect("run forge-runtime")
}

fn invoke_stdin(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge-runtime");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for forge-runtime")
}

fn json(output: Output) -> Value {
    let Output {
        status,
        stdout,
        stderr,
    } = output;
    assert!(
        status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&stderr)
    );
    serde_json::from_slice(&stdout).expect("valid JSON output")
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("test path is UTF-8")
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

fn canonical_records() -> Vec<String> {
    let fixture: Value = serde_json::from_str(GOLDEN).expect("golden fixture");
    fixture["records"]
        .as_array()
        .expect("record fixtures")
        .iter()
        .map(|record| {
            record["expected"]["canonical_record_json"]
                .as_str()
                .expect("canonical record")
                .to_owned()
        })
        .collect()
}

fn record_set(records: &[String]) -> String {
    format!("[{}]", records.join(","))
}

fn write_set(root: &TempDir, contents: &str) -> PathBuf {
    let path = root.path().join("records.json");
    fs::write(&path, contents).expect("write record-set fixture");
    path
}

fn append_file(state: &TempDir, key: &str, file: &Path) -> Output {
    invoke(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "--idempotency-key",
        key,
        "governance",
        "journal",
        "append",
        "--file",
        path_text(file),
    ])
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

fn assert_keys(value: &Value, expected: &[&str]) {
    let actual = value
        .as_object()
        .expect("JSON object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected.iter().copied().collect());
}

#[test]
fn invalid_append_preflight_never_creates_a_database() {
    for invalid in [
        "not-json".to_owned(),
        "[]".to_owned(),
        "x".repeat(1_048_577),
    ] {
        let state = private_state();
        let file = write_set(&state, &invalid);
        let output = append_file(&state, "invalid-key", &file);
        assert!(!output.status.success(), "invalid append must fail");
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

#[test]
fn missing_key_fails_before_the_record_set_path_is_opened() {
    let state = private_state();
    let missing = state.path().join("does-not-exist.json");
    let output = invoke(&[
        "--state-dir",
        path_text(state.path()),
        "governance",
        "journal",
        "append",
        "--file",
        path_text(&missing),
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--idempotency-key"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn invalid_key_fails_before_the_record_set_path_is_opened() {
    let state = private_state();
    let missing = state.path().join("does-not-exist.json");
    let output = append_file(&state, "unsafe\u{202e}key", &missing);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("idempotency key is invalid"), "{error}");
    assert!(!error.contains("No such file"), "{error}");
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn invalid_key_preflight_does_not_wait_for_stdin() {
    let state = private_state();
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--state-dir",
            path_text(state.path()),
            "--idempotency-key",
            "unsafe\u{202e}key",
            "governance",
            "journal",
            "append",
            "--file",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge-runtime with open stdin");
    let exited = (0..100).any(|_| {
        thread::sleep(Duration::from_millis(10));
        child
            .try_wait()
            .expect("poll invalid-key command")
            .is_some()
    });
    if !exited {
        child.kill().expect("kill blocked command");
        panic!("invalid idempotency key blocked while waiting for stdin");
    }
    let output = child.wait_with_output().expect("collect preflight failure");
    assert!(String::from_utf8_lossy(&output.stderr).contains("idempotency key is invalid"));
}

#[test]
fn append_rejects_non_regular_paths_without_creating_a_database() {
    let state = private_state();
    let output = append_file(&state, "directory-key", state.path());
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("regular non-symlink"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[cfg(unix)]
#[test]
fn append_rejects_symlink_paths_without_creating_a_database() {
    use std::os::unix::fs::symlink;

    let state = private_state();
    let target = write_set(&state, &record_set(&canonical_records()));
    let link = state.path().join("linked-records.json");
    symlink(target, &link).expect("create symlink");
    let output = append_file(&state, "symlink-key", &link);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("regular non-symlink"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn missing_database_reads_fail_without_creating_state() {
    let state = private_state();
    let output = journal_read(&state, &["list"]);
    assert!(!output.status.success());
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn malformed_read_inputs_fail_before_the_database_is_opened() {
    let state = private_state();
    for suffix in [
        vec!["show", "bad\nid"],
        vec!["list", "--aggregate-id", ""],
        vec!["head", "EvidenceRecord", "bad\nid"],
        vec![
            "view",
            "KnowledgeClaim",
            "bad\nid",
            "--as-of-unix-ms",
            "1700000002000",
        ],
    ] {
        let output = journal_read(&state, &suffix);
        assert!(!output.status.success(), "invalid read must fail");
        assert!(String::from_utf8_lossy(&output.stderr).contains("input is invalid"));
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

#[test]
fn append_replay_and_conflict_preserve_exact_batch_identity() {
    let state = private_state();
    let records = canonical_records();
    let exact_set = record_set(&records);
    let file = write_set(&state, &exact_set);
    let stored = json(append_file(&state, "stable-key", &file));
    assert_receipt(&stored, "stored");

    let replay = json(invoke_stdin(
        &[
            "--state-dir",
            path_text(state.path()),
            "--json",
            "--idempotency-key",
            "stable-key",
            "governance",
            "journal",
            "append",
            "--file",
            "-",
        ],
        &exact_set,
    ));
    assert_receipt(&replay, "exact_replay");
    assert_eq!(replay["batch_id"], stored["batch_id"]);
    assert_eq!(replay["appended_at_unix_ms"], stored["appended_at_unix_ms"]);

    let changed = write_set(&state, &record_set(&records[..1]));
    assert!(!append_file(&state, "stable-key", &changed).status.success());
    assert_eq!(
        json(journal_read(&state, &["list"]))["records"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn reads_map_only_public_fields_and_require_explicit_record_reveal() {
    let state = private_state();
    let records = canonical_records();
    let file = write_set(&state, &record_set(&records));
    json(append_file(&state, "read-key", &file));

    let hidden = json(journal_read(&state, &["show", "evr-0001"]));
    assert_inspection(&hidden, false);
    let revealed = json(journal_read(
        &state,
        &["show", "evr-0001", "--include-record"],
    ));
    assert_inspection(&revealed, true);
    assert_eq!(revealed["canonical_record_json"], records[0]);

    let list = json(journal_read(&state, &["list"]));
    assert_keys(&list, &["api_version", "kind", "records"]);
    assert_eq!(list["kind"], "GovernanceRecordInspectionList");
    assert!(
        list["records"]
            .as_array()
            .is_some_and(|items| items.len() == 2)
    );
    assert!(
        list["records"]
            .as_array()
            .expect("records")
            .iter()
            .all(|item| item.get("canonical_record_json").is_none())
    );
    let filtered = json(journal_read(
        &state,
        &[
            "list",
            "--kind",
            "KnowledgeClaim",
            "--aggregate-id",
            "claim-harness-check",
        ],
    ));
    assert_eq!(filtered["records"].as_array().map(Vec::len), Some(1));
    assert_eq!(filtered["records"][0]["record_id"], "kcr-0001");
}

#[test]
fn structural_head_is_explicitly_sequence_only() {
    let state = private_state();
    let file = write_set(&state, &record_set(&canonical_records()));
    json(append_file(&state, "head-key", &file));
    let head = json(journal_read(
        &state,
        &["head", "EvidenceRecord", "evidence-check-pass"],
    ));
    assert_keys(
        &head,
        &[
            "aggregate_id",
            "api_version",
            "canonical_sha256",
            "interpretation",
            "kind",
            "record_id",
            "record_kind",
            "sequence",
            "updated_at_unix_ms",
        ],
    );
    assert_eq!(head["kind"], "GovernanceStructuralHead");
    assert_eq!(head["interpretation"], "structural_sequence_only");
    assert_eq!(head["record_id"], "evr-0001");
}

#[test]
fn reads_refuse_v24_without_migration_but_append_migrates_it_to_current() {
    let state = private_state();
    json(invoke(&["--state-dir", path_text(state.path()), "--json"]));
    let database = state.path().join("hub.sqlite3");
    downgrade_empty_current_to_v24(&database);

    assert!(!journal_read(&state, &["list"]).status.success());
    assert_eq!(stored_version(&database), 24);
    let file = write_set(&state, &record_set(&canonical_records()));
    assert_receipt(&json(append_file(&state, "migration-key", &file)), "stored");
    assert_eq!(stored_version(&database), 27);
}

fn assert_receipt(value: &Value, disposition: &str) {
    assert_keys(
        value,
        &[
            "api_version",
            "appended_at_unix_ms",
            "batch_id",
            "disposition",
            "kind",
            "record_count",
            "record_ids",
            "record_set_sha256",
            "request_sha256",
        ],
    );
    assert_eq!(value["api_version"], "forgeos.governance-journal/v1");
    assert_eq!(value["kind"], "GovernanceRecordAppendReceipt");
    assert_eq!(value["disposition"], disposition);
    assert_eq!(value["record_count"], 2);
    assert_eq!(
        value["record_ids"],
        serde_json::json!(["evr-0001", "kcr-0001"])
    );
}

fn assert_inspection(value: &Value, includes_record: bool) {
    let mut fields = vec![
        "aggregate_id",
        "api_version",
        "appended_at_unix_ms",
        "batch_id",
        "batch_ordinal",
        "canonical_record_bytes",
        "canonical_sha256",
        "created_at_unix_ms",
        "kind",
        "record_id",
        "record_kind",
        "sequence",
    ];
    if includes_record {
        fields.push("canonical_record_json");
    }
    assert_keys(value, &fields);
    assert_eq!(value["kind"], "GovernanceRecordInspection");
    assert_eq!(value["record_id"], "evr-0001");
}

fn downgrade_empty_current_to_v24(database: &Path) {
    let connection = Connection::open(database).expect("open database");
    connection
        .execute_batch(RESTORE_HISTORICAL_ANALYSES_SQL)
        .expect("restore v24 endpoint definitions");
    connection
        .execute_batch(
            "DROP TABLE governance_claim_validation_jobs;
             DROP TABLE governance_claim_semantic_views;
             DROP TABLE governance_semantic_heads;
             DROP TABLE governance_structural_heads;
             DROP TABLE governance_records;
             DROP TABLE governance_record_append_batches;
             PRAGMA user_version = 24;",
        )
        .expect("downgrade empty additive schema");
}

fn stored_version(database: &Path) -> i64 {
    Connection::open(database)
        .expect("open database")
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("stored version")
}
