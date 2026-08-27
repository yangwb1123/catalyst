use forge_runtime_domain::{
    GroupAgentGraphExecutionScheduleStore, GroupAgentScheduledNodeContractStore,
};
use rusqlite::{Connection, params, types::Value};

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V20_TO_V21_SQL, MIGRATE_V21_TO_V22_SQL, MIGRATE_V22_TO_V23_SQL, MIGRATE_V23_TO_V24_SQL,
    migrate_with_before_final_fault_for_test, open_database,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version, sqlite_group_agent_graph_execution_schedule_support,
    sqlite_group_agent_scheduled_node_contract_support,
};

const SUCCESSOR_TABLE: &str = "group_agent_graph_scheduled_node_successor_candidates";
const SUCCESSOR_CREATED_INDEX: &str =
    "group_agent_graph_scheduled_node_successor_candidates_created";
const INITIAL_TABLE: &str = "group_agent_graph_scheduled_node_contract_candidates";
const V23_MAX_BYTES: i64 = 4 * 1024 * 1024;
const V24_MAX_BYTES: i64 = 8 * 1024 * 1024;

#[test]
fn v23_successor_row_survives_migration_to_current_byte_for_byte() {
    let fixture = v23_fixture();
    let connection = fixture.connection();
    insert_successor(&connection, "preserved", 257, 257).expect("seed v23 successor row");
    let before = successor_row(&connection);
    drop(connection);

    let migrated = open_database(&fixture.database).expect("migrate exact v23 fixture to current");
    assert_eq!(schema_version(&migrated), super::SCHEMA_VERSION);
    assert_eq!(successor_row(&migrated), before);
    drop(migrated);

    let reopened = open_database(&fixture.database).expect("reopen migrated current fixture");
    assert_eq!(schema_version(&reopened), super::SCHEMA_VERSION);
    assert_eq!(successor_row(&reopened), before);
    drop((reopened, fixture));
}

#[test]
fn v23_rejects_successor_contract_larger_than_four_mib() {
    let fixture = v23_fixture();
    let connection = fixture.connection();

    insert_successor(
        &connection,
        "v23-over-limit",
        V23_MAX_BYTES + 1,
        V23_MAX_BYTES + 1,
    )
    .expect_err("v23 rejects a paired 4 MiB + 1 contract");
    assert_eq!(row_count(&connection, SUCCESSOR_TABLE), 0);
    drop((connection, fixture));
}

#[test]
fn v24_successor_contract_bound_accepts_four_mib_plus_one_and_eight_mib_only() {
    let fixture = scheduled_fixture();
    let connection = fixture.connection();

    insert_successor(
        &connection,
        "above-v23",
        V23_MAX_BYTES + 1,
        V23_MAX_BYTES + 1,
    )
    .expect("v24 accepts paired 4 MiB + 1 contract");
    assert_successor_length(&connection, V23_MAX_BYTES + 1);
    connection
        .execute(&format!("DELETE FROM {SUCCESSOR_TABLE}"), [])
        .expect("clear first boundary row");

    insert_successor(&connection, "v24-maximum", V24_MAX_BYTES, V24_MAX_BYTES)
        .expect("v24 accepts paired 8 MiB contract");
    assert_successor_length(&connection, V24_MAX_BYTES);
    connection
        .execute(&format!("DELETE FROM {SUCCESSOR_TABLE}"), [])
        .expect("clear maximum boundary row");

    insert_successor(
        &connection,
        "v24-over-limit",
        V24_MAX_BYTES + 1,
        V24_MAX_BYTES + 1,
    )
    .expect_err("v24 rejects paired 8 MiB + 1 contract");
    insert_successor(
        &connection,
        "v24-byte-mismatch",
        V23_MAX_BYTES + 1,
        V23_MAX_BYTES,
    )
    .expect_err("v24 rejects a contract_bytes/blob-length mismatch");
    assert_eq!(row_count(&connection, SUCCESSOR_TABLE), 0);
    drop((connection, fixture));
}

#[test]
fn v24_successor_rejects_predecessor_count_and_ordinal_drift() {
    let fixture = scheduled_fixture();
    let connection = fixture.connection();
    insert_successor(&connection, "exact-slot", 257, 257).expect("seed exact successor row");
    for update in [
        format!("UPDATE {SUCCESSOR_TABLE} SET required_predecessor_node_count=1"),
        format!("UPDATE {SUCCESSOR_TABLE} SET execution_ordinal=32"),
    ] {
        connection
            .execute(&update, [])
            .expect_err("v24 rejects successor metadata drift");
    }
    drop((connection, fixture));
}

