use serde::Deserialize;

use crate::governance_contract::{
    ClaimObjectValue, ClaimState, ClaimType, EvidenceType, GovernanceRecord, ValidationPlan,
};

use super::*;

const FIXTURE: &str =
    include_str!("../../../../../../docs/contracts/fixtures/governance-evidence-claim-v1.json");
const SEMANTIC_FIXTURE: &str =
    include_str!("../../../../../../docs/contracts/fixtures/governance-semantic-view-v1.json");
const VALID_FROM: i64 = 1_700_000_001_000;

#[derive(Deserialize)]
struct GoldenFixture {
    records: Vec<GoldenEntry>,
}

#[derive(Deserialize)]
struct GoldenEntry {
    record: GovernanceRecord,
}

fn records() -> Vec<GovernanceRecord> {
    serde_json::from_str::<GoldenFixture>(FIXTURE)
        .expect("fixture")
        .records
        .into_iter()
        .map(|entry| entry.record)
        .collect()
}

fn claim() -> GovernanceRecord {
    records().remove(1)
}

fn claim_mut(record: &mut GovernanceRecord) -> &mut crate::governance_contract::KnowledgeClaim {
    let GovernanceRecord::Claim(value) = record else {
        panic!("expected claim")
    };
    value
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
    let mut record = claim();
    let value = claim_mut(&mut record);
    value.metadata.aggregate_id = aggregate_id.into();
    value.metadata.record_id = record_id.into();
    value.spec.object_value = ClaimObjectValue::String(object.into());
    reseal(&mut record);
    record.validate().expect("valid variant");
    record
}

fn assumption() -> GovernanceRecord {
    let mut record = claim_variant("claim-assumption", "kcr-assumption", "assumed");
    let value = claim_mut(&mut record);
    value.spec.claim_type = ClaimType::Assumption;
    value.spec.confidence_micros = Some(500_000);
    value.spec.review_by_unix_ms = None;
    value.spec.validation_plan = Some(ValidationPlan {
        due_at_unix_ms: VALID_FROM + 2_000,
        impact_if_false: "invalidates the plan".into(),
        method: "collect a fresh test run".into(),
        owner_id: "validation-owner".into(),
        required_evidence_types: vec![EvidenceType::GateResult, EvidenceType::TestRun],
    });
    value.status.state = ClaimState::Open;
    value.status.valid_until_unix_ms = None;
    reseal(&mut record);
    record.validate().expect("valid assumption");
    record
}

fn claim_with_state(claim_type: ClaimType, state: ClaimState, suffix: &str) -> GovernanceRecord {
    let mut record = claim_variant(
        &format!("claim-{suffix}"),
        &format!("kcr-{suffix}"),
        "shadow-state",
    );
    let value = claim_mut(&mut record);
    value.spec.claim_type = claim_type;
    value.status.state = state;
    value.spec.confidence_micros = matches!(
        claim_type,
        ClaimType::Assumption | ClaimType::Hypothesis | ClaimType::Inference
    )
    .then_some(500_000);
    value.spec.validation_plan =
        matches!(claim_type, ClaimType::Assumption | ClaimType::Hypothesis).then(|| {
            ValidationPlan {
                due_at_unix_ms: VALID_FROM + 2_000,
                impact_if_false: "invalidates the plan".into(),
                method: "collect a fresh test run".into(),
                owner_id: "validation-owner".into(),
                required_evidence_types: vec![EvidenceType::TestRun],
            }
        });
    value.spec.queue_ref = (claim_type == ClaimType::Unknown).then(|| "unknown-queue".into());
    value.status.valid_until_unix_ms = None;
    reseal(&mut record);
    record.validate().expect("valid shadow-state claim");
    record
}

fn successor(previous: &GovernanceRecord, state: ClaimState) -> GovernanceRecord {
    let mut next = previous.clone();
    let prior = previous.metadata().clone();
    let value = claim_mut(&mut next);
    value.metadata.record_id = format!("{}-next", prior.record_id);
    value.metadata.sequence += 1;
    value.metadata.created_at_unix_ms += 1;
    value.metadata.supersedes_record_ids = vec![prior.record_id];
    value.status.state = state;
    value.status.valid_from_unix_ms = value.metadata.created_at_unix_ms;
    reseal(&mut next);
    next
}

