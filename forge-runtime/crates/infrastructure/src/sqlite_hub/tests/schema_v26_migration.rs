use rusqlite::Connection;
use serde::Deserialize;

use crate::SqliteHubStore;
use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{
    AppendGovernanceRecordBatch, GovernanceRecordAppendDisposition, GovernanceRecordAppendReceipt,
    GovernanceRecordJournalStore, GovernanceRecordKind, HubStoreError,
};

use super::{
    RESTORE_HISTORICAL_ANALYSES_SQL, open_database,
    open_existing_dispatch_preflight_read_only_database,
    open_existing_dispatch_reentry_read_only_database,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version, sqlite_group_agent_graph_execution_schedule_support,
};

const BATCHES: &str = "governance_record_append_batches";
const RECORDS: &str = "governance_records";
const HEADS: &str = "governance_structural_heads";
const GOVERNANCE_GOLDEN: &str =
    include_str!("../../../../../../docs/contracts/fixtures/governance-evidence-claim-v1.json");

#[derive(Deserialize)]
struct GoldenFixture {
    records: Vec<GoldenEntry>,
}

#[derive(Deserialize)]
struct GoldenEntry {
    record: GovernanceRecord,
}

#[test]
fn canonical_and_endpoint_only_v25_lineages_converge_on_current() {
    let (canonical, original_receipt) = canonical_v25_fixture();
    let endpoint_only = endpoint_only_v25_fixture();

    let canonical_current =
        open_database(&canonical.database).expect("migrate canonical v25 to current");
    let endpoint_current =
        open_database(&endpoint_only.database).expect("migrate endpoint-only v25 to current");

    assert_eq!(schema_version(&canonical_current), super::SCHEMA_VERSION);
    assert_eq!(schema_version(&endpoint_current), super::SCHEMA_VERSION);
    assert_eq!(
        schema_snapshot(&canonical_current),
        schema_snapshot(&endpoint_current)
    );
    assert_endpoint_relaxed(&canonical_current);
    for table in [BATCHES, RECORDS, HEADS] {
        assert!(schema_object_named(&endpoint_current, table), "{table}");
    }
    drop(canonical_current);

    let canonical_reopened = open_database(&canonical.database).expect("reopen migrated current");
    assert_eq!(schema_version(&canonical_reopened), super::SCHEMA_VERSION);
    drop(canonical_reopened);

    assert_canonical_journal_survives(&canonical.store, &original_receipt);
    drop((endpoint_current, canonical, endpoint_only));
}

#[test]
fn malformed_endpoint_only_v25_is_rejected_without_repair() {
    let fixture = endpoint_only_v25_fixture();
    let connection = fixture.connection();
    connection
        .execute_batch("CREATE TABLE rogue_endpoint_only_v25(id TEXT)")
        .expect("install rogue object");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);

    let error = open_database(&fixture.database)
        .expect_err("malformed endpoint-only v25 must not be repaired");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error}");

    let unchanged = Connection::open(&fixture.database).expect("reopen rejected fixture");
    assert_eq!(schema_version(&unchanged), 25);
    assert_eq!(schema_snapshot(&unchanged), before);
    drop((unchanged, fixture));
}

#[test]
fn effect_free_dispatch_readers_accept_endpoint_only_v25() {
    let fixture = endpoint_only_v25_fixture();

    let preflight = open_existing_dispatch_preflight_read_only_database(&fixture.database)
        .expect("preflight reads endpoint-only v25");
    assert_eq!(schema_version(&preflight), 25);
    drop(preflight);

    let writer = Connection::open(&fixture.database).expect("open endpoint-only v25 writer");
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA wal_autocheckpoint = 0;")
        .expect("prepare hot endpoint-only v25 WAL");
    writer
        .execute(
            "INSERT INTO groups(id,name,idempotency_key,created_at_ms)
             VALUES(?1,?2,?3,?4)",
            ("v25-hot-group", "Hot v25 group", "v25-hot-group-key", 1_i64),
        )
        .expect("commit endpoint-only v25 hot-WAL row");
    let reentry = open_existing_dispatch_reentry_read_only_database(&fixture.database)
        .expect("re-entry reads endpoint-only v25");
    assert_eq!(schema_version(&reentry), 25);
    let group_count: i64 = reentry
        .query_row(
            "SELECT COUNT(*) FROM groups WHERE id = 'v25-hot-group'",
            [],
            |row| row.get(0),
        )
        .expect("read endpoint-only v25 hot-WAL row");
    assert_eq!(group_count, 1);
    drop((reentry, writer, fixture));
}

