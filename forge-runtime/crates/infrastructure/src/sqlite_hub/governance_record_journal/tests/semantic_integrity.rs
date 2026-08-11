use crate::runtime_domain::governance_contract::{
    ClaimState, ClaimType, EvidenceType, GovernanceRecord, ValidationPlan,
};
use crate::runtime_domain::{
    GovernanceRecordJournalStore, GovernanceRecordKind, GovernanceSemanticListFilter,
    GovernanceSemanticViewStore, GovernanceValidationJobFilter, HubStoreError,
};

use super::fixtures::{
    StoreFixture, change_record_id, claim_successor, claim_variant, evidence_successor,
    golden_records, request, reseal, rewrite_record_batch,
};

#[test]
fn semantic_read_rejects_dangling_and_wrong_subject_historical_references() {
    assert_rewritten_reference_is_corrupt(ReferenceCorruption::Dangling);
    assert_rewritten_reference_is_corrupt(ReferenceCorruption::WrongSubject);
}

#[test]
fn semantic_read_rejects_a_coherently_stored_derivation_cycle() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "cycle-evidence", 100);
    let original = claim_variant(&records[1], "kcr-cycle-a", "claim-cycle-a", Vec::new());
    let derived = claim_variant(
        &records[1],
        "kcr-cycle-b",
        "claim-cycle-b",
        vec!["kcr-cycle-a".into()],
    );
    append(&fixture, std::slice::from_ref(&original), "cycle-a", 200);
    append(&fixture, &[derived], "cycle-b", 300);
    let mut corrupted = original.clone();
    claim_mut(&mut corrupted).spec.derived_from_claim_record_ids = vec!["kcr-cycle-b".into()];
    reseal(&mut corrupted);
    rewrite_record_batch(
        &fixture,
        &corrupted,
        &request(&[original], "cycle-a", 200),
        &request(&[corrupted.clone()], "cycle-a", 200),
    );
    assert_corrupt_projection(&fixture, "claim-cycle-a");
}

#[test]
fn semantic_rebuild_rolls_back_on_coherent_relation_corruption() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "rebuild-evidence", 100);
    let original = claim_variant(&records[1], "kcr-rebuild", "claim-rebuild", Vec::new());
    append(
        &fixture,
        std::slice::from_ref(&original),
        "rebuild-claim",
        200,
    );
    let before = semantic_identity_rows(&fixture);
    let mut corrupted = original.clone();
    claim_mut(&mut corrupted)
        .spec
        .supporting_evidence_record_ids = vec!["evr-missing".into()];
    reseal(&mut corrupted);
    rewrite_record_batch(
        &fixture,
        &corrupted,
        &request(&[original], "rebuild-claim", 200),
        &request(&[corrupted.clone()], "rebuild-claim", 200),
    );
    let error = fixture
        .store
        .rebuild_governance_semantic_views()
        .expect_err("relation corruption aborts the combined rebuild");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(semantic_identity_rows(&fixture), before);
}

#[test]
fn valid_history_one_over_the_view_bound_is_unavailable() {
    let fixture = StoreFixture::new();
    let template = golden_records().remove(0);
    let versions = evidence_history(template, 1_025);
    for (index, batch) in versions.chunks(256).enumerate() {
        append(
            &fixture,
            batch,
            &format!("history-bound-{index}"),
            u64::try_from(index + 1).expect("append time"),
        );
    }
    let error = fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::EvidenceRecord,
            "evidence-history-bound",
        )
        .expect_err("bounded view cannot partially validate a longer legal history");
    assert!(
        matches!(error, HubStoreError::Unavailable { .. }),
        "{error:?}"
    );
    fixture
        .connection()
        .execute(
            "UPDATE governance_structural_heads
             SET record_id='evr-history-0001',sequence=1,
                 canonical_sha256=(SELECT canonical_sha256 FROM governance_records
                   WHERE record_id='evr-history-0001'),
                 updated_at_ms=(SELECT appended_at_ms FROM governance_records
                   WHERE record_id='evr-history-0001')
             WHERE record_kind='EvidenceRecord'
               AND aggregate_id='evidence-history-bound'",
            [],
        )
        .expect("install stale in-bound head over an oversized stored history");
    let bounded = fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::EvidenceRecord,
            "evidence-history-bound",
        )
        .expect_err("bounded history query must stop before decoding excess rows");
    assert!(
        matches!(bounded, HubStoreError::Unavailable { .. }),
        "{bounded:?}"
    );
}

