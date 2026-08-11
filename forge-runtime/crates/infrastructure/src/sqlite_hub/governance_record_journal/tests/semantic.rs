use crate::runtime_domain::governance_contract::{
    ClaimObjectValue, ClaimState, ClaimType, EvidenceType, GovernanceRecord, ValidationPlan,
};
use crate::runtime_domain::{
    GovernanceRecordJournalStore, GovernanceRecordKind, GovernanceSemanticListFilter,
    GovernanceSemanticViewStore, GovernanceTemporalState, GovernanceValidationJobFilter, HubStore,
    HubStoreError,
};

use std::sync::{Arc, Mutex};

use super::fixtures::{
    StoreFixture, claim_successor, claim_variant, evidence_successor, golden_records, request,
    reseal, row_count,
};

const AS_OF: i64 = 1_700_000_002_000;

#[test]
fn semantic_read_is_one_snapshot_across_history_closure_and_materialization() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    fixture
        .store
        .append_governance_record_batch(&request(&records, "snapshot-base", 100))
        .expect("append snapshot base");
    let successor = claim_successor(&records[1], "kcr-snapshot-0002", Vec::new());
    let append = request(&[successor], "snapshot-successor", 200);
    let writer = fixture.store.clone();
    let outcome = Arc::new(Mutex::new(None));
    let observed = Arc::clone(&outcome);
    super::super::semantic::install_after_snapshot_hook(move || {
        let result = writer.append_governance_record_batch(&append);
        *observed.lock().expect("record writer outcome") = Some(result);
    });

    let old = fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::KnowledgeClaim,
            "claim-harness-check",
        )
        .expect("snapshot read remains internally old");
    assert_eq!(old.head.record_id, "kcr-0001");
    outcome
        .lock()
        .expect("writer outcome")
        .take()
        .expect("writer ran after first snapshot read")
        .expect("writer committed successor");

    let fresh = fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::KnowledgeClaim,
            "claim-harness-check",
        )
        .expect("next read sees committed successor");
    assert_eq!(fresh.head.record_id, "kcr-snapshot-0002");
}

#[test]
fn semantic_scan_decodes_a_shared_owning_batch_once() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    fixture
        .store
        .append_governance_record_batch(&request(&records, "scan-shared-batch", 100))
        .expect("append shared batch");
    let connection = fixture.connection();
    let (decoded_batches, (records, bytes, work)) = super::super::semantic::scan_stats(
        &connection,
        &[
            (GovernanceRecordKind::EvidenceRecord, "evidence-check-pass"),
            (GovernanceRecordKind::KnowledgeClaim, "claim-harness-check"),
        ],
    )
    .expect("verify two projections through one scan verifier");
    assert_eq!(decoded_batches, 1);
    assert_eq!(records, 2);
    assert!(bytes > 0);
    assert!(work > 0);
}

#[test]
fn current_live_reader_uses_a_read_only_snapshot_while_a_writer_is_active() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    fixture
        .store
        .append_governance_record_batch(&request(&records, "live-semantic-base", 100))
        .expect("append live semantic fixture");
    let writer = prepare_hot_writer(&fixture);

    let live =
        super::super::super::SqliteHubStore::open_existing_current_live_read_only(&fixture.path)
            .expect("open exact current live reader");
    let projection = live
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::KnowledgeClaim,
            "claim-harness-check",
        )
        .expect("read a deferred semantic snapshot while writer is active");
    assert_eq!(projection.head.record_id, "kcr-0001");
    assert!(
        !live
            .list_groups()
            .expect("read committed WAL row")
            .is_empty()
    );
    let error = live
        .create_group("forbidden", "live-reader-write")
        .expect_err("live read-only mode rejects logical writes");
    assert!(
        matches!(error, HubStoreError::Unavailable { .. }),
        "{error}"
    );
    writer.execute_batch("ROLLBACK").expect("release writer");
}

fn prepare_hot_writer(fixture: &StoreFixture) -> rusqlite::Connection {
    let writer = fixture.connection();
    writer
        .execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA wal_autocheckpoint=0;
             INSERT INTO groups(id,name,idempotency_key,created_at_ms)
               VALUES('live-group','Live committed group','live-group-key',1);
             BEGIN IMMEDIATE;
             INSERT INTO groups(id,name,idempotency_key,created_at_ms)
               VALUES('pending-group','Uncommitted group','pending-group-key',2);",
        )
        .expect("prepare committed hot WAL plus active writer");
    assert!(fixture.path.with_extension("sqlite3-wal").exists());
    assert!(fixture.path.with_extension("sqlite3-shm").exists());
    writer
}

