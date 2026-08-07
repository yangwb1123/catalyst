use rusqlite::{Connection, types::Value};

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V11_TO_V12_SQL, MIGRATE_V12_TO_V13_SQL, migrate_with_before_final_fault_for_test,
    open_database,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_exists, schema_object_named,
    schema_v12_migration_tests::legacy_v11_database,
    schema_version, table_columns,
};

const SCHEDULE_TABLE: &str = "group_agent_graph_execution_schedules";
const SCHEDULE_INDEX: &str = "group_agent_graph_execution_schedules_created";
const SCHEDULED_CONTRACT_TABLE: &str = "group_agent_graph_scheduled_node_contract_candidates";
const SCHEDULED_CONTRACT_INDEXES: &[&str] = &[
    "group_agent_graph_scheduled_node_candidates_project_lane",
    "group_agent_graph_scheduled_node_candidates_created",
];
const SCHEDULED_PROVIDER_REQUEST_OBJECTS: &[&str] = &[
    "group_agent_graph_scheduled_node_provider_requests",
    "group_agent_graph_scheduled_node_provider_requests_project_lane",
    "group_agent_graph_scheduled_node_provider_requests_created",
];
const SCHEDULED_DISPATCH_LIFECYCLE_OBJECTS: &[&str] = &[
    "group_agent_graph_scheduled_node_dispatch_lifecycles",
    "group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active",
    "group_agent_graph_scheduled_node_dispatch_lifecycles_created",
];
const SUCCESSOR_CANDIDATE_OBJECTS: &[&str] = &[
    "group_agent_graph_scheduled_node_successor_candidates",
    "group_agent_graph_scheduled_node_successor_candidates_created",
];
const ACTIVE_TABLES: &[&str] = &[
    "group_agent_graph_runs",
    "group_agent_graph_run_events",
    "group_agent_graph_node_execution_contracts",
    "group_agent_graph_node_dispatch_requests",
    "group_agent_graph_node_dispatch_claims",
    "group_agent_project_lane_ownerships",
];

#[test]
fn active_v4_claim_and_old_schema_survive_v13_migration_and_reopen() {
    let (root, database) = legacy_active_v12_database();
    let legacy = Connection::open(&database).expect("open active v12 fixture");
    let before_rows = active_rows(&legacy);
    let before_schema = old_schema(&schema_snapshot(&legacy));
    drop(legacy);

    let connection = open_database(&database).expect("active v12 Hub migrates to v15");
    assert_v13_shape(&connection);
    assert_eq!(active_rows(&connection), before_rows);
    assert_eq!(old_schema(&schema_snapshot(&connection)), before_schema);
    assert_active_lane(&connection);
    assert_foreign_keys_clean(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated v15 Hub reopens");
    assert_v13_shape(&reopened);
    assert_eq!(active_rows(&reopened), before_rows);
    assert_active_lane(&reopened);
    drop((reopened, root));
}

#[test]
fn v12_future_schedule_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_active_v12_database();
    let blocker = Connection::open(&database).expect("open v12 blocker fixture");
    let before_rows = active_rows(&blocker);
    blocker
        .execute_batch(&format!("CREATE TABLE {SCHEDULE_TABLE}(blocker TEXT)"))
        .expect("install future v13 table blocker");
    drop(blocker);

    assert_open_corrupt(&database);
    let unchanged = Connection::open(&database).expect("reopen rejected v12 fixture");
    assert_eq!(schema_version(&unchanged), 12);
    assert_eq!(table_columns(&unchanged, SCHEDULE_TABLE), ["blocker"]);
    assert!(!schema_object_named(&unchanged, SCHEDULE_INDEX));
    assert_eq!(active_rows(&unchanged), before_rows);
    assert_active_lane(&unchanged);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v12_to_current_atomically() {
    let (root, database) = legacy_active_v12_database();
    let connection = Connection::open(&database).expect("open v12 rollback fixture");
    let before_schema = schema_snapshot(&connection);
    let before_rows = active_rows(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_v13_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_v13_final_fault(id TEXT)")
    })
    .expect_err("final v15 validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_eq!(schema_version(&connection), 12);
    assert_eq!(schema_snapshot(&connection), before_schema);
    assert_eq!(active_rows(&connection), before_rows);
    assert!(!schema_object_named(&connection, SCHEDULE_TABLE));
    assert!(!schema_object_named(&connection, SCHEDULE_INDEX));
    assert!(!schema_object_named(&connection, "rogue_v13_final_fault"));
    assert_active_lane(&connection);
    drop((connection, root));
}

#[test]
fn malformed_v13_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v13_cases() {
        let (root, database) = legacy_active_v12_database();
        let connection = Connection::open(&database).expect("open malformed fixture");
        let before_rows = active_rows(&connection);
        connection.execute_batch(&sql).expect("forge malformed v13");
        drop(connection);

        assert_open_corrupt(&database);
        let unchanged = Connection::open(&database).expect("reopen malformed v13");
        assert_eq!(schema_version(&unchanged), 13);
        assert_eq!(active_rows(&unchanged), before_rows);
        assert_active_lane(&unchanged);
        drop((unchanged, root));
    }
}

