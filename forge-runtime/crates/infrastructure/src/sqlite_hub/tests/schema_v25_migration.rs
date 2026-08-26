use rusqlite::{Connection, params};

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V24_TO_V25_SQL, migrate_with_before_final_fault_for_test, open_database,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version, sqlite_group_agent_graph_execution_schedule_support,
};

const BATCHES: &str = "governance_record_append_batches";
const RECORDS: &str = "governance_records";
const HEADS: &str = "governance_structural_heads";
const APPENDED_INDEX: &str = "governance_records_appended";
const AGGREGATE_INDEX: &str = "governance_records_aggregate_appended";
const KIND_INDEX: &str = "governance_records_kind_appended";
type ScheduleRow = (String, String, Vec<u8>, i64);

#[test]
fn v24_data_survives_current_migrations_and_journal_starts_empty() {
    let fixture = exact_v24_fixture();
    let before = schedule_rows(&fixture.connection());

    let migrated = open_database(&fixture.database).expect("migrate exact v24 fixture to current");
    assert_eq!(schema_version(&migrated), super::SCHEMA_VERSION);
    assert_eq!(schedule_rows(&migrated), before);
    assert_journal_empty(&migrated);
    drop(migrated);

    let reopened = open_database(&fixture.database).expect("reopen migrated current fixture");
    assert_eq!(schema_version(&reopened), super::SCHEMA_VERSION);
    assert_eq!(schedule_rows(&reopened), before);
    assert_journal_empty(&reopened);
    drop((reopened, fixture));
}

#[test]
fn v25_journal_constraints_reject_invalid_rows_and_protect_links() {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let connection = fixture.connection();
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .expect("enable foreign keys");
    insert_batch(&connection, "batch-1", "append-key-1", 1).expect("insert exact batch");
    insert_record(&connection, "record-1", "batch-1", 0, "aggregate-1", 1)
        .expect("insert exact record");
    insert_head(&connection, "record-1", "aggregate-1", 1).expect("insert exact head");

    assert_batch_constraints(&connection);
    assert_record_constraints(&connection);
    assert_head_constraints(&connection);
    assert_restrictive_foreign_keys(&connection);
    drop((connection, fixture));
}

#[test]
fn v25_journal_count_ordinal_and_blob_bounds_are_exact() {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let connection = fixture.connection();
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .expect("enable foreign keys");
    insert_bounded_batch(&connection, "max-batch", "max-key", 256, 1_048_576)
        .expect("maximum batch bounds are accepted");
    insert_sized_record(&connection, "max-record", 255, 131_072, 131_072)
        .expect("maximum record bounds are accepted");
    insert_bounded_batch(&connection, "too-many", "too-many-key", 257, 2)
        .expect_err("record count over maximum is rejected");
    insert_bounded_batch(&connection, "too-large", "too-large-key", 1, 1_048_577)
        .expect_err("record set over maximum is rejected");
    insert_sized_record(&connection, "bad-ordinal", 256, 2, 2)
        .expect_err("ordinal over maximum is rejected");
    insert_sized_record(&connection, "bad-size", 1, 131_073, 131_073)
        .expect_err("record blob over maximum is rejected");
    insert_sized_record(&connection, "bad-length", 1, 2, 1)
        .expect_err("record byte mismatch is rejected");
    drop((connection, fixture));
}

#[test]
fn v25_public_list_filters_use_ordered_narrow_indexes() {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let connection = fixture.connection();
    assert_plan_uses(&connection, "aggregate_id='aggregate-1'", AGGREGATE_INDEX);
    assert_plan_uses(&connection, "record_kind='EvidenceRecord'", KIND_INDEX);
    drop((connection, fixture));
}

#[test]
fn v25_integrity_summary_uses_the_covering_aggregate_sequence_index() {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let connection = fixture.connection();
    let details = integrity_summary_plan(&connection);
    assert!(details.contains("USING COVERING INDEX"), "{details}");
    assert!(
        details.contains("record_kind=? AND aggregate_id=?"),
        "{details}"
    );
    drop((connection, fixture));
}

#[test]
fn malformed_canonical_v25_objects_are_rejected_without_repair() {
    assert_canonical_v25_rejected("malformed record-set bound", |connection| {
        let malformed = MIGRATE_V24_TO_V25_SQL.replacen("1048576", "1048575", 1);
        connection.execute_batch(&malformed)
    });
    assert_canonical_v25_rejected("missing appended index", |connection| {
        connection.execute_batch(MIGRATE_V24_TO_V25_SQL)?;
        connection.execute_batch(&format!("DROP INDEX {APPENDED_INDEX}"))
    });
    assert_canonical_v25_rejected("missing aggregate list index", |connection| {
        connection.execute_batch(MIGRATE_V24_TO_V25_SQL)?;
        connection.execute_batch(&format!("DROP INDEX {AGGREGATE_INDEX}"))
    });
    assert_canonical_v25_rejected("missing kind list index", |connection| {
        connection.execute_batch(MIGRATE_V24_TO_V25_SQL)?;
        connection.execute_batch(&format!("DROP INDEX {KIND_INDEX}"))
    });
    assert_canonical_v25_rejected("rogue object", |connection| {
        connection.execute_batch(MIGRATE_V24_TO_V25_SQL)?;
        connection.execute_batch("CREATE TABLE rogue_v25_table(id TEXT)")
    });
}

