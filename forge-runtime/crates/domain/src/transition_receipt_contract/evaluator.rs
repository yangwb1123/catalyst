use super::{
    ASSESSMENT_API_VERSION, ASSESSMENT_MODE, ASSESSMENT_RESULT, ApplicabilityRelation,
    CANONICALIZATION, ChainRelation, ContinuityRelation, EdgeRelation, NoDecision, NotEvaluated,
    PreconditionResult, PreconditionsRelation, RecoveryRelation, TargetRelation,
    TransitionAssessmentRequest, TransitionDeclaredAssessment, TransitionDeclaredRelations,
    TransitionDeclaredTarget, TransitionReasonCode, TransitionReceipt,
    TransitionReceiptContractError, TransitionState, TransitionTemporalRelation, codec, invalid,
    vocabulary,
};

/// Projects every receipt field compared by the declared evaluator.
///
/// # Errors
///
/// Returns an error when the receipt is malformed.
pub fn declared_target(
    value: &TransitionReceipt,
) -> Result<TransitionDeclaredTarget, TransitionReceiptContractError> {
    super::validation::validate_receipt(value, false)?;
    let target = TransitionDeclaredTarget {
        actor: value.actor.clone(),
        applicability: value.applicability.clone(),
        approval_refs: value.approval_refs.clone(),
        bindings: value.bindings.clone(),
        capability_grant_ref: value.capability_grant_ref.clone(),
        declared_controller: value.declared_controller.clone(),
        preconditions: value.preconditions.clone(),
        previous_receipt_id: value.previous_receipt_id.clone(),
        previous_receipt_sha256: value.previous_receipt_sha256.clone(),
        reason_codes: value.reason_codes.clone(),
        sequence: value.sequence,
        task_binding: value.task_binding.clone(),
        transition: value.transition.clone(),
        transition_vocabulary_sha256: value.transition_vocabulary_sha256.clone(),
        waiver_refs: value.waiver_refs.clone(),
        work_id: value.work_id.clone(),
    };
    super::validation::validate_target(&target)?;
    Ok(target)
}

/// Evaluates caller-declared relations without mutating or authorizing state.
///
/// # Errors
///
/// Returns an error when the request violates the strict pure contract.
pub fn evaluate_declared_assessment(
    request: &TransitionAssessmentRequest,
) -> Result<TransitionDeclaredAssessment, TransitionReceiptContractError> {
    super::validation::validate_request(request, false)?;
    let relations = evaluate_relations(request)?;
    let mut assessment = assessment_without_digest(request, relations);
    assessment.assessment_sha256 = codec::assessment_sha256_unchecked(&assessment)?;
    super::assessment_validation::validate_assessment_shape(&assessment)?;
    Ok(assessment)
}

/// Re-evaluates and requires exact authority-neutral equality.
///
/// # Errors
///
/// Returns an error for malformed input or any derived-field drift.
pub fn validate_assessment(
    request: &TransitionAssessmentRequest,
    assessment: &TransitionDeclaredAssessment,
) -> Result<(), TransitionReceiptContractError> {
    super::assessment_validation::validate_assessment_shape(assessment)?;
    if evaluate_declared_assessment(request)? == *assessment {
        Ok(())
    } else {
        Err(invalid(
            "assessment is not the exact authority-neutral reassembly",
        ))
    }
}

fn assessment_without_digest(
    request: &TransitionAssessmentRequest,
    relations: TransitionDeclaredRelations,
) -> TransitionDeclaredAssessment {
    let receipt = &request.transition_receipt;
    let reason_codes = expected_reason_codes(&relations);
    TransitionDeclaredAssessment {
        api_version: ASSESSMENT_API_VERSION.into(),
        approval_state: NotEvaluated::NotEvaluated,
        assessment_mode: ASSESSMENT_MODE.into(),
        assessment_sha256: String::new(),
        authorization_decision: NoDecision::None,
        canonicalization: CANONICALIZATION.into(),
        completion_attestation: false,
        controller_authentication_state: NotEvaluated::NotEvaluated,
        effect_attestation: false,
        evidence_state: NotEvaluated::NotEvaluated,
        execution_attestation: false,
        expected_target_sha256: request.expected_target_sha256.clone(),
        grant_state: NotEvaluated::NotEvaluated,
        ledger_state: NotEvaluated::NotEvaluated,
        permission_attestation: false,
        persistence_attestation: false,
        policy_decision: NoDecision::None,
        precondition_truth_state: NotEvaluated::NotEvaluated,
        reason_codes,
        receipt_id: receipt.receipt_id.clone(),
        receipt_sha256: receipt.receipt_sha256.clone(),
        relations,
        request_sha256: request.request_sha256.clone(),
        result: ASSESSMENT_RESULT.into(),
        transition_attestation: false,
        transition_vocabulary_sha256: receipt.transition_vocabulary_sha256.clone(),
        waiver_state: NotEvaluated::NotEvaluated,
    }
}