#[test]
fn v24_keeps_initial_candidate_contract_bound_at_four_mib() {
    let (fixture, request) = sqlite_group_agent_scheduled_node_contract_support::prepared_fixture();
    fixture
        .store
        .admit_group_agent_scheduled_node_contract(&request)
        .expect("admit initial candidate fixture");
    let connection = fixture.connection();

    update_initial_contract(&connection, V23_MAX_BYTES, V23_MAX_BYTES)
        .expect("initial candidate accepts paired 4 MiB contract");
    update_initial_contract(&connection, V23_MAX_BYTES + 1, V23_MAX_BYTES + 1)
        .expect_err("initial candidate still rejects paired 4 MiB + 1 contract");
    let bytes: i64 = connection
        .query_row(
            &format!("SELECT contract_bytes FROM {INITIAL_TABLE}"),
            [],
            |row| row.get(0),
        )
        .expect("read retained initial candidate size");
    assert_eq!(bytes, V23_MAX_BYTES);
    drop((connection, fixture));
}

#[test]
fn malformed_current_v24_objects_are_rejected_without_repair() {
    assert_current_v24_rejected_without_repair("malformed bound literal", |connection| {
        let malformed = MIGRATE_V23_TO_V24_SQL.replace("8388608", "8388607");
        connection.execute_batch(&malformed)
    });
    assert_current_v24_rejected_without_repair("missing successor index", |connection| {
        connection.execute_batch(MIGRATE_V23_TO_V24_SQL)?;
        connection.execute_batch(&format!("DROP INDEX {SUCCESSOR_CREATED_INDEX}"))
    });
    assert_current_v24_rejected_without_repair("rogue object", |connection| {
        connection.execute_batch(MIGRATE_V23_TO_V24_SQL)?;
        connection.execute_batch("CREATE TABLE rogue_v24_table(id TEXT)")
    });
}

#[test]
fn final_validation_fault_rolls_v23_to_current_back_atomically() {
    let fixture = v23_fixture();
    let connection = fixture.connection();
    insert_successor(&connection, "rollback", 513, 513).expect("seed rollback successor row");
    let before_schema = schema_snapshot(&connection);
    let before_row = successor_row(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), super::SCHEMA_VERSION);
        assert_eq!(successor_row(migrated), before_row);
        migrated.execute_batch("CREATE TABLE rogue_v26_final_fault(id TEXT)")
    })
    .expect_err("final v26 validation rejects injected rogue object");

    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_eq!(schema_version(&connection), 23);
    assert_eq!(schema_snapshot(&connection), before_schema);
    assert_eq!(successor_row(&connection), before_row);
    assert!(!schema_object_named(&connection, "rogue_v26_final_fault"));
    assert!(!schema_object_named(
        &connection,
        "group_agent_graph_scheduled_node_successor_candidates_v24"
    ));
    drop((connection, fixture));
}

fn v23_fixture() -> super::sqlite_group_agent_graph_run_support::Fixture {
    let fixture = scheduled_fixture();
    let connection = fixture.connection();
    restore_exact_v24(&connection);
    connection
        .execute_batch(MIGRATE_V20_TO_V21_SQL)
        .expect("restore exact v21 successor table");
    connection
        .execute_batch(MIGRATE_V21_TO_V22_SQL)
        .expect("restore exact v22 schema");
    connection
        .execute_batch(MIGRATE_V22_TO_V23_SQL)
        .expect("restore exact v23 schema");
    connection
        .execute_batch(super::RESTORE_HISTORICAL_ANALYSES_SQL)
        .expect("restore historical analyses definitions for downgraded fixture");
    assert_eq!(schema_version(&connection), 23);
    drop(connection);
    fixture
}

fn scheduled_fixture() -> super::sqlite_group_agent_graph_run_support::Fixture {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let request = sqlite_group_agent_graph_execution_schedule_support::request(
        &fixture,
        "schema-v24-schedule",
        40,
    );
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&request)
        .expect("seed execution schedule");
    fixture
}