#[test]
fn final_validation_fault_rolls_v24_to_current_back_atomically() {
    let fixture = exact_v24_fixture();
    let connection = fixture.connection();
    let before_schema = schema_snapshot(&connection);
    let before_rows = schedule_rows(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), super::SCHEMA_VERSION);
        assert_journal_empty(migrated);
        migrated.execute_batch("CREATE TABLE rogue_v26_final_fault(id TEXT)")
    })
    .expect_err("final v26 validation rejects injected rogue object");

    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_eq!(schema_version(&connection), 24);
    assert_eq!(schema_snapshot(&connection), before_schema);
    assert_eq!(schedule_rows(&connection), before_rows);
    for object in [
        BATCHES,
        RECORDS,
        HEADS,
        APPENDED_INDEX,
        AGGREGATE_INDEX,
        KIND_INDEX,
        "rogue_v26_final_fault",
    ] {
        assert!(!schema_object_named(&connection, object), "{object}");
    }
    drop((connection, fixture));
}

fn exact_v24_fixture() -> super::sqlite_group_agent_graph_run_support::Fixture {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let connection = fixture.connection();
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
            "PRAGMA foreign_keys = ON;
             DROP TABLE governance_structural_heads;
             DROP TABLE governance_records;
             DROP TABLE governance_record_append_batches;
             PRAGMA user_version = 24;",
        )
        .expect("restore exact v24 schema");
    assert_eq!(schema_version(&connection), 24);
    drop(connection);
    fixture
}

fn assert_journal_empty(connection: &Connection) {
    for table in [BATCHES, RECORDS, HEADS] {
        assert_eq!(row_count(connection, table), 0, "{table}");
    }
}

fn schedule_rows(connection: &Connection) -> Vec<ScheduleRow> {
    let mut statement = connection
        .prepare(
            "SELECT id,graph_run_id,schedule_sha256,created_at_ms
             FROM group_agent_graph_execution_schedules ORDER BY id",
        )
        .expect("prepare schedule snapshot");
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query schedule snapshot")
        .collect::<Result<_, _>>()
        .expect("read schedule snapshot")
}

fn insert_batch(
    connection: &Connection,
    batch_id: &str,
    key: &str,
    count: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        &format!(
            "INSERT INTO {BATCHES}(
               batch_id,journal_version,idempotency_key,request_sha256,
               record_set_sha256,record_count,record_set_bytes,appended_at_ms
             ) VALUES(?1,1,?2,zeroblob(32),zeroblob(32),?3,2,10)"
        ),
        params![batch_id, key, count],
    )
}

fn insert_bounded_batch(
    connection: &Connection,
    batch_id: &str,
    key: &str,
    count: i64,
    bytes: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        &format!(
            "INSERT INTO {BATCHES} VALUES(
               ?1,1,?2,zeroblob(32),zeroblob(32),?3,?4,10
             )"
        ),
        params![batch_id, key, count, bytes],
    )
}

fn insert_record(
    connection: &Connection,
    record_id: &str,
    batch_id: &str,
    ordinal: i64,
    aggregate_id: &str,
    sequence: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        &format!(
            "INSERT INTO {RECORDS}(
               record_id,batch_id,batch_ordinal,record_kind,aggregate_id,sequence,
               canonical_sha256,canonical_record_blob,canonical_record_bytes,
               created_at_unix_ms,appended_at_ms
             ) VALUES(?1,?2,?3,'EvidenceRecord',?4,?5,zeroblob(32),x'7b7d',2,5,10)"
        ),
        params![record_id, batch_id, ordinal, aggregate_id, sequence],
    )
}

fn insert_head(
    connection: &Connection,
    record_id: &str,
    aggregate_id: &str,
    sequence: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        &format!(
            "INSERT INTO {HEADS}(
               record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
             ) VALUES('EvidenceRecord',?1,?2,?3,zeroblob(32),10)"
        ),
        params![aggregate_id, record_id, sequence],
    )
}

fn insert_sized_record(
    connection: &Connection,
    record_id: &str,
    ordinal: i64,
    blob_bytes: i64,
    declared_bytes: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        &format!(
            "INSERT INTO {RECORDS} VALUES(
               ?1,'max-batch',?2,'KnowledgeClaim',?1,?3,
               zeroblob(32),zeroblob(?4),?5,5,10
             )"
        ),
        params![record_id, ordinal, ordinal + 1, blob_bytes, declared_bytes],
    )
}

