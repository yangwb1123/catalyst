use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V10_TO_V11_SQL, MIGRATE_V11_TO_V12_SQL, migrate_with_before_final_fault_for_test,
    open_database, open_existing_dispatch_preflight_read_only_database, schema_object_exists,
    schema_object_named, schema_v11_migration_tests::legacy_v10_database, schema_version,
    table_columns,
};

const LIFECYCLE_TABLES: &[&str] = &[
    "group_agent_graph_node_dispatch_claims",
    "group_agent_project_lane_ownerships",
    "group_agent_graph_node_terminal_artifacts",
    "group_agent_graph_node_terminal_receipts",
];

#[cfg(unix)]
#[test]
fn dispatch_preflight_reads_exact_v11_without_migration_or_sidecars() {
    use std::os::unix::fs::PermissionsExt;

    let (root, database) = legacy_v11_database();
    let wal = Connection::open(&database).expect("open v11 WAL fixture");
    wal.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("persist v11 WAL header");
    wal.close()
        .map_err(|(_, error)| error)
        .expect("close v11 WAL fixture cleanly");
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600))
        .expect("restrict v11 fixture");
    let before = std::fs::read(&database).expect("read v11 fixture");

    let connection = open_existing_dispatch_preflight_read_only_database(&database)
        .expect("dispatch preflight accepts exact v11");
    assert_eq!(schema_version(&connection), 11);
    assert_legacy_v3_run(&connection);
    drop(connection);

    assert_eq!(
        std::fs::read(&database).expect("reread v11 fixture"),
        before
    );
    for suffix in ["-wal", "-shm", "-journal"] {
        assert!(!std::path::PathBuf::from(format!("{}{suffix}", database.display())).exists());
    }
    drop(root);
}

#[test]
fn v11_request_survives_current_migration_and_reopen() {
    let (root, database) = legacy_v11_database();
    let connection = open_database(&database).expect("v11 Hub migrates to v15");
    assert_current_shape(&connection);
    assert_legacy_v3_run(&connection);
    assert_lifecycle_empty(&connection);
    assert_foreign_keys_clean(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated v15 Hub reopens");
    assert_current_shape(&reopened);
    assert_legacy_v3_run(&reopened);
    drop((reopened, root));
}

#[test]
fn v11_future_claim_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v11_database();
    let blocker = Connection::open(&database).expect("open v11 blocker fixture");
    blocker
        .execute_batch("CREATE TABLE group_agent_graph_node_dispatch_claims(blocker TEXT)")
        .expect("install future v12 table blocker");
    drop(blocker);

    assert_open_corrupt(&database);
    let unchanged = Connection::open(&database).expect("reopen rejected v11 fixture");
    assert_eq!(schema_version(&unchanged), 11);
    assert_eq!(
        table_columns(&unchanged, "group_agent_graph_node_dispatch_claims"),
        ["blocker"]
    );
    assert_legacy_v3_run(&unchanged);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v11_to_current_atomically() {
    let (root, database) = legacy_v11_database();
    let connection = Connection::open(&database).expect("open v11 rollback fixture");
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_current_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_v13_final_fault(id TEXT)")
    })
    .expect_err("final v15 validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    assert_eq!(schema_version(&connection), 11);
    for table in LIFECYCLE_TABLES {
        assert!(!schema_object_named(&connection, table));
    }
    assert!(!schema_object_named(&connection, "rogue_v13_final_fault"));
    assert_legacy_v3_run(&connection);
    drop((connection, root));
}

#[test]
fn malformed_v12_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v12_cases() {
        let (root, database) = legacy_v11_database();
        let connection = Connection::open(&database).expect("open v11 malformed fixture");
        connection.execute_batch(&sql).expect("forge malformed v12");
        drop(connection);
        assert_open_corrupt(&database);
        let unchanged = Connection::open(&database).expect("reopen malformed v12");
        assert_eq!(schema_version(&unchanged), 12);
        assert_legacy_v3_run(&unchanged);
        drop((unchanged, root));
    }
}

#[test]
fn v12_run_state_check_rejects_partial_claim_shapes() {
    let (root, database) = legacy_v11_database();
    let connection = open_database(&database).expect("migrate state-check fixture");
    for assignment in [
        "run_version=4",
        "status='dispatch_unknown'",
        "dispatch_authority_released=1",
        "last_event_seq=4",
    ] {
        let sql =
            format!("UPDATE group_agent_graph_runs SET {assignment} WHERE id='graph-run-legacy'");
        assert!(connection.execute_batch(&sql).is_err(), "{assignment}");
    }
    assert_legacy_v3_run(&connection);
    drop((connection, root));
}

