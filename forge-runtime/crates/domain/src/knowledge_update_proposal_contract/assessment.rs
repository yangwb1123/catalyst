use super::{
    ASSESSMENT_API_VERSION, ASSESSMENT_MODE, ASSESSMENT_REQUEST_API_VERSION, ASSESSMENT_RESULT,
    BindingRelation, CANONICALIZATION, GrantRefRelation, KnowledgeUpdateAssessmentRequest,
    KnowledgeUpdateDeclaredAssessment, KnowledgeUpdateDeclaredRelations,
    KnowledgeUpdateDeclaredTarget, KnowledgeUpdateProposal, KnowledgeUpdateProposalContractError,
    KnowledgeUpdateReasonCode, MutationsRelation, NoDecision, NotEvaluated, ProposerRelation,
    RecordSetRelation, ScopeRelation, TaskBindingRelation, TemporalRelation, codec, invalid,
    validation,
};

/// Seals and validates a proposal supplied with blank identity fields.
///
/// # Errors
///
/// Returns an error when the proposal declarations, record set, lifecycle, or bounds are invalid.
pub fn seal_proposal(
    proposal: &KnowledgeUpdateProposal,
) -> Result<KnowledgeUpdateProposal, KnowledgeUpdateProposalContractError> {
    validation::validate_unsealed_proposal(proposal)?;
    let mut sealed = proposal.clone();
    sealed.proposal_sha256 = codec::proposal_sha256_unchecked(&sealed)?;
    sealed.proposal_id = format!("knowledge-update-proposal-{}", sealed.proposal_sha256);
    validation::validate_proposal(&sealed)?;
    Ok(sealed)
}

/// Seals a strict declared-assessment request from explicit caller declarations.
///
/// # Errors
///
/// Returns an error when the proposal, target, evaluation time, or derived request is invalid.
pub fn seal_assessment_request(
    proposal: &KnowledgeUpdateProposal,
    expected_target: &KnowledgeUpdateDeclaredTarget,
    evaluated_at_unix_ms: i64,
) -> Result<KnowledgeUpdateAssessmentRequest, KnowledgeUpdateProposalContractError> {
    validation::validate_proposal(proposal)?;
    validation::validate_target(expected_target)?;
    let mut request = KnowledgeUpdateAssessmentRequest {
        api_version: ASSESSMENT_REQUEST_API_VERSION.into(),
        canonicalization: CANONICALIZATION.into(),
        evaluated_at_unix_ms,
        expected_target: expected_target.clone(),
        expected_target_sha256: codec::declared_target_sha256(expected_target)?,
        knowledge_update_proposal: proposal.clone(),
        request_sha256: String::new(),
    };
    request.request_sha256 = codec::assessment_request_sha256_unchecked(&request)?;
    validation::validate_assessment_request(&request)?;
    Ok(request)
}

/// Evaluates caller-declared relations without truth, authority, persistence, or apply.
///
/// # Errors
///
/// Returns an error when the request is invalid or the bounded assessment cannot be encoded.
pub fn evaluate_declared_assessment(
    request: &KnowledgeUpdateAssessmentRequest,
) -> Result<KnowledgeUpdateDeclaredAssessment, KnowledgeUpdateProposalContractError> {
    validation::validate_assessment_request(request)?;
    let relations = evaluate_relations(request);
    let proposal = &request.knowledge_update_proposal;
    let mut assessment = KnowledgeUpdateDeclaredAssessment {
        api_version: ASSESSMENT_API_VERSION.into(),
        assessment_mode: ASSESSMENT_MODE.into(),
        assessment_sha256: String::new(),
        authorization_decision: NoDecision::None,
        canonicalization: CANONICALIZATION.into(),
        conflict_state: NotEvaluated::NotEvaluated,
        context_state: NotEvaluated::NotEvaluated,
        current_knowledge_state: NotEvaluated::NotEvaluated,
        effect_attestation: false,
        evidence_state: NotEvaluated::NotEvaluated,
        execution_attestation: false,
        expected_target_sha256: request.expected_target_sha256.clone(),
        freshness_state: NotEvaluated::NotEvaluated,
        grant_state: NotEvaluated::NotEvaluated,
        knowledge_adoption_attestation: false,
        permission_attestation: false,
        persistence_attestation: false,
        policy_decision: NoDecision::None,
        proposal_id: proposal.proposal_id.clone(),
        proposal_sha256: proposal.proposal_sha256.clone(),
        proposer_authentication_state: NotEvaluated::NotEvaluated,
        reason_codes: expected_reason_codes(&relations),
        relations,
        request_sha256: request.request_sha256.clone(),
        result: ASSESSMENT_RESULT.into(),
        truth_attestation: false,
    };
    assessment.assessment_sha256 = codec::assessment_sha256(&assessment)?;
    validation::validate_assessment_shape(&assessment)?;
    Ok(assessment)
}

