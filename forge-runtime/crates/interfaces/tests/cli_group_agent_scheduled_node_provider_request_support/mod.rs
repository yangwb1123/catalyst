use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use rusqlite::{Connection, types::Value as SqlValue};
use tempfile::TempDir;

use super::{
    group_agent_graph_run_support::{Fixture, command, invoke_with_stdin},
    group_agent_graph_support::{successful_json, text},
};

pub(super) fn prepare_provider_request(fixture: &Fixture, contract_id: &str, key: &str) -> String {
    let output = scheduled_provider_request_command(
        fixture,
        &["prepare", contract_id, "--idempotency-key", key],
    )
    .output()
    .expect("prepare scheduled provider request");
    text(&successful_json(&output)["inspection"]["record"]["provider_request_id"])
}

pub(super) fn export_scheduled_release_control(
    fixture: &Fixture,
    provider_request_id: &str,
) -> Vec<u8> {
    let output = scheduled_provider_request_command(
        fixture,
        &["release-control", "export", provider_request_id],
    )
    .output()
    .expect("export scheduled release control");
    assert_provider_request_success(&output);
    output.stdout
}

pub(super) fn authorize_scheduled_with_core(control: &[u8]) -> Vec<u8> {
    let mut child = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .args([
            "run",
            "./cmd/forge",
            "graph-scheduled-node-dispatch-authorize",
            "--control",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn scheduled authorization Core");
    child.stdin.take().unwrap().write_all(control).unwrap();
    let output = child.wait_with_output().expect("wait for Core");
    assert_provider_request_success(&output);
    assert!(!output.stdout.ends_with(b"\n"));
    output.stdout
}

pub(super) fn scheduled_provider_request_command(fixture: &Fixture, tail: &[&str]) -> Command {
    let mut args = vec![
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "provider-request",
    ];
    args.extend_from_slice(tail);
    command(fixture.state.path(), fixture.cwd.path(), &args)
}

pub(super) fn readiness_prefix() -> Vec<&'static str> {
    vec![
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "provider-request",
        "readiness",
    ]
}

pub(super) fn readiness_tail<'a>(
    request: &'a str,
    authorization: &'a str,
    pricing: &'a str,
) -> Vec<&'a str> {
    let mut args = readiness_prefix();
    args.extend([
        "verify",
        request,
        "--authorization",
        authorization,
        "--pricing",
        pricing,
    ]);
    args
}

pub(super) fn stored_body(state: &Path, request_id: &str) -> Vec<u8> {
    Connection::open(state.join("hub.sqlite3"))
        .expect("open Hub")
        .query_row(
            "SELECT provider_request_blob
             FROM group_agent_graph_scheduled_node_provider_requests
             WHERE id=?1",
            [request_id],
            |row| row.get(0),
        )
        .expect("stored exact provider request body")
}

pub(super) fn downgrade_empty_v15_sidecar(database: &Path) {
    let connection = Connection::open(database).expect("open current Hub for v14 fixture");
    connection
        .execute_batch(
            "DROP INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active;
             DROP INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_created;
             DROP TABLE group_agent_graph_scheduled_node_dispatch_lifecycles;
             DROP TABLE group_agent_graph_scheduled_node_provider_requests;
             PRAGMA user_version = 14;
             PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .expect("construct exact v14 fixture");
    assert!(!provider_request_table_exists(&connection));
}

pub(super) fn provider_request_table_exists(connection: &Connection) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_schema
               WHERE type='table'
                 AND name='group_agent_graph_scheduled_node_provider_requests'
             )",
            [],
            |row| row.get(0),
        )
        .expect("provider request table presence")
}

pub(super) fn reject_pre_hub(args: &[&str], expected: &str) {
    let state = TempDir::new().expect("isolated state");
    let cwd = TempDir::new().expect("isolated cwd");
    let output = invoke_with_stdin(state.path(), cwd.path(), args, &[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "stderr was {stderr:?}");
    assert!(!state.path().join("hub.sqlite3").exists());
}

pub(super) fn assert_cli_conflict(output: &Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("conflict"), "stderr was {stderr:?}");
}

pub(super) fn all_hub_state(state: &Path) -> BTreeMap<String, Vec<Vec<SqlValue>>> {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    table_names(&connection)
        .into_iter()
        .map(|table| {
            let rows = snapshot_table(&connection, &table);
            (table, rows)
        })
        .collect()
}

pub(super) fn checkpoint(state: &Path) {
    Connection::open(state.join("hub.sqlite3"))
        .expect("open Hub for checkpoint")
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint Hub");
}

pub(super) fn state_file_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_state_files(root, root, &mut files);
    files
}

fn collect_state_files(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in fs::read_dir(current).expect("read state directory") {
        let path = entry.expect("state entry").path();
        if path.is_dir() {
            collect_state_files(root, &path, files);
        } else {
            let relative = path.strip_prefix(root).expect("state path below root");
            files.insert(
                relative.to_owned(),
                fs::read(&path).expect("read state file"),
            );
        }
    }
}

pub(super) fn golden_scheduled_authorization() -> Vec<u8> {
    let fixture =
        fs::read(repository_root().join(
            "docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json",
        ))
        .expect("read scheduled authorization golden");
    let value: serde_json::Value = serde_json::from_slice(&fixture).expect("golden JSON");
    text(&value["canonical_authorization_json"]).into_bytes()
}

pub(super) fn assert_false_scheduled_readiness_effects(value: &serde_json::Value) {
    for field in [
        "final_effectful_preflight_performed",
        "lifecycle_contract_admitted",
        "execution_authority_released",
        "dispatch_authority_released",
        "fresh_off_machine_consent_obtained",
        "credential_read",
        "credential_preflight_performed",
        "provider_constructed",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "project_lane_claimed",
        "provider_request_sent",
        "execution_performed",
        "progress_observed",
        "terminal_receipt_recorded",
        "successor_advance_authorized",
        "result_produced_or_persisted",
        "database_written",
        "conversation_prompt_or_memory_written",
        "writeback_performed",
    ] {
        assert_eq!(value[field], false, "{field}");
    }
}

pub(super) fn assert_future_only_scheduled_authorization_decisions(value: &serde_json::Value) {
    assert_eq!(value["authorization_decisions_are_future_only"], true);
    assert_eq!(value["all_current_effect_facts_false"], true);
    for decision in value["authorization_decisions"]
        .as_object()
        .unwrap()
        .values()
    {
        assert_eq!(decision, true);
    }
}

fn table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type='table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare table inventory");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query table inventory")
        .collect::<Result<_, _>>()
        .expect("collect table inventory")
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

fn forge_core_dir() -> PathBuf {
    repository_root().join("forge-core")
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

pub(super) fn assert_failure(output: &Output, expected: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "stderr was {stderr:?}");
}

pub(super) fn assert_provider_request_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
