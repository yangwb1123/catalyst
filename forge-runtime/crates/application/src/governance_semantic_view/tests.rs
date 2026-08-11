use std::sync::Arc;

use serde::Deserialize;

use crate::runtime_domain::governance_contract::{
    ClaimObjectValue, ClaimState, ClaimType, EvidenceType, GovernanceRecord, ValidationPlan,
};
use crate::runtime_domain::{
    GovernanceClaimConflictGroup, GovernanceRecordKind, GovernanceSemanticListFilter,
    GovernanceSemanticProjection, GovernanceSemanticViewStore, GovernanceValidationJob,
    GovernanceValidationJobFilter, HubStoreError, governance_claim_conflict_groups,
    governance_semantic_projection, governance_validation_job,
};
use crate::{GovernanceSemanticViewService, GovernanceSemanticViewServiceError};

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/governance-evidence-claim-v1.json");
const AS_OF: i64 = 1_700_000_002_000;

#[derive(Deserialize)]
struct GoldenFixture {
    records: Vec<GoldenEntry>,
}

#[derive(Deserialize)]
struct GoldenEntry {
    record: GovernanceRecord,
}

#[derive(Clone)]
struct TestStore {
    projection: GovernanceSemanticProjection,
    conflicts: Vec<GovernanceClaimConflictGroup>,
    jobs: Vec<GovernanceValidationJob>,
}

impl GovernanceSemanticViewStore for TestStore {
    fn inspect_governance_semantic_projection(
        &self,
        _record_kind: GovernanceRecordKind,
        _aggregate_id: &str,
    ) -> Result<GovernanceSemanticProjection, HubStoreError> {
        Ok(self.projection.clone())
    }

    fn list_governance_claim_conflicts(
        &self,
        _filter: &GovernanceSemanticListFilter,
    ) -> Result<Vec<GovernanceClaimConflictGroup>, HubStoreError> {
        Ok(self.conflicts.clone())
    }

    fn list_governance_validation_jobs(
        &self,
        _filter: &GovernanceValidationJobFilter,
    ) -> Result<Vec<GovernanceValidationJob>, HubStoreError> {
        Ok(self.jobs.clone())
    }

    fn rebuild_governance_semantic_views(&self) -> Result<usize, HubStoreError> {
        Ok(3)
    }
}

fn fixture_claim() -> GovernanceRecord {
    serde_json::from_str::<GoldenFixture>(FIXTURE)
        .expect("fixture")
        .records
        .remove(1)
        .record
}

fn reseal(record: &mut GovernanceRecord) {
    match record {
        GovernanceRecord::Evidence(value) => value.integrity.canonical_sha256.clear(),
        GovernanceRecord::Claim(value) => value.integrity.canonical_sha256.clear(),
    }
    let digest = record.expected_sha256().expect("digest");
    match record {
        GovernanceRecord::Evidence(value) => value.integrity.canonical_sha256 = digest,
        GovernanceRecord::Claim(value) => value.integrity.canonical_sha256 = digest,
    }
}

fn claim_variant(aggregate_id: &str, record_id: &str, object: &str) -> GovernanceRecord {
    let mut record = fixture_claim();
    let GovernanceRecord::Claim(claim) = &mut record else {
        unreachable!()
    };
    claim.metadata.aggregate_id = aggregate_id.into();
    claim.metadata.record_id = record_id.into();
    claim.spec.object_value = ClaimObjectValue::String(object.into());
    reseal(&mut record);
    record.validate().expect("valid claim variant");
    record
}

fn assumption() -> GovernanceRecord {
    let mut record = claim_variant("claim-assumption", "kcr-assumption", "assumed");
    let GovernanceRecord::Claim(claim) = &mut record else {
        unreachable!()
    };
    claim.spec.claim_type = ClaimType::Assumption;
    claim.spec.confidence_micros = Some(500_000);
    claim.spec.review_by_unix_ms = None;
    claim.spec.validation_plan = Some(ValidationPlan {
        due_at_unix_ms: AS_OF - 1,
        impact_if_false: "invalidates the local plan".into(),
        method: "collect a fresh test run".into(),
        owner_id: "validation-owner".into(),
        required_evidence_types: vec![EvidenceType::TestRun],
    });
    claim.status.state = ClaimState::Open;
    claim.status.valid_until_unix_ms = None;
    reseal(&mut record);
    record.validate().expect("valid assumption");
    record
}