#[test]
fn golden_projection_and_digest_are_deterministic() {
    let projection = governance_semantic_projection(&claim(), 77).expect("projection");
    assert_eq!(projection.head.record_id, "kcr-0001");
    assert_eq!(projection.head.declared_state, "candidate");
    assert_eq!(
        projection.claim.as_ref().expect("claim").claim_type,
        ClaimType::Fact
    );
    assert_eq!(
        projection.projection_sha256,
        "9754f24cb1c6f33d72492e1391c9bc70e44d5893020def288610b84f17e88fea"
    );
    projection.validate().expect("valid projection");
    let assessment = evaluate_governance_semantic_projection(projection.clone(), 1_700_000_002_000)
        .expect("golden assessment");
    let expected: serde_json::Value =
        serde_json::from_str(SEMANTIC_FIXTURE).expect("semantic fixture");
    assert_eq!(
        expected["source_record_ref"],
        "docs/contracts/fixtures/governance-evidence-claim-v1.json#/records/1/record"
    );
    let source: serde_json::Value = serde_json::from_str(FIXTURE).expect("source fixture");
    assert_eq!(
        source.pointer("/records/1/record"),
        Some(&serde_json::to_value(claim()).expect("claim value"))
    );
    assert_eq!(
        expected["expected_assessment"],
        serde_json::json!({
            "api_version": "forgeos.governance-semantic-view/v1",
            "kind": "GovernanceSemanticAssessment",
            "interpretation": "semantic_projection_only_no_truth_or_authority",
            "semantic_view_version": assessment.v,
            "projection": assessment.projection,
            "evaluated_at_unix_ms": assessment.evaluated_at_unix_ms,
            "temporal_state": assessment.temporal_state,
        })
    );

    let mut tampered = projection;
    tampered.head.scope = "module:other".into();
    assert!(tampered.validate().is_err());
}

#[test]
fn explicit_time_evaluation_covers_every_temporal_state() {
    let mut fact = claim();
    let value = claim_mut(&mut fact);
    value.spec.review_by_unix_ms = Some(VALID_FROM + 2_000);
    value.status.valid_until_unix_ms = Some(VALID_FROM + 4_000);
    reseal(&mut fact);
    let projection = governance_semantic_projection(&fact, 80).expect("fact projection");

    for (as_of, expected) in [
        (VALID_FROM - 1, GovernanceTemporalState::NotYetValid),
        (VALID_FROM, GovernanceTemporalState::Fresh),
        (VALID_FROM + 2_000, GovernanceTemporalState::ReviewOverdue),
        (VALID_FROM + 4_000, GovernanceTemporalState::ValidityExpired),
    ] {
        let assessment =
            evaluate_governance_semantic_projection(projection.clone(), as_of).expect("assessment");
        assert_eq!(assessment.temporal_state, expected);
    }

    let assumption = governance_semantic_projection(&assumption(), 81).expect("assumption");
    let overdue = evaluate_governance_semantic_projection(assumption, VALID_FROM + 2_000)
        .expect("overdue assessment");
    assert_eq!(
        overdue.temporal_state,
        GovernanceTemporalState::ValidationOverdue
    );
}

#[test]
fn lifecycle_enforces_initial_state_identity_and_shadow_edges() {
    let fact = claim();
    validate_governance_semantic_transition(None, &fact).expect("candidate starts fact");

    let contested = successor(&fact, ClaimState::Contested);
    contested.validate().expect("valid contested successor");
    validate_governance_semantic_transition(Some(&fact), &contested)
        .expect("candidate can become contested");

    let mut changed = successor(&fact, ClaimState::Candidate);
    claim_mut(&mut changed).spec.object_value = ClaimObjectValue::String("changed".into());
    reseal(&mut changed);
    changed
        .validate()
        .expect("individually valid changed claim");
    assert!(validate_governance_semantic_transition(Some(&fact), &changed).is_err());

    let open = assumption();
    validate_governance_semantic_transition(None, &open).expect("assumption starts open");
    let testing = successor(&open, ClaimState::Testing);
    testing.validate().expect("valid testing successor");
    validate_governance_semantic_transition(Some(&open), &testing).expect("open to testing");
    assert!(validate_governance_semantic_transition(Some(&testing), &open).is_err());
}