fn evaluate_relations(
    request: &TransitionAssessmentRequest,
) -> Result<TransitionDeclaredRelations, TransitionReceiptContractError> {
    let current = &request.transition_receipt;
    let previous = request.previous_receipt.as_ref();
    Ok(TransitionDeclaredRelations {
        applicability: ApplicabilityRelation::InternallyConsistentDeclaredApplicability,
        chain: chain_relation(current, previous),
        continuity: continuity_relation(current, previous),
        edge: edge_relation(current, previous),
        preconditions: preconditions_relation(current),
        recovery: recovery_relation(current, previous),
        target: if request.expected_target == declared_target(current)? {
            TargetRelation::SameDeclaredTarget
        } else {
            TargetRelation::TargetMismatch
        },
        temporal: temporal_relation(request),
    })
}

fn chain_relation(
    current: &TransitionReceipt,
    previous: Option<&TransitionReceipt>,
) -> ChainRelation {
    let initial = current.sequence == 1
        && previous.is_none()
        && current.previous_receipt_id.is_none()
        && current.previous_receipt_sha256.is_none()
        && current.transition.from_state == TransitionState::Draft;
    if initial {
        return ChainRelation::InitialDeclaredChain;
    }
    let Some(previous) = previous else {
        return ChainRelation::PredecessorMismatch;
    };
    let identity = previous.sequence.checked_add(1) == Some(current.sequence)
        && current.previous_receipt_id.as_ref() == Some(&previous.receipt_id)
        && current.previous_receipt_sha256.as_ref() == Some(&previous.receipt_sha256);
    let scope = current.work_id == previous.work_id
        && current.task_binding.project_id == previous.task_binding.project_id
        && current.task_binding.change_id == previous.task_binding.change_id;
    if identity && scope {
        ChainRelation::SameDeclaredPredecessor
    } else {
        ChainRelation::PredecessorMismatch
    }
}

fn continuity_relation(
    current: &TransitionReceipt,
    previous: Option<&TransitionReceipt>,
) -> ContinuityRelation {
    let consistent = previous.map_or(
        current.transition.from_state == TransitionState::Draft,
        |old| old.transition.to_state == current.transition.from_state,
    );
    if consistent {
        ContinuityRelation::SameDeclaredStateContinuity
    } else {
        ContinuityRelation::StateContinuityMismatch
    }
}

fn edge_relation(
    current: &TransitionReceipt,
    previous: Option<&TransitionReceipt>,
) -> EdgeRelation {
    let transition = &current.transition;
    let dynamic = previous.is_some_and(|old| {
        matches!(
            transition.from_state,
            TransitionState::NeedsInfo | TransitionState::Blocked
        ) && old.transition.to_state == transition.from_state
            && old.transition.resume_state == Some(transition.to_state)
    });
    if vocabulary::statically_listed(transition.from_state, transition.to_state) || dynamic {
        EdgeRelation::ListedDeclaredEdge
    } else {
        EdgeRelation::UnlistedDeclaredEdge
    }
}

fn preconditions_relation(current: &TransitionReceipt) -> PreconditionsRelation {
    let positive = current.preconditions.iter().all(|value| {
        matches!(
            value.result,
            PreconditionResult::Pass | PreconditionResult::Na
        )
    });
    if positive {
        PreconditionsRelation::DeclaredPassOrNaOnly
    } else {
        PreconditionsRelation::DeclaredFailOrUnknownPresent
    }
}