fn store() -> TestStore {
    let first = governance_semantic_projection(&claim_variant("claim-a", "kcr-a", "first"), 10)
        .expect("first projection");
    let second = governance_semantic_projection(&claim_variant("claim-b", "kcr-b", "second"), 11)
        .expect("second projection");
    let conflicts =
        governance_claim_conflict_groups(vec![first.clone(), second], AS_OF).expect("conflicts");
    let assumption = governance_semantic_projection(&assumption(), 12).expect("assumption");
    let job = governance_validation_job(&assumption, AS_OF)
        .expect("validation job")
        .expect("scheduled job");
    TestStore {
        projection: first,
        conflicts,
        jobs: vec![job],
    }
}

#[test]
fn service_validates_and_evaluates_every_store_boundary() {
    let store = store();
    let aggregate = store.projection.head.aggregate_id.clone();
    let service = GovernanceSemanticViewService::new(Arc::new(store));
    let assessment = service
        .inspect(GovernanceRecordKind::KnowledgeClaim, &aggregate, AS_OF)
        .expect("assessment");
    assert_eq!(assessment.evaluated_at_unix_ms, AS_OF);

    let conflict_filter = GovernanceSemanticListFilter {
        as_of_unix_ms: AS_OF,
        limit: 1,
    };
    assert_eq!(
        service
            .list_conflicts(&conflict_filter)
            .expect("groups")
            .len(),
        1
    );
    let job_filter = GovernanceValidationJobFilter {
        as_of_unix_ms: AS_OF,
        due_only: true,
        limit: 1,
    };
    assert_eq!(
        service
            .list_validation_jobs(&job_filter)
            .expect("jobs")
            .len(),
        1
    );
    assert_eq!(service.rebuild().expect("rebuild"), 3);
}

#[test]
fn preflight_rejects_implicit_or_unbounded_queries_without_a_store() {
    assert!(matches!(
        GovernanceSemanticViewService::preflight_inspect("invalid id", AS_OF),
        Err(GovernanceSemanticViewServiceError::InvalidInput { .. })
    ));
    assert!(GovernanceSemanticViewService::preflight_inspect("claim-a", -1).is_err());
    assert!(
        GovernanceSemanticViewService::preflight_conflicts(&GovernanceSemanticListFilter {
            as_of_unix_ms: AS_OF,
            limit: 0,
        })
        .is_err()
    );
}

#[test]
fn inconsistent_projection_and_evaluation_time_are_rejected() {
    let mut bad_projection = store();
    bad_projection.projection.head.aggregate_id = "claim-other".into();
    let service = GovernanceSemanticViewService::new(Arc::new(bad_projection));
    assert!(matches!(
        service.inspect(GovernanceRecordKind::KnowledgeClaim, "claim-a", AS_OF),
        Err(GovernanceSemanticViewServiceError::InconsistentStoreResult)
    ));

    let mut bad_job = store();
    bad_job.jobs[0].evaluated_at_unix_ms += 1;
    let service = GovernanceSemanticViewService::new(Arc::new(bad_job));
    assert!(matches!(
        service.list_validation_jobs(&GovernanceValidationJobFilter {
            as_of_unix_ms: AS_OF,
            due_only: true,
            limit: 1,
        }),
        Err(GovernanceSemanticViewServiceError::InconsistentStoreResult)
    ));
}

#[test]
fn mock_store_cannot_inject_unbound_conflict_or_job_identity() {
    let mut bad_conflict = store();
    bad_conflict.conflicts[0].conflict_key_sha256 = "0".repeat(64);
    let service = GovernanceSemanticViewService::new(Arc::new(bad_conflict));
    assert!(matches!(
        service.list_conflicts(&GovernanceSemanticListFilter {
            as_of_unix_ms: AS_OF,
            limit: 1,
        }),
        Err(GovernanceSemanticViewServiceError::InconsistentStoreResult)
    ));

    let mut bad_job = store();
    bad_job.jobs[0].job_id = format!("governance-validation-job-{}", "0".repeat(64));
    let service = GovernanceSemanticViewService::new(Arc::new(bad_job));
    assert!(matches!(
        service.list_validation_jobs(&GovernanceValidationJobFilter {
            as_of_unix_ms: AS_OF,
            due_only: true,
            limit: 1,
        }),
        Err(GovernanceSemanticViewServiceError::InconsistentStoreResult)
    ));
}
