use std::path::Path;
use std::sync::{Arc, Barrier};

use crate::runtime_domain::{
    AppendGovernanceRecordBatch, AppendGovernanceRecordBatchResult,
    GovernanceRecordAppendDisposition, GovernanceRecordJournalStore, GovernanceRecordKind,
    HubStoreError,
};

use super::fixtures::{
    StoreFixture, change_record_id, evidence_successor, golden_records, request, reseal, row_count,
};

#[test]
fn append_is_durable_and_exact_replay_keeps_the_original_receipt() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    let first = request(&records, "append-replay", 100);
    let stored = fixture
        .store
        .append_governance_record_batch(&first)
        .expect("store exact batch");
    assert_eq!(
        stored.disposition,
        GovernanceRecordAppendDisposition::Stored
    );
    assert_eq!(stored.receipt.record_count, 2);

    let retry = request(&records, "append-replay", 999);
    let replay = fixture
        .store
        .append_governance_record_batch(&retry)
        .expect("replay exact batch");
    assert_eq!(
        replay.disposition,
        GovernanceRecordAppendDisposition::ExactReplay
    );
    assert_eq!(replay.receipt, stored.receipt);
    assert_counts(&fixture, 1, 2, 2);
}

#[test]
fn concurrent_exact_append_stores_once_and_every_loser_replays() {
    let fixture = StoreFixture::new();
    let exact = request(&golden_records()[..1], "concurrent-exact", 100);
    let results = append_concurrently(&fixture.path, vec![exact; 8]);
    let mut stored = Vec::new();
    let mut replayed = Vec::new();
    for result in results {
        let result = result.expect("exact contender succeeds");
        match result.disposition {
            GovernanceRecordAppendDisposition::Stored => stored.push(result.receipt),
            GovernanceRecordAppendDisposition::ExactReplay => replayed.push(result.receipt),
        }
    }
    assert_eq!(stored.len(), 1);
    assert_eq!(replayed.len(), 7);
    assert!(replayed.iter().all(|receipt| receipt == &stored[0]));
    assert_counts(&fixture, 1, 1, 1);
    assert_no_partial_batch(&fixture);
}

#[test]
fn concurrent_same_sequence_with_different_identity_has_one_winner() {
    let fixture = StoreFixture::new();
    let first = golden_records().remove(0);
    let mut competing = first.clone();
    change_record_id(&mut competing, "evr-concurrent-sequence");
    let requests = vec![
        request(&[first], "concurrent-sequence-a", 100),
        request(&[competing], "concurrent-sequence-b", 200),
    ];
    let results = append_concurrently(&fixture.path, requests);
    assert_one_stored_one_conflict(&results);
    assert_counts(&fixture, 1, 1, 1);
    assert_no_partial_batch(&fixture);
}

#[test]
fn idempotency_and_immutable_identity_conflicts_write_nothing() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    let first = request(&records[..1], "identity-one", 100);
    fixture
        .store
        .append_governance_record_batch(&first)
        .expect("store initial evidence");

    let mut changed = records[0].clone();
    let crate::runtime_domain::governance_contract::GovernanceRecord::Evidence(evidence) =
        &mut changed
    else {
        panic!("expected evidence");
    };
    evidence.metadata.source_revision = "changed-revision".into();
    reseal(&mut changed);
    assert_conflict(&fixture, &request(&[changed], "identity-one", 200));
    assert_conflict(&fixture, &request(&records[..1], "identity-two", 200));

    let mut tuple_collision = records[0].clone();
    change_record_id(&mut tuple_collision, "evr-tuple-collision");
    assert_conflict(
        &fixture,
        &request(&[tuple_collision], "identity-three", 200),
    );
    assert_counts(&fixture, 1, 1, 1);
}

#[test]
fn sqlite_failure_after_batch_insert_rolls_the_whole_append_back() {
    let fixture = StoreFixture::new();
    let mut connection = fixture.connection();
    connection
        .execute_batch(
            "CREATE TEMP TRIGGER force_claim_failure BEFORE INSERT ON governance_records
             WHEN NEW.record_id='kcr-0001' BEGIN SELECT RAISE(ABORT,'forced failure'); END;",
        )
        .expect("install connection-local failure");
    let append = request(&golden_records(), "forced-rollback", 100);

    super::super::write::append(&mut connection, &append)
        .expect_err("forced second-record failure rolls back");
    assert_eq!(
        row_count(&connection, "governance_record_append_batches"),
        0
    );
    assert_eq!(row_count(&connection, "governance_records"), 0);
    assert_eq!(row_count(&connection, "governance_structural_heads"), 0);
}

#[test]
fn exact_replay_fails_closed_for_missing_stale_or_corrupt_projection() {
    replay_with_missing_head_is_corrupt();
    replay_with_stale_head_is_corrupt();
    replay_with_divergent_head_is_corrupt();
}

#[test]
fn exact_replay_revalidates_request_digest_and_batch_siblings() {
    assert_replay_rejects_batch_corruption(
        "UPDATE governance_record_append_batches SET record_set_sha256=zeroblob(32)",
    );
    assert_replay_rejects_batch_corruption(
        "UPDATE governance_records SET canonical_record_blob=zeroblob(canonical_record_bytes)
         WHERE record_id='kcr-0001'",
    );
}