fn recovery_relation(
    current: &TransitionReceipt,
    previous: Option<&TransitionReceipt>,
) -> RecoveryRelation {
    let consistent =
        inherited_resume_matches(current, previous) && recovery_exit_matches(current, previous);
    if consistent {
        RecoveryRelation::InternallyConsistentDeclaredRecovery
    } else {
        RecoveryRelation::ReworkOrResumeMismatch
    }
}

fn inherited_resume_matches(
    current: &TransitionReceipt,
    previous: Option<&TransitionReceipt>,
) -> bool {
    let transition = &current.transition;
    if transition.from_state != TransitionState::NeedsInfo
        || transition.to_state != TransitionState::Blocked
    {
        return true;
    }
    previous.is_some_and(|old| transition.resume_state == old.transition.resume_state)
}

fn recovery_exit_matches(
    current: &TransitionReceipt,
    previous: Option<&TransitionReceipt>,
) -> bool {
    let transition = &current.transition;
    let Some(old) = previous else {
        return !matches!(
            transition.from_state,
            TransitionState::ChangesRequested
                | TransitionState::NeedsInfo
                | TransitionState::Blocked
        );
    };
    match transition.from_state {
        TransitionState::ChangesRequested => matches_exit(
            transition.to_state,
            old.transition.rework_target,
            &[
                TransitionState::Blocked,
                TransitionState::Rejected,
                TransitionState::Superseded,
            ],
        ),
        TransitionState::NeedsInfo => matches_exit(
            transition.to_state,
            old.transition.resume_state,
            &[
                TransitionState::Blocked,
                TransitionState::Rejected,
                TransitionState::Superseded,
            ],
        ),
        TransitionState::Blocked => matches_exit(
            transition.to_state,
            old.transition.resume_state,
            &[TransitionState::Rejected, TransitionState::Superseded],
        ),
        _ => true,
    }
}

fn matches_exit(
    target: TransitionState,
    bound: Option<TransitionState>,
    escalations: &[TransitionState],
) -> bool {
    Some(target) == bound || escalations.contains(&target)
}

fn temporal_relation(request: &TransitionAssessmentRequest) -> TransitionTemporalRelation {
    let current = request.transition_receipt.transition.declared_at_unix_ms;
    let nondecreasing = current <= request.evaluated_at_unix_ms
        && request
            .previous_receipt
            .as_ref()
            .is_none_or(|old| old.transition.declared_at_unix_ms <= current);
    if nondecreasing {
        TransitionTemporalRelation::NondecreasingDeclaredTime
    } else {
        TransitionTemporalRelation::TemporalDeclarationMismatch
    }
}

pub(super) fn expected_reason_codes(
    relations: &TransitionDeclaredRelations,
) -> Vec<TransitionReasonCode> {
    let mut reasons = Vec::new();
    push_negative_relations(relations, &mut reasons);
    reasons.sort_unstable_by_key(|reason| reason.as_str());
    reasons
}

fn push_negative_relations(
    value: &TransitionDeclaredRelations,
    reasons: &mut Vec<TransitionReasonCode>,
) {
    if value.chain == ChainRelation::PredecessorMismatch {
        reasons.push(TransitionReasonCode::PredecessorMismatch);
    }
    if value.continuity == ContinuityRelation::StateContinuityMismatch {
        reasons.push(TransitionReasonCode::StateContinuityMismatch);
    }
    if value.edge == EdgeRelation::UnlistedDeclaredEdge {
        reasons.push(TransitionReasonCode::UnlistedDeclaredEdge);
    }
    if value.preconditions == PreconditionsRelation::DeclaredFailOrUnknownPresent {
        reasons.push(TransitionReasonCode::DeclaredFailOrUnknownPresent);
    }
    if value.recovery == RecoveryRelation::ReworkOrResumeMismatch {
        reasons.push(TransitionReasonCode::ReworkOrResumeMismatch);
    }
    if value.target == TargetRelation::TargetMismatch {
        reasons.push(TransitionReasonCode::TargetMismatch);
    }
    if value.temporal == TransitionTemporalRelation::TemporalDeclarationMismatch {
        reasons.push(TransitionReasonCode::TemporalDeclarationMismatch);
    }
}
