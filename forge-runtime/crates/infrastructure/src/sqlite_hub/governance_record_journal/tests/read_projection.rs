use rusqlite::params;

use crate::runtime_domain::{
    GovernanceRecordJournalStore, GovernanceRecordKind, GovernanceRecordListFilter, HubStoreError,
    MAX_GOVERNANCE_RECORD_LIST_LIMIT,
};

use super::fixtures::{
    StoreFixture, checkpoint, evidence_successor, golden_records, request, reseal, row_count,
};

#[test]
fn metadata_content_list_order_and_filters_are_exact() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "read-evidence", 100);
    append(&fixture, &records[1..], "read-claim", 200);

    let metadata = fixture
        .store
        .inspect_governance_record("evr-0001", false)
        .expect("inspect metadata");
    assert!(metadata.canonical_record_json.is_none());
    let revealed = fixture
        .store
        .inspect_governance_record("evr-0001", true)
        .expect("reveal exact record");
    assert_eq!(
        revealed.canonical_record_json.as_deref(),
        Some(
            records[0]
                .canonical_record_json()
                .expect("canonical")
                .as_str()
        )
    );

    let listed = fixture
        .store
        .list_governance_records(&filter(None, None, 10, false))
        .expect("list newest first");
    assert_eq!(listed[0].metadata.record_id, "kcr-0001");
    assert_eq!(listed[1].metadata.record_id, "evr-0001");
    let evidence = fixture
        .store
        .list_governance_records(&filter(
            Some(GovernanceRecordKind::EvidenceRecord),
            Some("evidence-check-pass"),
            1,
            true,
        ))
        .expect("filtered reveal");
    assert_eq!(evidence.len(), 1);
    assert!(evidence[0].canonical_record_json.is_some());
}

#[test]
fn integrity_validating_list_keeps_the_public_hundred_record_bound() {
    let fixture = StoreFixture::new();
    let error = fixture
        .store
        .list_governance_records(&filter(
            None,
            None,
            MAX_GOVERNANCE_RECORD_LIST_LIMIT + 1,
            false,
        ))
        .expect_err("list over the public bound is rejected before reads");
    assert!(matches!(error, HubStoreError::Conflict { .. }), "{error:?}");
}

#[test]
fn metadata_only_show_and_list_fail_closed_on_corrupt_record_bytes() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "metadata-only", 100);
    fixture
        .connection()
        .execute(
            "UPDATE governance_records SET canonical_record_blob=zeroblob(canonical_record_bytes)
             WHERE record_id='evr-0001'",
            [],
        )
        .expect("corrupt only exact content");

    let show_error = fixture
        .store
        .inspect_governance_record("evr-0001", false)
        .expect_err("metadata-only show validates exact content");
    assert!(
        matches!(show_error, HubStoreError::Corrupt { .. }),
        "{show_error:?}"
    );
    let list_error = fixture
        .store
        .list_governance_records(&filter(None, None, 10, false))
        .expect_err("metadata-only list validates exact content");
    assert!(
        matches!(list_error, HubStoreError::Corrupt { .. }),
        "{list_error:?}"
    );
}

#[test]
fn default_show_and_list_reject_batch_request_and_set_digest_corruption() {
    assert_default_reads_reject_batch_digest("request_sha256=zeroblob(32)");
    assert_default_reads_reject_batch_digest("record_set_sha256=zeroblob(32)");
}

#[test]
fn metadata_only_show_and_list_validate_unreturned_owning_batch_siblings() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "metadata-sibling", 100);
    fixture
        .connection()
        .execute(
            "UPDATE governance_records SET canonical_record_blob=zeroblob(canonical_record_bytes)
             WHERE record_id='evr-0001'",
            [],
        )
        .expect("corrupt unreturned batch sibling");
    let error = fixture
        .store
        .inspect_governance_record("kcr-0001", false)
        .expect_err("show validates the complete owning batch");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    let error = fixture
        .store
        .list_governance_records(&filter(None, None, 1, false))
        .expect_err("bounded list validates unreturned batch siblings");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

#[test]
fn metadata_only_read_rejects_batch_ordinal_outside_owning_count() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "metadata-ordinal", 100);
    fixture
        .connection()
        .execute(
            "UPDATE governance_records SET batch_ordinal=2 WHERE record_id='kcr-0001'",
            [],
        )
        .expect("diverge ordinal from owning batch count");
    let error = fixture
        .store
        .inspect_governance_record("kcr-0001", false)
        .expect_err("metadata-only read validates owning count");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

