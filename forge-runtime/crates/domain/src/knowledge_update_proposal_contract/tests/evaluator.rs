use super::{
    super::*,
    support::{fixture, reseal_proposal, reseal_request},
};

type TargetMutator = fn(&mut KnowledgeUpdateDeclaredTarget);

const TARGET_MISMATCHES: [(KnowledgeUpdateReasonCode, TargetMutator); 7] = [
    (KnowledgeUpdateReasonCode::BindingMismatch, drift_binding),
    (KnowledgeUpdateReasonCode::GrantRefMismatch, drift_grant_ref),
    (
        KnowledgeUpdateReasonCode::MutationsMismatch,
        drift_mutations,
    ),
    (KnowledgeUpdateReasonCode::ProposerMismatch, drift_proposer),
    (
        KnowledgeUpdateReasonCode::RecordSetMismatch,
        drift_record_set,
    ),
    (KnowledgeUpdateReasonCode::ScopeMismatch, drift_scope),
    (KnowledgeUpdateReasonCode::TaskBindingMismatch, drift_task),
];

#[test]
fn evaluator_reports_each_records_free_target_mismatch() {
    let base = fixture().assessment_request;
    for (reason, mutate) in TARGET_MISMATCHES {
        assert_mismatch(&base, reason, mutate);
    }
}

fn assert_mismatch(
    base: &KnowledgeUpdateAssessmentRequest,
    reason: KnowledgeUpdateReasonCode,
    mutate: impl FnOnce(&mut KnowledgeUpdateDeclaredTarget),
) {
    let mut request = base.clone();
    mutate(&mut request.expected_target);
    reseal_request(&mut request);
    let assessment = evaluate_declared_assessment(&request).expect("mismatch assessment");
    assert_eq!(assessment.reason_codes, vec![reason]);
}

fn drift_binding(target: &mut KnowledgeUpdateDeclaredTarget) {
    target.bindings.context_sha256 = "0".repeat(64);
}

fn drift_grant_ref(target: &mut KnowledgeUpdateDeclaredTarget) {
    target.capability_grant_ref.authority_domain = "other.domain".into();
}

fn drift_mutations(target: &mut KnowledgeUpdateDeclaredTarget) {
    target.mutations[0].rationale = "different declared rationale".into();
}

fn drift_proposer(target: &mut KnowledgeUpdateDeclaredTarget) {
    target.proposer.principal_id = "other-proposer".into();
}

fn drift_record_set(target: &mut KnowledgeUpdateDeclaredTarget) {
    target.record_set_sha256 = "0".repeat(64);
}

fn drift_scope(target: &mut KnowledgeUpdateDeclaredTarget) {
    target.knowledge_scope.object_ref = "knowledge:other".into();
}

fn drift_task(target: &mut KnowledgeUpdateDeclaredTarget) {
    target.task_binding.role = "other-role".into();
}

#[test]
fn future_submission_is_a_relation_not_an_authority_decision() {
    let mut request = fixture().assessment_request;
    let mut proposal = request.knowledge_update_proposal.clone();
    proposal.submitted_at_unix_ms = request.evaluated_at_unix_ms + 1;
    request.knowledge_update_proposal = reseal_proposal(&proposal).expect("future declaration");
    reseal_request(&mut request);
    let assessment = evaluate_declared_assessment(&request).expect("future assessment");
    assert_eq!(
        assessment.relations.temporal,
        TemporalRelation::FutureDeclaredSubmission
    );
    assert_eq!(
        assessment.reason_codes,
        vec![KnowledgeUpdateReasonCode::TemporalDeclarationMismatch]
    );
    assert_eq!(assessment.authorization_decision, NoDecision::None);
    assert!(!assessment.knowledge_adoption_attestation);
}

#[test]
fn assessment_instance_drift_is_rejected_even_when_rehashed() {
    let golden = fixture();
    let mut assessment = golden.expected_assessment;
    assessment.truth_attestation = true;
    assessment.assessment_sha256.clear();
    assessment.assessment_sha256 = assessment_sha256(&assessment).expect("rehashed drift");
    assert!(validate_assessment(&golden.assessment_request, &assessment).is_err());
}