fn restore_exact_v24(connection: &Connection) {
    connection
        .execute_batch(super::DROP_V29_CONTROLLER_SQL)
        .expect("drop v29 controller journal");
    connection
        .execute_batch(super::DROP_V28_LINEAGE_SQL)
        .expect("drop v28 Run lineage table");
    connection
        .execute_batch(super::RESTORE_HISTORICAL_ANALYSES_SQL)
        .expect("restore v24 endpoint definitions");
    connection
        .execute_batch(super::DROP_V27_SEMANTIC_VIEW_SQL)
        .expect("drop v27 semantic projection");
    connection
        .execute_batch(
            "DROP TABLE governance_structural_heads;
             DROP TABLE governance_records;
             DROP TABLE governance_record_append_batches;
             PRAGMA user_version = 24;",
        )
        .expect("restore exact v24 schema");
}

fn insert_successor(
    connection: &Connection,
    tag: &str,
    blob_bytes: i64,
    declared_bytes: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        &format!(
            "INSERT INTO {SUCCESSOR_TABLE}(
               id,graph_run_id,graph_id,schedule_id,contract_version,
               scheduler_protocol_version,node_execution_protocol_version,
               execution_schedule_protocol_version,contract_scope,
               control_snapshot_sha256,schedule_sha256,expected_last_event_seq,
               expected_last_event_sha256,execution_ordinal,node_id,
               authored_node_index,topology_wave_index,attempt,project_lane_sha256,
               request_id,request_sha256,required_predecessor_node_count,
               predecessor_receipt_count,lifecycle_contract_admitted,
               provider_request_present,execution_authority_released,
               dispatch_authority_released,progress_observed,
               successor_advance_authorized,contract_blob,contract_bytes,
               contract_sha256,idempotency_key,created_at_ms
             )
             SELECT
               ?1,graph_run_id,graph_id,id,2,1,2,1,'schedule_successor_only',
               control_snapshot_sha256,schedule_sha256,expected_last_event_seq,
               expected_last_event_sha256,1,'successor-node',1,1,1,zeroblob(32),
               ?2,zeroblob(32),0,0,0,0,0,0,0,0,zeroblob(?3),?4,
               zeroblob(32),?5,60
             FROM group_agent_graph_execution_schedules"
        ),
        params![
            format!("successor-{tag}"),
            format!("request-{tag}"),
            blob_bytes,
            declared_bytes,
            format!("successor-key-{tag}"),
        ],
    )
}

fn update_initial_contract(
    connection: &Connection,
    blob_bytes: i64,
    declared_bytes: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        &format!(
            "UPDATE {INITIAL_TABLE}
             SET contract_blob=zeroblob(?1), contract_bytes=?2"
        ),
        params![blob_bytes, declared_bytes],
    )
}

fn successor_row(connection: &Connection) -> Vec<Value> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM {SUCCESSOR_TABLE}"))
        .expect("prepare successor row snapshot");
    let column_count = statement.column_count();
    statement
        .query_row([], |row| {
            (0..column_count)
                .map(|column| row.get(column))
                .collect::<rusqlite::Result<Vec<Value>>>()
        })
        .expect("read successor row snapshot")
}

fn assert_successor_length(connection: &Connection, expected: i64) {
    let actual: (i64, i64) = connection
        .query_row(
            &format!("SELECT length(contract_blob),contract_bytes FROM {SUCCESSOR_TABLE}"),
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read successor contract size");
    assert_eq!(actual, (expected, expected));
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}

fn assert_current_v24_rejected_without_repair(
    name: &str,
    mutate: impl FnOnce(&Connection) -> rusqlite::Result<()>,
) {
    let fixture = v23_fixture();
    let connection = fixture.connection();
    mutate(&connection).unwrap_or_else(|error| panic!("forge {name}: {error}"));
    assert_eq!(schema_version(&connection), 24);
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);

    let error = open_database(&fixture.database)
        .err()
        .unwrap_or_else(|| panic!("{name} unexpectedly opened"));
    assert!(
        matches!(error, HubStoreError::Corrupt { .. }),
        "{name}: {error:?}"
    );

    let unchanged = Connection::open(&fixture.database).expect("reopen rejected v24 fixture");
    assert_eq!(schema_version(&unchanged), 24, "{name}");
    assert_eq!(schema_snapshot(&unchanged), before, "{name}");
    drop((unchanged, fixture));
}