#[test]
fn semantic_read_rejects_invalid_historical_lifecycle_transition() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "lifecycle-evidence", 100);
    let open = assumption(&records[1], "kcr-life-1", "claim-life");
    append(&fixture, std::slice::from_ref(&open), "lifecycle-open", 200);
    let mut testing = claim_successor(&open, "kcr-life-2", Vec::new());
    claim_mut(&mut testing).status.state = ClaimState::Testing;
    reseal(&mut testing);
    append(
        &fixture,
        std::slice::from_ref(&testing),
        "lifecycle-testing",
        300,
    );
    let mut invalid = claim_successor(&testing, "kcr-life-3", Vec::new());
    claim_mut(&mut invalid).status.state = ClaimState::Open;
    reseal(&mut invalid);
    invalid
        .validate()
        .expect("open is individually shadow-valid");
    let valid_tail = claim_successor(&testing, "kcr-life-3", Vec::new());
    append(
        &fixture,
        std::slice::from_ref(&valid_tail),
        "lifecycle-tail",
        400,
    );
    rewrite_record_batch(
        &fixture,
        &invalid,
        &request(&[valid_tail], "lifecycle-tail", 400),
        &request(&[invalid.clone()], "lifecycle-tail", 400),
    );
    assert_corrupt_projection(&fixture, "claim-life");
}

#[test]
fn balanced_semantic_head_and_claim_child_corruption_fail_exact_parity() {
    assert_balanced_projection_corruption(
        "UPDATE governance_semantic_heads SET aggregate_id='balanced-extra-head'
         WHERE aggregate_id='claim-harness-check'",
    );
    assert_balanced_projection_corruption(
        "UPDATE governance_claim_semantic_views SET aggregate_id='balanced-extra-child'
         WHERE aggregate_id='claim-harness-check'",
    );
}

#[test]
fn balanced_validation_job_corruption_fails_exact_completeness() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "job-evidence", 100);
    let claim = assumption(&records[1], "kcr-job", "claim-job");
    append(&fixture, &[claim], "job-claim", 200);
    fixture
        .connection()
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             UPDATE governance_claim_validation_jobs SET aggregate_id='balanced-extra-job'
             WHERE aggregate_id='claim-job';",
        )
        .expect("install balanced job corruption");
    let error = fixture
        .store
        .list_governance_validation_jobs(&GovernanceValidationJobFilter {
            as_of_unix_ms: 1_700_000_002_000,
            due_only: false,
            limit: 10,
        })
        .expect_err("balanced job drift must fail");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

#[test]
fn balanced_claim_head_deletion_fails_conflict_and_job_lists() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "deleted-head-evidence", 100);
    let claim = assumption(&records[1], "kcr-deleted-head", "claim-deleted-head");
    append(&fixture, &[claim], "deleted-head-claim", 200);
    fixture
        .connection()
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DELETE FROM governance_claim_validation_jobs
               WHERE aggregate_id='claim-deleted-head';
             DELETE FROM governance_claim_semantic_views
               WHERE aggregate_id='claim-deleted-head';
             DELETE FROM governance_semantic_heads
               WHERE aggregate_id='claim-deleted-head';
             DELETE FROM governance_structural_heads
               WHERE aggregate_id='claim-deleted-head';",
        )
        .expect("delete every materialized identity for an immutable claim");
    let conflicts = fixture
        .store
        .list_governance_claim_conflicts(&GovernanceSemanticListFilter {
            as_of_unix_ms: 1_700_000_002_000,
            limit: 10,
        })
        .expect_err("balanced Claim deletion must not become an empty conflict list");
    assert!(matches!(conflicts, HubStoreError::Corrupt { .. }));
    let jobs = fixture
        .store
        .list_governance_validation_jobs(&GovernanceValidationJobFilter {
            as_of_unix_ms: 1_700_000_002_000,
            due_only: false,
            limit: 10,
        })
        .expect_err("balanced Claim deletion must not become an empty job list");
    assert!(matches!(jobs, HubStoreError::Corrupt { .. }));
}

#[derive(Clone, Copy)]
enum ReferenceCorruption {
    Dangling,
    WrongSubject,
}