#[test]
fn sequence_one_accepts_every_adr45_shadow_state_for_v26_backfill() {
    let states = [
        (ClaimType::Fact, ClaimState::Candidate, "fact-candidate"),
        (ClaimType::Fact, ClaimState::Contested, "fact-contested"),
        (ClaimType::Constraint, ClaimState::Candidate, "constraint"),
        (ClaimType::Decision, ClaimState::Proposed, "decision"),
        (ClaimType::Inference, ClaimState::Candidate, "inference"),
        (ClaimType::Assumption, ClaimState::Open, "assumption-open"),
        (
            ClaimType::Assumption,
            ClaimState::Testing,
            "assumption-testing",
        ),
        (ClaimType::Hypothesis, ClaimState::Open, "hypothesis-open"),
        (
            ClaimType::Hypothesis,
            ClaimState::Testing,
            "hypothesis-testing",
        ),
        (ClaimType::Lesson, ClaimState::Candidate, "lesson"),
        (ClaimType::Proposal, ClaimState::Draft, "proposal-draft"),
        (
            ClaimType::Proposal,
            ClaimState::Submitted,
            "proposal-submitted",
        ),
        (ClaimType::Unknown, ClaimState::Open, "unknown-open"),
        (
            ClaimType::Unknown,
            ClaimState::Investigating,
            "unknown-investigating",
        ),
    ];
    for (claim_type, state, suffix) in states {
        let record = claim_with_state(claim_type, state, suffix);
        validate_governance_semantic_transition(None, &record).expect("valid sequence-one state");
    }
}

#[test]
fn conflict_groups_require_active_claims_with_distinct_objects() {
    let first = governance_semantic_projection(&claim_variant("claim-a", "kcr-a", "first"), 90)
        .expect("first");
    let second = governance_semantic_projection(&claim_variant("claim-b", "kcr-b", "second"), 91)
        .expect("second");
    let groups =
        governance_claim_conflict_groups(vec![second.clone(), first.clone()], VALID_FROM + 1)
            .expect("groups");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].members[0].aggregate_id, "claim-a");
    assert_eq!(groups[0].members[1].aggregate_id, "claim-b");

    assert!(
        governance_claim_conflict_groups(vec![first.clone(), first], VALID_FROM + 1)
            .expect("same objects")
            .is_empty()
    );
    assert!(
        governance_claim_conflict_groups(vec![second], VALID_FROM + 86_400_000)
            .expect("expired candidates")
            .is_empty()
    );
}

#[test]
fn validation_job_identity_is_stable_and_due_is_caller_time_relative() {
    let projection = governance_semantic_projection(&assumption(), 100).expect("projection");
    let pending = governance_validation_job(&projection, VALID_FROM + 1_999)
        .expect("pending job")
        .expect("job exists");
    let due = governance_validation_job(&projection, VALID_FROM + 2_000)
        .expect("due job")
        .expect("job exists");
    assert_eq!(pending.job_id, due.job_id);
    assert!(!pending.due);
    assert!(due.due);
    assert_eq!(
        due.job_id,
        "governance-validation-job-2b03d42237c1c2735347381845e704f47f46165d0172b6f213ba4b8ff287b1c5"
    );
}