fn assert_batch_constraints(connection: &Connection) {
    insert_batch(connection, "batch-2", "append-key-1", 1).expect_err("idempotency key is unique");
    connection
        .execute(
            &format!(
                "INSERT INTO {BATCHES} VALUES(
                   'bad-version',2,'bad-version-key',zeroblob(32),zeroblob(32),1,2,10
                 )"
            ),
            [],
        )
        .expect_err("journal version is exact");
    insert_batch(connection, "empty-batch", "empty-key", 0).expect_err("record count is bounded");
}

fn assert_record_constraints(connection: &Connection) {
    insert_record(connection, "record-2", "batch-1", 0, "aggregate-2", 1)
        .expect_err("batch ordinal is unique");
    insert_record(connection, "record-3", "batch-1", 1, "aggregate-1", 1)
        .expect_err("aggregate sequence is unique");
    insert_record(connection, "record-4", "missing-batch", 1, "aggregate-4", 1)
        .expect_err("batch foreign key is required");
    connection
        .execute(
            &format!(
                "INSERT INTO {RECORDS} VALUES(
                   'bad-record','batch-1',1,'Other','aggregate-x',1,
                   zeroblob(32),x'7b7d',1,5,10
                 )"
            ),
            [],
        )
        .expect_err("kind and byte length are exact");
}

fn assert_head_constraints(connection: &Connection) {
    insert_head(connection, "missing-record", "aggregate-x", 1)
        .expect_err("head record foreign key is required");
    insert_head(connection, "record-1", "aggregate-2", 1)
        .expect_err("one record cannot head multiple aggregates");
    insert_record(connection, "record-2", "batch-1", 1, "aggregate-2", 1)
        .expect("insert second record");
    insert_head(connection, "record-2", "aggregate-1", 1)
        .expect_err("head aggregate key is unique");
}

fn assert_restrictive_foreign_keys(connection: &Connection) {
    connection
        .execute(
            &format!("DELETE FROM {BATCHES} WHERE batch_id='batch-1'"),
            [],
        )
        .expect_err("referenced batch cannot be deleted");
    connection
        .execute(
            &format!("DELETE FROM {RECORDS} WHERE record_id='record-1'"),
            [],
        )
        .expect_err("head record cannot be deleted");
}

fn assert_canonical_v25_rejected(
    name: &str,
    mutate: impl FnOnce(&Connection) -> rusqlite::Result<()>,
) {
    let fixture = exact_v24_fixture();
    let connection = fixture.connection();
    mutate(&connection).unwrap_or_else(|error| panic!("forge {name}: {error}"));
    assert_eq!(schema_version(&connection), 25);
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);

    let error = open_database(&fixture.database)
        .err()
        .unwrap_or_else(|| panic!("{name} unexpectedly opened"));
    assert!(
        matches!(error, HubStoreError::Corrupt { .. }),
        "{name}: {error:?}"
    );
    let unchanged = Connection::open(&fixture.database).expect("reopen rejected v25 fixture");
    assert_eq!(schema_version(&unchanged), 25, "{name}");
    assert_eq!(schema_snapshot(&unchanged), before, "{name}");
    drop((unchanged, fixture));
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}

fn assert_plan_uses(connection: &Connection, predicate: &str, index: &str) {
    let sql = format!(
        "EXPLAIN QUERY PLAN
         SELECT r.record_id,b.batch_id
         FROM governance_records r
         LEFT JOIN governance_record_append_batches b ON b.batch_id=r.batch_id
         WHERE {predicate}
         ORDER BY r.appended_at_ms DESC,r.record_id DESC LIMIT 20"
    );
    let mut statement = connection.prepare(&sql).expect("prepare list plan");
    let details = statement
        .query_map([], |row| row.get::<_, String>(3))
        .expect("query list plan")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect list plan")
        .join(" | ");
    assert!(details.contains(index), "{index}: {details}");
    assert!(
        !details.contains("USE TEMP B-TREE FOR ORDER BY"),
        "{details}"
    );
}

fn integrity_summary_plan(connection: &Connection) -> String {
    let mut statement = connection
        .prepare(
            "EXPLAIN QUERY PLAN
             SELECT COUNT(*),MIN(sequence),MAX(sequence) FROM governance_records
             WHERE record_kind=?1 AND aggregate_id=?2",
        )
        .expect("prepare integrity summary plan");
    statement
        .query_map(params!["EvidenceRecord", "aggregate-1"], |row| {
            row.get::<_, String>(3)
        })
        .expect("query integrity summary plan")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect integrity summary plan")
        .join(" | ")
}