#[test]
fn v13_sidecar_constraints_are_strict_and_leave_main_journal_unchanged() {
    let (root, database) = legacy_active_v12_database();
    let connection = open_database(&database).expect("migrate constraint fixture");
    let events_before = row_count(&connection, "group_agent_graph_run_events");
    insert_valid_schedule(&connection);
    assert_eq!(
        row_count(&connection, "group_agent_graph_run_events"),
        events_before
    );

    for assignment in invalid_schedule_assignments() {
        let sql = format!("UPDATE {SCHEDULE_TABLE} SET {assignment} WHERE id='schedule-1'");
        assert!(
            connection.execute_batch(&sql).is_err(),
            "accepted {assignment}"
        );
    }
    assert_uniqueness_and_foreign_key_constraints(&connection);
    assert_eq!(
        row_count(&connection, "group_agent_graph_run_events"),
        events_before
    );
    drop((connection, root));
}

pub(super) fn legacy_active_v12_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v11_database();
    let connection = Connection::open(&database).expect("open v11 fixture");
    connection
        .execute_batch(MIGRATE_V11_TO_V12_SQL)
        .expect("migrate fixture to v12");
    seed_active_v4_claim(&connection);
    drop(connection);
    (root, database)
}

fn seed_active_v4_claim(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,
               event_sha256,created_at_ms
             ) VALUES(
               'graph-run-legacy',4,4,'node_dispatch_released',x'7b7d',2,zeroblob(32),13
             );
             INSERT INTO group_agent_graph_node_dispatch_claims(
               dispatch_id,claim_version,graph_run_id,authorization_id,authorization_sha256,
               dispatch_request_id,dispatch_request_sha256,logical_request_sha256,
               request_body_sha256,request_body_bytes,pricing_snapshot_sha256,node_id,attempt,
               max_cost_usd_micros,consent_contract_version,lane_ownership_id,
               project_lane_sha256,expected_last_event_seq,expected_last_event_sha256,
               claim_event_sha256,claim_blob,claim_bytes,released_at_ms
             ) VALUES(
               'dispatch-legacy',1,'graph-run-legacy','authorization-legacy',zeroblob(32),
               'request-legacy',zeroblob(32),zeroblob(32),zeroblob(32),2,zeroblob(32),
               'node-legacy',1,1,1,'lane-legacy',zeroblob(32),3,zeroblob(32),
               zeroblob(32),x'7b7d',2,13
             );
             INSERT INTO group_agent_project_lane_ownerships(
               lane_ownership_id,lane_version,project_lane_sha256,graph_run_id,dispatch_id,
               node_id,attempt,claim_event_sha256,lane_blob,lane_bytes,claimed_at_ms
             ) VALUES(
               'lane-legacy',1,zeroblob(32),'graph-run-legacy','dispatch-legacy',
               'node-legacy',1,zeroblob(32),x'7b7d',2,13
             );
             UPDATE group_agent_graph_runs
             SET run_version=4,status='dispatch_unknown',dispatch_authority_released=1,
                 last_event_seq=4,journal_bytes=8
             WHERE id='graph-run-legacy';",
        )
        .expect("seed active v4 claim and lane");
}

fn insert_valid_schedule(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO group_agent_graph_execution_schedules(
               id,graph_run_id,graph_id,schedule_version,scheduler_protocol_version,
               execution_schedule_protocol_version,control_snapshot_sha256,
               expected_last_event_seq,expected_last_event_sha256,initial_node,
               node_count,wave_count,execution_contract_present,dispatch_authority_released,
               progress_observed,successor_advanced,schedule_blob,schedule_bytes,
               schedule_sha256,idempotency_key,created_at_ms
             ) VALUES(
               'schedule-1','graph-run-legacy','graph-legacy',1,1,1,zeroblob(32),1,zeroblob(32),
               'node-legacy',2,1,0,0,0,0,x'7b7d',2,zeroblob(32),'schedule-key',14
             );",
        )
        .expect("insert structurally valid passive schedule");
}

fn invalid_schedule_assignments() -> [&'static str; 13] {
    [
        "schedule_version=2",
        "scheduler_protocol_version=2",
        "execution_schedule_protocol_version=2",
        "expected_last_event_seq=2",
        "initial_node=''",
        "node_count=1",
        "wave_count=3",
        "execution_contract_present=1",
        "dispatch_authority_released=1",
        "progress_observed=1",
        "successor_advanced=1",
        "schedule_bytes=3",
        "created_at_ms=-1",
    ]
}