#[test]
fn projection_recomputes_conflict_identity_and_rejects_authority_states() {
    let mut projection = governance_semantic_projection(&claim(), 110).expect("projection");
    projection
        .claim
        .as_mut()
        .expect("claim")
        .conflict_key_sha256 = "0".repeat(64);
    projection.projection_sha256 =
        governance_semantic_projection_sha256(&projection).expect("reseal projection");
    assert!(projection.validate().is_err());

    for (record, forbidden) in [
        (
            claim_with_state(ClaimType::Fact, ClaimState::Candidate, "fact"),
            "confirmed",
        ),
        (
            claim_with_state(ClaimType::Fact, ClaimState::Candidate, "fact-two"),
            "accepted",
        ),
        (
            claim_with_state(ClaimType::Assumption, ClaimState::Open, "assumption"),
            "validated",
        ),
        (
            claim_with_state(ClaimType::Unknown, ClaimState::Open, "unknown"),
            "resolved",
        ),
    ] {
        let mut projection = governance_semantic_projection(&record, 111).expect("projection");
        projection.head.declared_state = forbidden.into();
        projection.projection_sha256 =
            governance_semantic_projection_sha256(&projection).expect("reseal projection");
        assert!(projection.validate().is_err(), "accepted {forbidden}");
    }
}

#[test]
fn projection_binds_complete_validation_plan_to_validation_claim_types() {
    let mut fact = governance_semantic_projection(&claim(), 115).expect("fact projection");
    let fields = fact.claim.as_mut().expect("claim");
    fields.validation_due_unix_ms = Some(VALID_FROM + 2_000);
    fields.validation_owner_id = Some("validation-owner".into());
    fields.validation_plan_sha256 = Some("0".repeat(64));
    fields.required_evidence_types = vec![EvidenceType::TestRun];
    fact.projection_sha256 =
        governance_semantic_projection_sha256(&fact).expect("reseal projection");
    assert!(fact.validate().is_err());

    let mut assumption = governance_semantic_projection(&assumption(), 116).expect("assumption");
    let fields = assumption.claim.as_mut().expect("claim");
    fields.validation_due_unix_ms = None;
    fields.validation_owner_id = None;
    fields.validation_plan_sha256 = None;
    fields.required_evidence_types.clear();
    assumption.projection_sha256 =
        governance_semantic_projection_sha256(&assumption).expect("reseal projection");
    assert!(assumption.validate().is_err());
}

#[test]
fn conflict_and_job_validation_recompute_identity_and_shadow_state() {
    let first = governance_semantic_projection(&claim_variant("claim-x", "kcr-x", "first"), 120)
        .expect("first");
    let second = governance_semantic_projection(&claim_variant("claim-y", "kcr-y", "second"), 121)
        .expect("second");
    let mut group = governance_claim_conflict_groups(vec![first, second], VALID_FROM + 1)
        .expect("groups")
        .remove(0);
    group.conflict_key_sha256 = "0".repeat(64);
    assert!(group.validate().is_err());

    let first = governance_semantic_projection(&claim_variant("claim-x", "kcr-x", "first"), 120)
        .expect("first");
    let second = governance_semantic_projection(&claim_variant("claim-y", "kcr-y", "second"), 121)
        .expect("second");
    let mut group = governance_claim_conflict_groups(vec![first, second], VALID_FROM + 1)
        .expect("groups")
        .remove(0);
    group.members[0].declared_state = "confirmed".into();
    assert!(group.validate().is_err());

    let projection = governance_semantic_projection(&assumption(), 122).expect("projection");
    let mut job = governance_validation_job(&projection, VALID_FROM + 2_000)
        .expect("job")
        .expect("job exists");
    job.job_id = format!("{GOVERNANCE_VALIDATION_JOB_ID_PREFIX}{}", "0".repeat(64));
    assert!(job.validate().is_err());
    let mut job = governance_validation_job(&projection, VALID_FROM + 2_000)
        .expect("job")
        .expect("job exists");
    job.declared_state = "validated".into();
    assert!(job.validate().is_err());
    let mut job = governance_validation_job(&projection, VALID_FROM + 2_000)
        .expect("job")
        .expect("job exists");
    job.claim_type = ClaimType::Fact;
    job.declared_state = "candidate".into();
    assert!(job.validate().is_err());
}
