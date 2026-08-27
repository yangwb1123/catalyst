use serde::Deserialize;

use rusqlite::params;

use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{
    AppendGovernanceRecordBatch, GovernanceRecordJournalStore, GovernanceRecordKind,
    GovernanceSemanticViewStore, HubStoreError,
};

use super::{
    migrate_with_before_final_fault_for_test, open_database,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version, sqlite_group_agent_graph_execution_schedule_support,
};

const GOLDEN: &str =
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
fn populated_v26_journal_is_backfilled_and_reopens_as_current() {
    let fixture = populated_v26_fixture();
    let migrated = open_database(&fixture.database).expect("migrate populated v26 to current");
    assert_eq!(schema_version(&migrated), super::SCHEMA_VERSION);
    assert_eq!(row_count(&migrated, "governance_semantic_heads"), 2);
    assert_eq!(row_count(&migrated, "governance_claim_semantic_views"), 1);
    assert_eq!(row_count(&migrated, "run_lineages"), 0);
    drop(migrated);

    let reopened = open_database(&fixture.database).expect("reopen current Hub");
    assert_eq!(schema_version(&reopened), super::SCHEMA_VERSION);
    drop(reopened);
    let projection = fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::KnowledgeClaim,
            "claim-harness-check",
        )
        .expect("read backfilled semantic projection");
    assert_eq!(projection.head.record_id, "kcr-0001");
}

#[test]
fn final_validation_failure_rolls_v26_schema_and_backfill_back_atomically() {
    let fixture = populated_v26_fixture();
    let connection = fixture.connection();
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), super::SCHEMA_VERSION);
        assert_eq!(row_count(migrated, "governance_semantic_heads"), 2);
        assert_eq!(row_count(migrated, "run_lineages"), 0);
        migrated.execute_batch("CREATE TABLE rogue_current_final_fault(id TEXT)")
    })
    .expect_err("final current validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(schema_version(&connection), 26);
    assert_eq!(schema_snapshot(&connection), before);
    for object in [
        "governance_semantic_heads",
        "governance_claim_semantic_views",
        "governance_claim_validation_jobs",
        "run_lineages",
        "rogue_current_final_fault",
    ] {
        assert!(!schema_object_named(&connection, object), "{object}");
    }
}

#[test]
fn relation_backfill_failure_rolls_the_complete_v27_migration_back() {
    let fixture = populated_v26_fixture();
    let mut changed = records();
    let GovernanceRecord::Claim(claim) = &mut changed[1] else {
        unreachable!()
    };
    claim.spec.derived_from_claim_record_ids = vec!["kcr-missing-dependency".into()];
    reseal(&mut changed[1]);
    changed[1]
        .validate()
        .expect("unresolved relation remains individually valid before v27");
    rewrite_v26_batch(&fixture, &changed);

    let connection = fixture.connection();
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);
    let error = open_database(&fixture.database)
        .expect_err("v27 backfill must reject an unresolved Claim relation");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");

    let connection = fixture.connection();
    assert_eq!(schema_version(&connection), 26);
    assert_eq!(schema_snapshot(&connection), before);
    for object in [
        "governance_semantic_heads",
        "governance_claim_semantic_views",
        "governance_claim_validation_jobs",
    ] {
        assert!(!schema_object_named(&connection, object), "{object}");
    }
}

fn populated_v26_fixture() -> super::sqlite_group_agent_graph_run_support::Fixture {
    let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
    let records = records();
    fixture
        .store
        .append_governance_record_batch(&journal_request(&records))
        .expect("append v27 migration journal fixture");
    let connection = fixture.connection();
    connection
        .execute_batch(super::DROP_V29_CONTROLLER_SQL)
        .expect("remove v29 controller journal");
    connection
        .execute_batch(super::DROP_V28_LINEAGE_SQL)
        .expect("remove v28 Run lineage table");
    connection
        .execute_batch(super::DROP_V27_SEMANTIC_VIEW_SQL)
        .expect("remove v27 projection tables");
    connection
        .execute_batch("PRAGMA user_version=26")
        .expect("stamp exact v26 fixture");
    assert_eq!(schema_version(&connection), 26);
    drop(connection);
    fixture
}

fn records() -> Vec<GovernanceRecord> {
    serde_json::from_str::<GoldenFixture>(GOLDEN)
        .expect("golden fixture")
        .records
        .into_iter()
        .map(|entry| entry.record)
        .collect()
}