fn assert_replay_rejects_batch_corruption(sql: &str) {
    let fixture = StoreFixture::new();
    let records = golden_records();
    let append = request(&records, "replay-batch-corrupt", 100);
    fixture
        .store
        .append_governance_record_batch(&append)
        .expect("append replay batch fixture");
    fixture
        .connection()
        .execute(sql, [])
        .expect("corrupt replay owning batch");
    assert_replay_corrupt(&fixture, &append);
    assert_counts(&fixture, 1, 2, 2);
}

fn replay_with_missing_head_is_corrupt() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    let append = request(&records[..1], "missing-replay-head", 100);
    fixture
        .store
        .append_governance_record_batch(&append)
        .expect("append replay fixture");
    fixture
        .connection()
        .execute("DELETE FROM governance_structural_heads", [])
        .expect("remove projection head");
    assert_replay_corrupt(&fixture, &append);
}

fn replay_with_stale_head_is_corrupt() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    let first = request(&records[..1], "stale-replay-head", 100);
    fixture
        .store
        .append_governance_record_batch(&first)
        .expect("append replay fixture");
    let second = evidence_successor(&records[0], "evr-0002", 2);
    fixture
        .store
        .append_governance_record_batch(&request(&[second], "advance-replay-head", 200))
        .expect("advance projection head");
    fixture
        .connection()
        .execute(
            "UPDATE governance_structural_heads SET record_id='evr-0001',sequence=1,
             canonical_sha256=(SELECT canonical_sha256 FROM governance_records WHERE record_id='evr-0001')
             WHERE record_kind=?1 AND aggregate_id='evidence-check-pass'",
            [GovernanceRecordKind::EvidenceRecord.as_str()],
        )
        .expect("make replay projection stale");
    assert_replay_corrupt(&fixture, &first);
}

fn replay_with_divergent_head_is_corrupt() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    let append = request(&records[..1], "corrupt-replay-head", 100);
    fixture
        .store
        .append_governance_record_batch(&append)
        .expect("append replay fixture");
    fixture
        .connection()
        .execute(
            "UPDATE governance_structural_heads SET canonical_sha256=zeroblob(32)",
            [],
        )
        .expect("diverge projection digest");
    assert_replay_corrupt(&fixture, &append);
}

fn assert_replay_corrupt(
    fixture: &StoreFixture,
    append: &crate::runtime_domain::AppendGovernanceRecordBatch,
) {
    let error = fixture
        .store
        .append_governance_record_batch(append)
        .expect_err("projection corruption blocks replay");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

fn assert_conflict(
    fixture: &StoreFixture,
    append: &crate::runtime_domain::AppendGovernanceRecordBatch,
) {
    let error = fixture
        .store
        .append_governance_record_batch(append)
        .expect_err("append must conflict");
    assert!(matches!(error, HubStoreError::Conflict { .. }), "{error:?}");
}

fn assert_counts(fixture: &StoreFixture, batches: i64, records: i64, heads: i64) {
    let connection = fixture.connection();
    assert_eq!(
        row_count(&connection, "governance_record_append_batches"),
        batches
    );
    assert_eq!(row_count(&connection, "governance_records"), records);
    assert_eq!(row_count(&connection, "governance_structural_heads"), heads);
}

fn append_concurrently(
    path: &Path,
    requests: Vec<AppendGovernanceRecordBatch>,
) -> Vec<Result<AppendGovernanceRecordBatchResult, HubStoreError>> {
    let barrier = Arc::new(Barrier::new(requests.len()));
    let handles: Vec<_> = requests
        .into_iter()
        .map(|request| {
            let barrier = Arc::clone(&barrier);
            let path = path.to_path_buf();
            std::thread::spawn(move || {
                let store = super::super::super::SqliteHubStore::open(&path)
                    .expect("open concurrent journal contender");
                barrier.wait();
                store.append_governance_record_batch(&request)
            })
        })
        .collect();
    handles
        .into_iter()
        .map(|handle| handle.join().expect("journal contender joins"))
        .collect()
}

fn assert_one_stored_one_conflict(
    results: &[Result<AppendGovernanceRecordBatchResult, HubStoreError>],
) {
    let stored = results
        .iter()
        .filter(|result| {
            result
                .as_ref()
                .is_ok_and(|result| result.disposition == GovernanceRecordAppendDisposition::Stored)
        })
        .count();
    let conflicts = results
        .iter()
        .filter(|result| matches!(result, Err(HubStoreError::Conflict { .. })))
        .count();
    assert_eq!((stored, conflicts), (1, 1), "{results:?}");
}

fn assert_no_partial_batch(fixture: &StoreFixture) {
    let incomplete: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM governance_record_append_batches b
             WHERE b.record_count != (
               SELECT COUNT(*) FROM governance_records r WHERE r.batch_id=b.batch_id
             )",
            [],
            |row| row.get(0),
        )
        .expect("count incomplete journal batches");
    assert_eq!(incomplete, 0);
}