fn assert_uniqueness_and_foreign_key_constraints(connection: &Connection) {
    let unique = connection
        .execute_batch(
            "INSERT INTO group_agent_graph_execution_schedules
                 SELECT 'schedule-2',graph_run_id,graph_id,schedule_version,scheduler_protocol_version,
                   execution_schedule_protocol_version,control_snapshot_sha256,
                   expected_last_event_seq,expected_last_event_sha256,initial_node,
                   node_count,wave_count,execution_contract_present,dispatch_authority_released,
                   progress_observed,successor_advanced,schedule_blob,schedule_bytes,
                   schedule_sha256,'schedule-key-2',created_at_ms
                 FROM group_agent_graph_execution_schedules WHERE id='schedule-1';",
        )
        .expect_err("one schedule per Graph Run is enforced");
    assert!(
        unique.to_string().contains("UNIQUE constraint failed"),
        "wrong uniqueness error: {unique}"
    );
    let foreign_key = connection
        .execute_batch(
            "INSERT INTO group_agent_graph_execution_schedules
                 SELECT 'schedule-3','missing-run',graph_id,schedule_version,scheduler_protocol_version,
                   execution_schedule_protocol_version,control_snapshot_sha256,
                   expected_last_event_seq,expected_last_event_sha256,initial_node,
                   node_count,wave_count,execution_contract_present,dispatch_authority_released,
                   progress_observed,successor_advanced,schedule_blob,schedule_bytes,
                   schedule_sha256,'schedule-key-3',created_at_ms
                 FROM group_agent_graph_execution_schedules WHERE id='schedule-1';",
        )
        .expect_err("orphan schedule is rejected");
    assert!(
        foreign_key
            .to_string()
            .contains("FOREIGN KEY constraint failed"),
        "wrong foreign-key error: {foreign_key}"
    );
    assert_eq!(row_count(connection, SCHEDULE_TABLE), 1);
}

fn malformed_v13_cases() -> Vec<String> {
    vec![
        malformed(
            "graph_run_id TEXT NOT NULL UNIQUE",
            "graph_run_id TEXT NOT NULL",
        ),
        malformed("schedule_version = 1", "schedule_version BETWEEN 1 AND 2"),
        malformed(
            "expected_last_event_seq = 1",
            "expected_last_event_seq >= 1",
        ),
        malformed("successor_advanced = 0", "successor_advanced IN (0, 1)"),
        malformed(
            "schedule_bytes = length(schedule_blob)",
            "schedule_bytes >= 1",
        ),
        malformed(
            "ON group_agent_graph_execution_schedules(created_at_ms DESC, id DESC)",
            "ON group_agent_graph_execution_schedules(created_at_ms, id DESC)",
        ),
        format!("{MIGRATE_V12_TO_V13_SQL}\nCREATE TABLE rogue_v13_table(id TEXT);"),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V12_TO_V13_SQL.replacen(original, replacement, 1);
    assert_ne!(
        sql, MIGRATE_V12_TO_V13_SQL,
        "fixture replacement must match"
    );
    sql
}

fn assert_v13_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), 23);
    assert!(schema_object_exists(connection, "table", SCHEDULE_TABLE));
    assert!(schema_object_exists(connection, "index", SCHEDULE_INDEX));
    assert_eq!(row_count(connection, SCHEDULE_TABLE), 0);
}

fn active_rows(connection: &Connection) -> Vec<(&'static str, Vec<Vec<Value>>)> {
    ACTIVE_TABLES
        .iter()
        .map(|&table| (table, table_rows(connection, table)))
        .collect()
}

fn table_rows(connection: &Connection, table: &str) -> Vec<Vec<Value>> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .expect("prepare active table snapshot");
    let columns = statement.column_count();
    statement
        .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
        .expect("query active table snapshot")
        .collect::<Result<_, _>>()
        .expect("read active table snapshot")
}

fn old_schema(snapshot: &[SchemaRow]) -> Vec<SchemaRow> {
    snapshot
        .iter()
        .filter(|(_, name, _, _)| {
            name != SCHEDULE_TABLE
                && name != SCHEDULE_INDEX
                && name != SCHEDULED_CONTRACT_TABLE
                && !SCHEDULED_CONTRACT_INDEXES.contains(&name.as_str())
                && !SCHEDULED_PROVIDER_REQUEST_OBJECTS.contains(&name.as_str())
                && !SCHEDULED_DISPATCH_LIFECYCLE_OBJECTS.contains(&name.as_str())
                && !SUCCESSOR_CANDIDATE_OBJECTS.contains(&name.as_str())
        })
        .cloned()
        .collect()
}

fn assert_active_lane(connection: &Connection) {
    let state: (i64, String, i64, i64) = connection
        .query_row(
            "SELECT run_version,status,dispatch_authority_released,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("active Graph Run survives");
    assert_eq!(state, (4, "dispatch_unknown".into(), 1, 4));
    assert_eq!(
        row_count(connection, "group_agent_project_lane_ownerships"),
        1
    );
}

fn assert_foreign_keys_clean(connection: &Connection) {
    assert_eq!(row_count(connection, "pragma_foreign_key_check"), 0);
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}

fn assert_open_corrupt(database: &std::path::Path) {
    assert!(matches!(
        open_database(database),
        Err(HubStoreError::Corrupt { .. })
    ));
}