fn journal_request(records: &[GovernanceRecord]) -> AppendGovernanceRecordBatch {
    let mut canonical: Vec<_> = records
        .iter()
        .map(|record| {
            (
                record.metadata().record_id.as_str(),
                record.canonical_record_json().expect("canonical record"),
            )
        })
        .collect();
    canonical.sort_by_key(|(record_id, _)| *record_id);
    AppendGovernanceRecordBatch::from_canonical_record_set(
        format!(
            "[{}]",
            canonical
                .into_iter()
                .map(|(_, record)| record)
                .collect::<Vec<_>>()
                .join(",")
        ),
        "v26-v27-backfill".into(),
        100,
    )
    .expect("valid migration append request")
}

fn rewrite_v26_batch(
    fixture: &super::sqlite_group_agent_graph_run_support::Fixture,
    changed_records: &[GovernanceRecord],
) {
    let original_records = records();
    let prior = journal_request(&original_records);
    let replacement = journal_request(changed_records);
    let claim = &changed_records[1];
    let canonical = claim
        .canonical_record_json()
        .expect("canonical changed claim");
    let connection = fixture.connection();
    connection
        .execute_batch("PRAGMA foreign_keys=OFF")
        .expect("disable foreign keys for coherent v26 rewrite");
    rewrite_batch_identity(&connection, &prior, &replacement);
    rewrite_claim_identity(&connection, claim, &canonical);
    connection
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA foreign_key_check")
        .expect("rewritten v26 fixture remains referentially complete");
}

fn rewrite_batch_identity(
    connection: &rusqlite::Connection,
    prior: &AppendGovernanceRecordBatch,
    replacement: &AppendGovernanceRecordBatch,
) {
    connection
        .execute(
            "UPDATE governance_records SET batch_id=?1 WHERE batch_id=?2",
            params![replacement.batch_id, prior.batch_id],
        )
        .expect("move exact records to replacement batch");
    connection
        .execute(
            "UPDATE governance_record_append_batches SET batch_id=?1,request_sha256=?2,
             record_set_sha256=?3,record_set_bytes=?4 WHERE batch_id=?5",
            params![
                replacement.batch_id,
                digest_bytes(&replacement.request_sha256),
                digest_bytes(&replacement.record_set_sha256),
                i64::try_from(replacement.canonical_record_set_json.len())
                    .expect("record set bytes fit SQLite"),
                prior.batch_id,
            ],
        )
        .expect("rewrite exact batch identity");
}

fn rewrite_claim_identity(
    connection: &rusqlite::Connection,
    claim: &GovernanceRecord,
    canonical: &str,
) {
    connection
        .execute(
            "UPDATE governance_records SET canonical_record_blob=?1,
             canonical_record_bytes=?2,canonical_sha256=?3 WHERE record_id=?4",
            params![
                canonical.as_bytes(),
                i64::try_from(canonical.len()).expect("claim bytes fit SQLite"),
                digest_bytes(&claim.integrity().canonical_sha256),
                claim.metadata().record_id,
            ],
        )
        .expect("rewrite exact claim bytes");
    connection
        .execute(
            "UPDATE governance_structural_heads SET canonical_sha256=?1
             WHERE record_kind='KnowledgeClaim' AND aggregate_id=?2",
            params![
                digest_bytes(&claim.integrity().canonical_sha256),
                claim.metadata().aggregate_id,
            ],
        )
        .expect("rewrite structural head digest");
}

fn reseal(record: &mut GovernanceRecord) {
    match record {
        GovernanceRecord::Evidence(value) => value.integrity.canonical_sha256.clear(),
        GovernanceRecord::Claim(value) => value.integrity.canonical_sha256.clear(),
    }
    let digest = record.expected_sha256().expect("changed claim digest");
    match record {
        GovernanceRecord::Evidence(value) => value.integrity.canonical_sha256 = digest,
        GovernanceRecord::Claim(value) => value.integrity.canonical_sha256 = digest,
    }
}

fn digest_bytes(value: &str) -> Vec<u8> {
    assert_eq!(value.len(), 64, "fixture digest length");
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("digest is ASCII");
            u8::from_str_radix(text, 16).expect("digest is lowercase hex")
        })
        .collect()
}

fn row_count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