fn assert_rewritten_reference_is_corrupt(kind: ReferenceCorruption) {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "reference-evidence", 100);
    let original = claim_variant(&records[1], "kcr-reference", "claim-reference", Vec::new());
    append(
        &fixture,
        std::slice::from_ref(&original),
        "reference-claim",
        200,
    );
    let target = reference_target(&fixture, &records[0], kind);
    let mut corrupted = original.clone();
    claim_mut(&mut corrupted)
        .spec
        .supporting_evidence_record_ids = vec![target];
    reseal(&mut corrupted);
    rewrite_record_batch(
        &fixture,
        &corrupted,
        &request(&[original], "reference-claim", 200),
        &request(&[corrupted.clone()], "reference-claim", 200),
    );
    assert_corrupt_projection(&fixture, "claim-reference");
}

fn reference_target(
    fixture: &StoreFixture,
    evidence: &GovernanceRecord,
    kind: ReferenceCorruption,
) -> String {
    if matches!(kind, ReferenceCorruption::Dangling) {
        return "evr-missing".into();
    }
    let mut other = evidence.clone();
    let GovernanceRecord::Evidence(value) = &mut other else {
        unreachable!()
    };
    value.metadata.record_id = "evr-other-subject".into();
    value.metadata.aggregate_id = "evidence-other-subject".into();
    value.spec.subjects = vec!["module:other".into()];
    reseal(&mut other);
    append(fixture, &[other], "other-subject", 150);
    "evr-other-subject".into()
}

fn assert_balanced_projection_corruption(statement: &str) {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "balanced-projection", 100);
    fixture
        .connection()
        .execute_batch(&format!("PRAGMA foreign_keys=OFF; {statement};"))
        .expect("install count-balanced projection corruption");
    let error = fixture
        .store
        .inspect_governance_semantic_projection(
            GovernanceRecordKind::EvidenceRecord,
            "evidence-check-pass",
        )
        .expect_err("balanced missing plus extra projection must fail");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

fn assumption(
    template: &GovernanceRecord,
    record_id: &str,
    aggregate_id: &str,
) -> GovernanceRecord {
    let mut record = claim_variant(template, record_id, aggregate_id, Vec::new());
    let claim = claim_mut(&mut record);
    claim.spec.claim_type = ClaimType::Assumption;
    claim.spec.confidence_micros = Some(500_000);
    claim.spec.review_by_unix_ms = None;
    claim.spec.validation_plan = Some(ValidationPlan {
        due_at_unix_ms: 1_700_000_003_000,
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

fn evidence_history(mut first: GovernanceRecord, count: usize) -> Vec<GovernanceRecord> {
    change_record_id(&mut first, "evr-history-0001");
    let GovernanceRecord::Evidence(evidence) = &mut first else {
        unreachable!()
    };
    evidence.metadata.aggregate_id = "evidence-history-bound".into();
    reseal(&mut first);
    let mut records = Vec::with_capacity(count);
    records.push(first);
    while records.len() < count {
        let sequence = i64::try_from(records.len() + 1).expect("history sequence");
        let record_id = format!("evr-history-{sequence:04}");
        let next = evidence_successor(
            records.last().expect("history predecessor"),
            &record_id,
            sequence,
        );
        records.push(next);
    }
    records
}

fn semantic_identity_rows(fixture: &StoreFixture) -> Vec<(String, String, String, Vec<u8>)> {
    let connection = fixture.connection();
    let mut statement = connection
        .prepare(
            "SELECT record_kind,aggregate_id,record_id,projection_sha256
             FROM governance_semantic_heads ORDER BY record_kind,aggregate_id",
        )
        .expect("prepare semantic identity snapshot");
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query semantic identity snapshot")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect semantic identity snapshot")
}

fn append(fixture: &StoreFixture, records: &[GovernanceRecord], key: &str, time: u64) {
    fixture
        .store
        .append_governance_record_batch(&request(records, key, time))
        .expect("append semantic integrity fixture");
}

fn assert_corrupt_projection(fixture: &StoreFixture, aggregate_id: &str) {
    let error = fixture
        .store
        .inspect_governance_semantic_projection(GovernanceRecordKind::KnowledgeClaim, aggregate_id)
        .expect_err("historical semantic corruption must fail closed");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

fn claim_mut(
    record: &mut GovernanceRecord,
) -> &mut crate::runtime_domain::governance_contract::KnowledgeClaim {
    let GovernanceRecord::Claim(claim) = record else {
        panic!("expected claim")
    };
    claim
}