/// Re-evaluates and requires exact byte-semantic equality.
///
/// # Errors
///
/// Returns an error when either input is invalid or the assessment differs from re-evaluation.
pub fn validate_assessment(
    request: &KnowledgeUpdateAssessmentRequest,
    assessment: &KnowledgeUpdateDeclaredAssessment,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    validation::validate_assessment_shape(assessment)?;
    let expected = evaluate_declared_assessment(request)?;
    if expected == *assessment {
        Ok(())
    } else {
        Err(invalid(
            "assessment differs from fresh authority-neutral declared assessment",
        ))
    }
}

fn evaluate_relations(
    request: &KnowledgeUpdateAssessmentRequest,
) -> KnowledgeUpdateDeclaredRelations {
    let expected = &request.expected_target;
    let actual = validation::declared_target_unchecked(&request.knowledge_update_proposal);
    KnowledgeUpdateDeclaredRelations {
        binding: relation(
            expected.bindings == actual.bindings,
            BindingRelation::SameDeclaredBinding,
            BindingRelation::BindingMismatch,
        ),
        grant_ref: relation(
            expected.capability_grant_ref == actual.capability_grant_ref,
            GrantRefRelation::SameDeclaredGrantRef,
            GrantRefRelation::GrantRefMismatch,
        ),
        mutations: relation(
            expected.mutations == actual.mutations,
            MutationsRelation::SameDeclaredMutations,
            MutationsRelation::MutationsMismatch,
        ),
        proposer: relation(
            expected.proposer == actual.proposer,
            ProposerRelation::SameDeclaredProposer,
            ProposerRelation::ProposerMismatch,
        ),
        record_set: relation(
            expected.record_set_sha256 == actual.record_set_sha256,
            RecordSetRelation::SameDeclaredRecordSet,
            RecordSetRelation::RecordSetMismatch,
        ),
        scope: relation(
            expected.knowledge_scope == actual.knowledge_scope,
            ScopeRelation::SameDeclaredScope,
            ScopeRelation::ScopeMismatch,
        ),
        task_binding: relation(
            expected.task_binding == actual.task_binding,
            TaskBindingRelation::SameDeclaredTaskBinding,
            TaskBindingRelation::TaskBindingMismatch,
        ),
        temporal: relation(
            request.knowledge_update_proposal.submitted_at_unix_ms <= request.evaluated_at_unix_ms,
            TemporalRelation::NonfutureDeclaredSubmission,
            TemporalRelation::FutureDeclaredSubmission,
        ),
    }
}

fn relation<T: Copy>(same: bool, positive: T, negative: T) -> T {
    if same { positive } else { negative }
}

pub(super) fn expected_reason_codes(
    relations: &KnowledgeUpdateDeclaredRelations,
) -> Vec<KnowledgeUpdateReasonCode> {
    let mut reasons = Vec::new();
    if relations.binding == BindingRelation::BindingMismatch {
        reasons.push(KnowledgeUpdateReasonCode::BindingMismatch);
    }
    if relations.grant_ref == GrantRefRelation::GrantRefMismatch {
        reasons.push(KnowledgeUpdateReasonCode::GrantRefMismatch);
    }
    if relations.mutations == MutationsRelation::MutationsMismatch {
        reasons.push(KnowledgeUpdateReasonCode::MutationsMismatch);
    }
    if relations.proposer == ProposerRelation::ProposerMismatch {
        reasons.push(KnowledgeUpdateReasonCode::ProposerMismatch);
    }
    if relations.record_set == RecordSetRelation::RecordSetMismatch {
        reasons.push(KnowledgeUpdateReasonCode::RecordSetMismatch);
    }
    if relations.scope == ScopeRelation::ScopeMismatch {
        reasons.push(KnowledgeUpdateReasonCode::ScopeMismatch);
    }
    if relations.task_binding == TaskBindingRelation::TaskBindingMismatch {
        reasons.push(KnowledgeUpdateReasonCode::TaskBindingMismatch);
    }
    if relations.temporal == TemporalRelation::FutureDeclaredSubmission {
        reasons.push(KnowledgeUpdateReasonCode::TemporalDeclarationMismatch);
    }
    reasons
}