#[test]
fn append_materializes_exact_semantic_heads_and_read_revalidates_them() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    fixture
        .store
        .append_governance_record_batch(&request(&records, "semantic-materialize", 100))
        .expect("append semantic fixture");

    let connection = fixture.connection();
    assert_eq!(row_count(&connection, "governance_semantic_heads"), 2);
    assert_eq!(row_count(&connection, "governance_claim_semantic_views"), 1);
    assert_eq!(
        row_count(&connection, "governance_claim_validation_jobs"),
        0
    );
    drop(connection);

    let projection = fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::KnowledgeClaim,
            "claim-harness-check",
        )
        .expect("inspect semantic projection");
    assert_eq!(projection.head.record_id, "kcr-0001");
    assert_eq!(projection.head.updated_at_ms, 100);
    assert_eq!(projection.head.declared_state, "candidate");
}

#[test]
fn semantic_identity_change_conflicts_without_partial_journal_or_projection_writes() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    fixture
        .store
        .append_governance_record_batch(&request(&records, "semantic-base", 100))
        .expect("append semantic base");
    let mut changed = claim_successor(&records[1], "kcr-0002", Vec::new());
    let GovernanceRecord::Claim(claim) = &mut changed else {
        unreachable!()
    };
    claim.spec.object_value = ClaimObjectValue::String("changed identity".into());
    reseal(&mut changed);
    changed.validate().expect("individually valid successor");

    let error = fixture
        .store
        .append_governance_record_batch(&request(&[changed], "semantic-changed", 200))
        .expect_err("semantic identity change must conflict");
    assert!(matches!(error, HubStoreError::Conflict { .. }), "{error:?}");
    let connection = fixture.connection();
    assert_eq!(row_count(&connection, "governance_records"), 2);
    assert_eq!(row_count(&connection, "governance_structural_heads"), 2);
    assert_eq!(row_count(&connection, "governance_semantic_heads"), 2);
}

#[test]
fn conflicts_and_validation_jobs_use_explicit_caller_time() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    fixture
        .store
        .append_governance_record_batch(&request(&records[..1], "semantic-evidence", 100))
        .expect("append evidence");

    let first = conflicting_claim(&records[1], "kcr-conflict-a", "claim-conflict-a", "first");
    let second = conflicting_claim(&records[1], "kcr-conflict-b", "claim-conflict-b", "second");
    fixture
        .store
        .append_governance_record_batch(&request(&[first, second], "semantic-conflicts", 200))
        .expect("append conflict candidates");
    let groups = fixture
        .store
        .list_governance_claim_conflicts(&GovernanceSemanticListFilter {
            as_of_unix_ms: AS_OF,
            limit: 10,
        })
        .expect("list conflicts");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].members.len(), 2);

    assert_validation_jobs(&fixture, &records);
}

fn assert_validation_jobs(fixture: &StoreFixture, records: &[GovernanceRecord]) {
    let assumption = assumption(&records[1]);
    fixture
        .store
        .append_governance_record_batch(&request(&[assumption], "semantic-assumption", 300))
        .expect("append assumption");
    let pending = fixture
        .store
        .list_governance_validation_jobs(&GovernanceValidationJobFilter {
            as_of_unix_ms: AS_OF,
            due_only: false,
            limit: 10,
        })
        .expect("list pending job");
    assert_eq!(pending.len(), 1);
    assert!(!pending[0].due);
    assert_eq!(pending[0].temporal_state, GovernanceTemporalState::Fresh);

    let due = fixture
        .store
        .list_governance_validation_jobs(&GovernanceValidationJobFilter {
            as_of_unix_ms: AS_OF + 1_000,
            due_only: true,
            limit: 10,
        })
        .expect("list due job");
    assert_eq!(due.len(), 1);
    assert!(due[0].due);
    assert_eq!(
        due[0].temporal_state,
        GovernanceTemporalState::ValidationOverdue
    );
}

