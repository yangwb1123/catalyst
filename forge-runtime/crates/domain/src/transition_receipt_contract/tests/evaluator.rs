use super::super::{
    ChainRelation, ContinuityRelation, EdgeRelation, PreconditionsRelation, RecoveryRelation,
    TargetRelation, TransitionTemporalRelation, evaluate_declared_assessment,
};
use super::{fixture, prior_entering, reseal_receipt, reseal_request, successor};
use crate::transition_receipt_contract::{TransitionReasonCode, TransitionState, vocabulary};

#[test]
fn all_authored_and_unlisted_state_edges_are_exact() {
    for source in vocabulary::STATES {
        let previous = prior_entering(source);
        for target in vocabulary::STATES {
            let current = successor(&previous, target);
            let mut request = fixture().assessment_request;
            request.previous_receipt = Some(previous.clone());
            request.transition_receipt = current.clone();
            request.expected_target = super::super::declared_target(&current).unwrap();
            request.evaluated_at_unix_ms = current.transition.declared_at_unix_ms;
            reseal_request(&mut request);
            let actual = evaluate_declared_assessment(&request)
                .unwrap()
                .relations
                .edge;
            let dynamic = matches!(
                source,
                TransitionState::NeedsInfo | TransitionState::Blocked
            ) && previous.transition.resume_state == Some(target);
            let listed = vocabulary::statically_listed(source, target) || dynamic;
            assert_eq!(actual == EdgeRelation::ListedDeclaredEdge, listed);
        }
    }
}

#[test]
fn initial_and_valid_predecessor_relations_match() {
    let golden = fixture();
    let initial = evaluate_declared_assessment(&golden.assessment_request).unwrap();
    assert_eq!(initial.relations.chain, ChainRelation::InitialDeclaredChain);
    let previous = prior_entering(TransitionState::NeedsEvidence);
    let current = successor(&previous, TransitionState::Baselined);
    let assessment = assessment_for(current, Some(previous));
    assert_eq!(
        assessment.relations.chain,
        ChainRelation::SameDeclaredPredecessor
    );
    assert_eq!(
        assessment.relations.continuity,
        ContinuityRelation::SameDeclaredStateContinuity
    );
}

#[test]
fn predecessor_state_time_and_target_mismatches_are_explained() {
    let previous = prior_entering(TransitionState::NeedsEvidence);
    let mut current = successor(&previous, TransitionState::Baselined);
    current.previous_receipt_id = Some(format!("transition-receipt-{}", "a".repeat(64)));
    current.previous_receipt_sha256 = Some("a".repeat(64));
    reseal_receipt(&mut current);
    let mut request = request_for(current, Some(previous));
    request.expected_target.actor.principal_id = "different-agent".into();
    request.expected_target_sha256 =
        super::super::declared_target_sha256(&request.expected_target).unwrap();
    request.evaluated_at_unix_ms -= 2;
    reseal_request(&mut request);
    let result = evaluate_declared_assessment(&request).unwrap();
    assert_eq!(result.relations.chain, ChainRelation::PredecessorMismatch);
    assert_eq!(result.relations.target, TargetRelation::TargetMismatch);
    assert_eq!(
        result.relations.temporal,
        TransitionTemporalRelation::TemporalDeclarationMismatch
    );
    assert!(
        result
            .reason_codes
            .contains(&TransitionReasonCode::PredecessorMismatch)
    );
}

#[test]
fn maximum_sequence_is_a_chain_mismatch_without_overflow() {
    let mut previous = prior_entering(TransitionState::NeedsEvidence);
    previous.sequence = i64::MAX;
    previous.previous_receipt_sha256 = Some("a".repeat(64));
    previous.previous_receipt_id = Some(format!("transition-receipt-{}", "a".repeat(64)));
    reseal_receipt(&mut previous);

    let mut current = fixture().transition_receipt;
    current.sequence = i64::MAX;
    current.previous_receipt_id = Some(previous.receipt_id.clone());
    current.previous_receipt_sha256 = Some(previous.receipt_sha256.clone());
    current.transition.from_state = TransitionState::NeedsEvidence;
    current.transition.to_state = TransitionState::Baselined;
    current.applicability.stage_id = "BASELINED".into();
    reseal_receipt(&mut current);
    assert_eq!(
        assessment_for(current, Some(previous)).relations.chain,
        ChainRelation::PredecessorMismatch
    );
}

#[test]
fn fail_or_unknown_preconditions_never_become_authority() {
    let mut current = fixture().transition_receipt;
    current.preconditions[0].result = super::super::PreconditionResult::Unknown;
    reseal_receipt(&mut current);
    let result = assessment_for(current, None);
    assert_eq!(
        result.relations.preconditions,
        PreconditionsRelation::DeclaredFailOrUnknownPresent
    );
    assert!(!result.transition_attestation);
    assert!(!result.permission_attestation);
}

#[test]
fn dynamic_resume_and_rework_use_explicit_predecessor_only() {
    let info = prior_entering(TransitionState::NeedsInfo);
    let blocked = successor(&info, TransitionState::Blocked);
    assert_eq!(
        assessment_for(blocked.clone(), Some(info.clone()))
            .relations
            .recovery,
        RecoveryRelation::InternallyConsistentDeclaredRecovery
    );
    let mut wrong = blocked;
    wrong.transition.resume_state = Some(TransitionState::Baselined);
    reseal_receipt(&mut wrong);
    assert_eq!(
        assessment_for(wrong, Some(info)).relations.recovery,
        RecoveryRelation::ReworkOrResumeMismatch
    );
    let changes = prior_entering(TransitionState::ChangesRequested);
    let wrong_target = successor(&changes, TransitionState::Designed);
    assert_eq!(
        assessment_for(wrong_target, Some(changes))
            .relations
            .recovery,
        RecoveryRelation::ReworkOrResumeMismatch
    );
}

fn request_for(
    current: super::super::TransitionReceipt,
    previous: Option<super::super::TransitionReceipt>,
) -> super::super::TransitionAssessmentRequest {
    let mut request = fixture().assessment_request;
    request.evaluated_at_unix_ms = current.transition.declared_at_unix_ms;
    request.expected_target = super::super::declared_target(&current).unwrap();
    request.previous_receipt = previous;
    request.transition_receipt = current;
    reseal_request(&mut request);
    request
}

fn assessment_for(
    current: super::super::TransitionReceipt,
    previous: Option<super::super::TransitionReceipt>,
) -> super::super::TransitionDeclaredAssessment {
    evaluate_declared_assessment(&request_for(current, previous)).unwrap()
}
