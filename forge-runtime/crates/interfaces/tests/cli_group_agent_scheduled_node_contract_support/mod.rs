use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use rusqlite::{Connection, types::Value as SqlValue};
use serde_json::Value;

use super::{
    PRICING,
    group_agent_graph_run_support::{Fixture, command, invoke_with_stdin},
    group_agent_graph_support::{successful_json, text},
};

pub(super) const CREDENTIAL_MARKER: &str = "scheduled-candidate-credential-must-not-be-read";
const CREDENTIAL_SENTINEL: &str =
    "scheduled-candidate-credential-must-not-be-read\r\nx-private-header: rejected";
const DEFAULT_ENDPOINT: &str = "https://api.openai.com/v1/responses";

pub(super) fn prepare_run(fixture: &Fixture, key: &str) -> String {
    let prepared = successful_json(&fixture.prepare(&fixture.plan(), key));
    text(&prepared["inspection"]["run"]["graph_run_id"])
}

pub(super) fn export_control(fixture: &Fixture, graph_run_id: &str) -> Vec<u8> {
    let output = command(fixture.state.path(), fixture.cwd.path(), &[
        "group",
        "graph",
        "run",
        "control",
        "export",
        graph_run_id,
    ])
    .output()
    .expect("export control");
    assert_success(&output);
    output.stdout
}

pub(super) fn build_schedule(control: &[u8]) -> Vec<u8> {
    run_go(
        &[
            "run",
            "./cmd/forge",
            "graph-execution-schedule",
            "--control",
            "-",
        ],
        control,
    )
}

pub(super) fn build_candidate(control: &[u8], schedule_sha256: &str) -> Vec<u8> {
    build_candidate_at(control, schedule_sha256, DEFAULT_ENDPOINT)
}

pub(super) fn build_candidate_at(control: &[u8], schedule_sha256: &str, endpoint: &str) -> Vec<u8> {
    run_go(&candidate_args(schedule_sha256, endpoint), control)
}

pub(super) fn build_legacy_contract(control: &[u8]) -> Vec<u8> {
    run_go(&legacy_args(), control)
}

fn candidate_args<'a>(schedule_sha256: &'a str, endpoint: &'a str) -> Vec<&'a str> {
    let mut args = vec![
        "run",
        "./cmd/forge",
        "graph-scheduled-node-contract",
        "--control",
        "-",
        "--schedule-sha256",
        schedule_sha256,
    ];
    args.extend(execution_args(endpoint));
    args
}

fn legacy_args() -> Vec<&'static str> {
    let mut args = vec![
        "run",
        "./cmd/forge",
        "graph-node-contract",
        "--control",
        "-",
    ];
    args.extend(execution_args(DEFAULT_ENDPOINT));
    args
}

fn execution_args(endpoint: &str) -> Vec<&str> {
    vec![
        "--endpoint",
        endpoint,
        "--model",
        "private-scheduled-contract-model",
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
        PRICING,
        "--max-result-bytes",
        "16384",
    ]
}

fn run_go(args: &[&str], stdin: &[u8]) -> Vec<u8> {
    let mut child = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge-core");
    child
        .stdin
        .take()
        .expect("Go stdin")
        .write_all(stdin)
        .expect("write exact control");
    let output = child.wait_with_output().expect("wait for forge-core");
    assert_success(&output);
    assert!(!output.stdout.ends_with(b"\n"));
    output.stdout
}

pub(super) fn admit_schedule(fixture: &Fixture, graph_run_id: &str, schedule: &[u8]) -> String {
    let output = invoke_with_stdin(
        fixture.state.path(),
        fixture.cwd.path(),
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
            "scheduled-contract-source-schedule",
        ],
        schedule,
    );
    text(&successful_json(&output)["inspection"]["record"]["schedule_id"])
}

pub(super) fn admit_candidate(fixture: &Fixture, run: &str, key: &str, bytes: &[u8]) -> Value {
    successful_json(&invoke_candidate(fixture, run, key, bytes))
}

pub(super) fn invoke_candidate(fixture: &Fixture, run: &str, key: &str, bytes: &[u8]) -> Output {
    let mut child = command(fixture.state.path(), fixture.cwd.path(), &[
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "admit",
        run,
        "--contract",
        "-",
        "--idempotency-key",
        key,
    ])
    .env("OPENAI_API_KEY", CREDENTIAL_SENTINEL)
    .env_remove("ANTHROPIC_API_KEY")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("spawn scheduled-contract CLI");
    child
        .stdin
        .take()
        .expect("scheduled-contract stdin")
        .write_all(bytes)
        .expect("write scheduled contract");
    child
        .wait_with_output()
        .expect("wait for scheduled-contract CLI")
}

pub(super) fn admit_legacy_contract(fixture: &Fixture, run: &str, bytes: &[u8]) {
    let output = invoke_with_stdin(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "contract",
            "admit",
            run,
            "--contract",
            "-",
            "--idempotency-key",
            "legacy-before-candidate",
        ],
        bytes,
    );
    successful_json(&output);
}

pub(super) fn non_candidate_hub_state(state: &Path) -> BTreeMap<String, Vec<Vec<SqlValue>>> {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    assert_candidate_table_present(&connection);
    non_candidate_table_names(&connection)
        .into_iter()
        .map(|table| {
            let rows = snapshot_table(&connection, &table);
            (table, rows)
        })
        .collect()
}

#[allow(dead_code)]
pub(super) fn non_provider_request_hub_state(state: &Path) -> BTreeMap<String, Vec<Vec<SqlValue>>> {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    let table = "group_agent_graph_scheduled_node_provider_requests";
    let present: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )
        .expect("provider request table presence");
    assert!(present, "schema v15 provider request table missing");
    non_candidate_table_names(&connection)
        .into_iter()
        .chain(std::iter::once(
            "group_agent_graph_scheduled_node_contract_candidates".to_owned(),
        ))
        .filter(|name| name != table)
        .map(|name| {
            let rows = snapshot_table(&connection, &name);
            (name, rows)
        })
        .collect()
}

fn assert_candidate_table_present(connection: &Connection) {
    let candidate_table_present: bool = connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_schema
               WHERE type='table'
                 AND name='group_agent_graph_scheduled_node_contract_candidates'
             )",
            [],
            |row| row.get(0),
        )
        .expect("candidate table presence");
    assert!(
        candidate_table_present,
        "schema v14 candidate table missing"
    );
}

fn non_candidate_table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type='table'
               AND name NOT LIKE 'sqlite_%'
               AND name<>'group_agent_graph_scheduled_node_contract_candidates'
             ORDER BY name",
        )
        .expect("prepare table inventory");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query table inventory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect table inventory")
}

fn snapshot_table(connection: &Connection, table: &str) -> Vec<Vec<SqlValue>> {
    let quoted = format!("\"{}\"", table.replace('"', "\"\""));
    let columns = connection
        .prepare(&format!("SELECT * FROM {quoted} LIMIT 0"))
        .expect("prepare table shape")
        .column_count();
    assert!(columns > 0, "table {table} has no columns");
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

pub(super) fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("canonical JSON")
}

pub(super) fn assert_conflict(output: &Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("conflicts"));
}

fn forge_core_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("forge-core")
}

pub(super) fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