fn canonical_v25_fixture() -> (
    super::sqlite_group_agent_graph_run_support::Fixture,
    GovernanceRecordAppendReceipt,
) {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let stored = fixture
        .store
        .append_governance_record_batch(&canonical_journal_request(100))
        .expect("append valid canonical v25 journal batch");
    assert_eq!(
        stored.disposition,
        GovernanceRecordAppendDisposition::Stored
    );
    let connection = fixture.connection();
    connection
        .execute_batch(super::DROP_V27_SEMANTIC_VIEW_SQL)
        .expect("drop v27 semantic projection");
    connection
        .execute_batch(RESTORE_HISTORICAL_ANALYSES_SQL)
        .expect("restore canonical v25 endpoint definitions");
    connection
        .execute_batch("PRAGMA user_version = 25")
        .expect("stamp canonical v25 fixture");
    assert_eq!(schema_version(&connection), 25);
    drop(connection);
    (fixture, stored.receipt)
}

fn canonical_journal_request(appended_at_ms: u64) -> AppendGovernanceRecordBatch {
    let fixture: GoldenFixture =
        serde_json::from_str(GOVERNANCE_GOLDEN).expect("governance golden fixture");
    let mut records: Vec<_> = fixture
        .records
        .into_iter()
        .map(|entry| entry.record)
        .collect();
    records.sort_by(|left, right| {
        left.metadata()
            .record_id
            .as_bytes()
            .cmp(right.metadata().record_id.as_bytes())
    });
    let canonical: Vec<_> = records
        .iter()
        .map(|record| record.canonical_record_json().expect("canonical record"))
        .collect();
    AppendGovernanceRecordBatch::from_canonical_record_set(
        format!("[{}]", canonical.join(",")),
        "v25-v26-preservation".into(),
        appended_at_ms,
    )
    .expect("valid journal request")
}

fn assert_canonical_journal_survives(
    store: &SqliteHubStore,
    original_receipt: &GovernanceRecordAppendReceipt,
) {
    let replay = store
        .append_governance_record_batch(&canonical_journal_request(999))
        .expect("replay canonical journal after v26 reopen");
    assert_eq!(
        replay.disposition,
        GovernanceRecordAppendDisposition::ExactReplay
    );
    assert_eq!(&replay.receipt, original_receipt);
    let inspection = store
        .inspect_governance_record("evr-0001", true)
        .expect("inspect migrated canonical record");
    assert!(inspection.canonical_record_json.is_some());
    let head = store
        .inspect_governance_structural_head(
            GovernanceRecordKind::EvidenceRecord,
            "evidence-check-pass",
        )
        .expect("inspect migrated structural head");
    assert_eq!(head.record_id, "evr-0001");
}

fn endpoint_only_v25_fixture() -> super::sqlite_group_agent_graph_run_support::Fixture {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let connection = fixture.connection();
    connection
        .execute_batch(super::DROP_V27_SEMANTIC_VIEW_SQL)
        .expect("drop v27 semantic projection");
    connection
        .execute_batch(&format!(
            "DROP TABLE {HEADS}; DROP TABLE {RECORDS}; DROP TABLE {BATCHES};
             PRAGMA user_version = 25;"
        ))
        .expect("restore endpoint-only v25 fixture");
    assert_eq!(schema_version(&connection), 25);
    assert_endpoint_relaxed(&connection);
    drop(connection);
    fixture
}

fn assert_endpoint_relaxed(connection: &Connection) {
    let definition: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name='group_model_analyses'",
            [],
            |row| row.get(0),
        )
        .expect("read analyses definition");
    assert!(
        definition.contains("endpoint LIKE 'http://%'"),
        "{definition}"
    );
}