#[test]
fn semantic_corruption_blocks_reads_and_replay_then_rebuild_repairs_projection() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    let append = request(&records, "semantic-rebuild", 100);
    fixture
        .store
        .append_governance_record_batch(&append)
        .expect("append rebuild fixture");
    fixture
        .connection()
        .execute(
            "UPDATE governance_semantic_heads SET projection_sha256=zeroblob(32)
             WHERE aggregate_id='claim-harness-check'",
            [],
        )
        .expect("corrupt semantic digest");

    let read = fixture.store.inspect_governance_semantic_projection(
        GovernanceRecordKind::KnowledgeClaim,
        "claim-harness-check",
    );
    assert!(matches!(read, Err(HubStoreError::Corrupt { .. })));
    let replay = fixture.store.append_governance_record_batch(&append);
    assert!(matches!(replay, Err(HubStoreError::Corrupt { .. })));

    assert_eq!(
        fixture
            .store
            .rebuild_governance_semantic_views()
            .expect("rebuild semantic views"),
        2
    );
    fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::KnowledgeClaim,
            "claim-harness-check",
        )
        .expect("rebuilt projection reads");
}

#[test]
fn evidence_refresh_preserves_claim_projection_with_the_same_aggregate_id() {
    let fixture = StoreFixture::new();
    let mut records = golden_records();
    for record in &mut records {
        match record {
            GovernanceRecord::Evidence(evidence) => {
                evidence.metadata.aggregate_id = "shared-semantic-aggregate".into();
            }
            GovernanceRecord::Claim(claim) => {
                claim.metadata.aggregate_id = "shared-semantic-aggregate".into();
            }
        }
        reseal(record);
    }
    fixture
        .store
        .append_governance_record_batch(&request(&records, "semantic-shared-base", 100))
        .expect("append same aggregate ID across record kinds");

    let successor = evidence_successor(&records[0], "evr-shared-0002", 2);
    fixture
        .store
        .append_governance_record_batch(&request(
            &[successor],
            "semantic-shared-evidence-successor",
            200,
        ))
        .expect("append evidence successor");

    fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::KnowledgeClaim,
            "shared-semantic-aggregate",
        )
        .expect("claim projection remains complete");
    let connection = fixture.connection();
    assert_eq!(row_count(&connection, "governance_semantic_heads"), 2);
    assert_eq!(row_count(&connection, "governance_claim_semantic_views"), 1);
}

#[test]
fn oversized_conflict_group_is_unavailable_instead_of_partial_or_corrupt() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    fixture
        .store
        .append_governance_record_batch(&request(&records[..1], "semantic-bound-evidence", 100))
        .expect("append evidence");
    let claims: Vec<_> = (0..101)
        .map(|index| {
            conflicting_claim(
                &records[1],
                &format!("kcr-conflict-{index:03}"),
                &format!("claim-conflict-{index:03}"),
                if index % 2 == 0 { "first" } else { "second" },
            )
        })
        .collect();
    fixture
        .store
        .append_governance_record_batch(&request(&claims, "semantic-bound-claims", 200))
        .expect("append bounded conflict candidates");

    let error = fixture
        .store
        .list_governance_claim_conflicts(&GovernanceSemanticListFilter {
            as_of_unix_ms: AS_OF,
            limit: 100,
        })
        .expect_err("oversized group cannot be truncated or called corruption");
    assert!(
        matches!(error, HubStoreError::Unavailable { .. }),
        "{error:?}"
    );
}

fn conflicting_claim(
    template: &GovernanceRecord,
    record_id: &str,
    aggregate_id: &str,
    object: &str,
) -> GovernanceRecord {
    let mut record = claim_variant(template, record_id, aggregate_id, Vec::new());
    let GovernanceRecord::Claim(claim) = &mut record else {
        unreachable!()
    };
    claim.spec.object_value = ClaimObjectValue::String(object.into());
    reseal(&mut record);
    record.validate().expect("valid conflict candidate");
    record
}

fn assumption(template: &GovernanceRecord) -> GovernanceRecord {
    let mut record = claim_variant(
        template,
        "kcr-semantic-assumption",
        "claim-semantic-assumption",
        Vec::new(),
    );
    let GovernanceRecord::Claim(claim) = &mut record else {
        unreachable!()
    };
    claim.spec.claim_type = ClaimType::Assumption;
    claim.spec.confidence_micros = Some(500_000);
    claim.spec.review_by_unix_ms = None;
    claim.spec.validation_plan = Some(ValidationPlan {
        due_at_unix_ms: AS_OF + 1_000,
        impact_if_false: "invalidates the plan".into(),
        method: "collect a test run".into(),
        owner_id: "validation-owner".into(),
        required_evidence_types: vec![EvidenceType::TestRun],
    });
    claim.status.state = ClaimState::Open;
    claim.status.valid_until_unix_ms = None;
    reseal(&mut record);
    record.validate().expect("valid assumption");
    record
}