#[test]
fn current_read_only_store_reads_journal_and_rejects_append() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "read-only", 100);
    checkpoint(&fixture.path);
    let read_only =
        super::super::super::SqliteHubStore::open_existing_current_read_only(&fixture.path)
            .expect("open journal read-only");
    read_only
        .inspect_governance_record("evr-0001", false)
        .expect("read journal without effects");
    let error = read_only
        .append_governance_record_batch(&request(&records[1..], "forbidden", 200))
        .expect_err("read-only journal cannot append");
    assert!(
        matches!(error, HubStoreError::Unavailable { .. }),
        "{error:?}"
    );
}

#[test]
fn rebuild_repairs_projection_and_rolls_back_on_relational_corruption() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "rebuild", 100);
    fixture
        .connection()
        .execute("DELETE FROM governance_structural_heads", [])
        .expect("drop rebuildable projection");
    assert_eq!(
        fixture
            .store
            .rebuild_governance_structural_heads()
            .expect("rebuild"),
        2
    );
    corrupt_claim_reference(&fixture, &records[1]);
    let before = head_rows(&fixture);
    let error = fixture
        .store
        .rebuild_governance_structural_heads()
        .expect_err("relational corruption aborts rebuild");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(head_rows(&fixture), before);
}

#[test]
fn rebuild_rejects_zero_row_and_cross_aggregate_incomplete_batches() {
    let zero = StoreFixture::new();
    let records = golden_records();
    append(&zero, &records[..1], "zero-row-batch", 100);
    zero.connection()
        .execute_batch(
            "DELETE FROM governance_claim_validation_jobs;
             DELETE FROM governance_claim_semantic_views;
             DELETE FROM governance_semantic_heads;
             DELETE FROM governance_structural_heads;
             DELETE FROM governance_records;",
        )
        .expect("remove the only durable record");
    assert_rebuild_corrupt_without_projection_change(&zero);

    let partial = StoreFixture::new();
    append(&partial, &records, "partial-batch", 100);
    partial
        .connection()
        .execute_batch(
            "DELETE FROM governance_claim_semantic_views WHERE record_id='kcr-0001';
             DELETE FROM governance_semantic_heads WHERE record_id='kcr-0001';
             DELETE FROM governance_structural_heads WHERE record_id='kcr-0001';
             DELETE FROM governance_records WHERE record_id='kcr-0001';",
        )
        .expect("remove one cross-aggregate batch record");
    assert_rebuild_corrupt_without_projection_change(&partial);
}

#[test]
fn rebuild_rejects_batch_count_and_digest_divergence() {
    assert_rebuild_batch_column_corrupt("record_count=1");
    assert_rebuild_batch_column_corrupt("request_sha256=zeroblob(32)");
}

#[test]
fn head_revalidates_exact_record_content_without_revealing_it() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "head-content", 100);
    fixture
        .connection()
        .execute_batch(
            "UPDATE governance_records SET canonical_sha256=zeroblob(32) WHERE record_id='evr-0001';
             UPDATE governance_structural_heads SET canonical_sha256=zeroblob(32)
             WHERE record_id='evr-0001';",
        )
        .expect("coordinate digest column corruption");
    let error = fixture
        .store
        .inspect_governance_structural_head(
            GovernanceRecordKind::EvidenceRecord,
            "evidence-check-pass",
        )
        .expect_err("head revalidates exact blob digest");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

#[test]
fn head_revalidates_request_digest_and_unreturned_batch_siblings() {
    assert_head_rejects_batch_corruption(
        "UPDATE governance_record_append_batches SET request_sha256=zeroblob(32)",
    );
    assert_head_rejects_batch_corruption(
        "UPDATE governance_records SET canonical_record_blob=zeroblob(canonical_record_bytes)
         WHERE record_id='kcr-0001'",
    );
}

#[test]
fn corrupt_head_owning_batch_blocks_successor_append_without_writes() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "head-successor-base", 100);
    fixture
        .connection()
        .execute(
            "UPDATE governance_records SET canonical_record_blob=zeroblob(canonical_record_bytes)
             WHERE record_id='kcr-0001'",
            [],
        )
        .expect("corrupt unreturned head batch sibling");
    let successor = evidence_successor(&records[0], "evr-head-successor", 2);
    let before = row_count(&fixture.connection(), "governance_records");
    let error = fixture
        .store
        .append_governance_record_batch(&request(&[successor], "head-successor", 200))
        .expect_err("head batch corruption precedes successor validation");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(
        row_count(&fixture.connection(), "governance_records"),
        before
    );
}