pub(super) fn legacy_v11_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v10_database();
    let connection = Connection::open(&database).expect("open v10 fixture");
    connection
        .execute_batch(MIGRATE_V10_TO_V11_SQL)
        .expect("migrate fixture to v11");
    seed_v11_request(&connection);
    drop(connection);
    (root, database)
}

fn seed_v11_request(connection: &Connection) {
    connection
        .execute_batch(
            "UPDATE group_agent_graph_runs
             SET run_version=3,status='awaiting_dispatch_authorization',
                 dispatch_request_present=1,last_event_seq=3,journal_bytes=6
             WHERE id='graph-run-legacy';
             INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,
               event_sha256,created_at_ms
             ) VALUES(
               'graph-run-legacy',3,3,'node_dispatch_request_prepared',
               x'7b7d',2,zeroblob(32),12
             );
             INSERT INTO group_agent_graph_node_dispatch_requests(
               id,graph_run_id,contract_id,request_version,codec_protocol_version,
               node_id,attempt,contract_sha256,request_sha256,project_lane_sha256,
               provider_kind,endpoint,model,destination_sha256,pricing_snapshot_sha256,
               provider_request_blob,provider_request_bytes,provider_request_sha256,
               dispatch_request_sha256,expected_last_event_seq,expected_last_event_sha256,
               idempotency_key,created_at_ms
             ) VALUES(
               'request-legacy','graph-run-legacy','contract-legacy',1,1,
               'node-legacy',1,zeroblob(32),zeroblob(32),zeroblob(32),
               'openai_responses','https://api.openai.com/v1/responses','model-1',
               zeroblob(32),zeroblob(32),x'7b7d',2,zeroblob(32),zeroblob(32),
               2,zeroblob(32),'request-legacy-key',12
             );",
        )
        .expect("seed v11 dispatch request");
}

fn malformed_v12_cases() -> Vec<String> {
    vec![
        malformed(
            "run_version IN (1, 2, 3, 4, 5)",
            "run_version BETWEEN 1 AND 5",
        ),
        malformed(
            "expected_last_event_seq = 3",
            "expected_last_event_seq >= 3",
        ),
        malformed(
            "max_cost_usd_micros BETWEEN 1 AND 1000000000000",
            "max_cost_usd_micros >= 0",
        ),
        malformed(
            "project_lane_sha256 BLOB NOT NULL UNIQUE",
            "project_lane_sha256 BLOB NOT NULL",
        ),
        malformed(
            "length(receipt_blob) BETWEEN 1 AND 65536",
            "length(receipt_blob) BETWEEN 1 AND 1048576",
        ),
        malformed("retry_authorized = 0", "retry_authorized IN (0, 1)"),
        malformed(
            "lane_release_authorized = 1",
            "lane_release_authorized IN (0, 1)",
        ),
        format!("{MIGRATE_V11_TO_V12_SQL}\nCREATE TABLE rogue_v12_table(id TEXT);"),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V11_TO_V12_SQL.replacen(original, replacement, 1);
    assert_ne!(
        sql, MIGRATE_V11_TO_V12_SQL,
        "fixture replacement must match"
    );
    sql
}

fn assert_current_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), 24);
    for table in LIFECYCLE_TABLES {
        assert!(
            schema_object_exists(connection, "table", table),
            "missing v12 table {table}"
        );
    }
}

fn assert_legacy_v3_run(connection: &Connection) {
    let row: (i64, String, i64, i64) = connection
        .query_row(
            "SELECT run_version,status,last_event_seq,journal_bytes
             FROM group_agent_graph_runs WHERE id='graph-run-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("legacy v3 Graph Run survives");
    assert_eq!(row, (3, "awaiting_dispatch_authorization".into(), 3, 6));
}

fn assert_lifecycle_empty(connection: &Connection) {
    for table in LIFECYCLE_TABLES {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = connection.query_row(&sql, [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0, "{table}");
    }
}

fn assert_foreign_keys_clean(connection: &Connection) {
    let violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign key check");
    assert_eq!(violations, 0);
}

fn assert_open_corrupt(database: &std::path::Path) {
    assert!(matches!(
        open_database(database),
        Err(HubStoreError::Corrupt { .. })
    ));
}