fn assert_head_rejects_batch_corruption(sql: &str) {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "head-batch-corrupt", 100);
    fixture
        .connection()
        .execute(sql, [])
        .expect("corrupt head owning batch");
    let error = fixture
        .store
        .inspect_governance_structural_head(
            GovernanceRecordKind::EvidenceRecord,
            "evidence-check-pass",
        )
        .expect_err("head validates its complete owning batch");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

fn append(
    fixture: &StoreFixture,
    records: &[crate::runtime_domain::governance_contract::GovernanceRecord],
    key: &str,
    time: u64,
) {
    fixture
        .store
        .append_governance_record_batch(&request(records, key, time))
        .expect("append read fixture");
}

fn filter(
    kind: Option<GovernanceRecordKind>,
    aggregate_id: Option<&str>,
    limit: usize,
    include_record: bool,
) -> GovernanceRecordListFilter {
    GovernanceRecordListFilter {
        record_kind: kind,
        aggregate_id: aggregate_id.map(str::to_owned),
        limit,
        include_record,
    }
}

fn corrupt_claim_reference(
    fixture: &StoreFixture,
    template: &crate::runtime_domain::governance_contract::GovernanceRecord,
) {
    let mut corrupted = template.clone();
    let crate::runtime_domain::governance_contract::GovernanceRecord::Claim(claim) = &mut corrupted
    else {
        panic!("expected claim");
    };
    claim.spec.supporting_evidence_record_ids = vec!["evr-missing".into()];
    reseal(&mut corrupted);
    let canonical = corrupted
        .canonical_record_json()
        .expect("corrupt canonical");
    let digest = super::super::super::group_run_codec::decode_hex_digest(
        &corrupted.integrity().canonical_sha256,
    )
    .expect("corrupt digest bytes");
    fixture
        .connection()
        .execute(
            "UPDATE governance_records SET canonical_record_blob=?1,canonical_record_bytes=?2,
             canonical_sha256=?3 WHERE record_id='kcr-0001'",
            params![
                canonical.as_bytes(),
                i64::try_from(canonical.len()).expect("canonical length fits SQLite"),
                digest.as_slice()
            ],
        )
        .expect("install relationally corrupt record");
}

fn head_rows(fixture: &StoreFixture) -> Vec<(String, String, String, i64, Vec<u8>, i64)> {
    let connection = fixture.connection();
    let mut statement = connection
        .prepare(
            "SELECT record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
             FROM governance_structural_heads ORDER BY record_kind,aggregate_id",
        )
        .expect("prepare head snapshot");
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .expect("query heads")
        .collect::<Result<_, _>>()
        .expect("head snapshot")
}

fn assert_rebuild_batch_column_corrupt(assignment: &str) {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "batch-column-corrupt", 100);
    fixture
        .connection()
        .execute(
            &format!("UPDATE governance_record_append_batches SET {assignment}"),
            [],
        )
        .expect("corrupt durable batch column");
    assert_rebuild_corrupt_without_projection_change(&fixture);
}

fn assert_rebuild_corrupt_without_projection_change(fixture: &StoreFixture) {
    let before = head_rows(fixture);
    let error = fixture
        .store
        .rebuild_governance_structural_heads()
        .expect_err("inexact durable batch aborts rebuild");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(head_rows(fixture), before);
}

fn assert_default_reads_reject_batch_digest(assignment: &str) {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "default-batch-digest", 100);
    fixture
        .connection()
        .execute(
            &format!("UPDATE governance_record_append_batches SET {assignment}"),
            [],
        )
        .expect("corrupt batch digest");
    let show = fixture
        .store
        .inspect_governance_record("evr-0001", false)
        .expect_err("default show validates batch digest");
    let list = fixture
        .store
        .list_governance_records(&filter(None, None, 10, false))
        .expect_err("default list validates batch digest");
    assert!(matches!(show, HubStoreError::Corrupt { .. }), "{show:?}");
    assert!(matches!(list, HubStoreError::Corrupt { .. }), "{list:?}");
}
