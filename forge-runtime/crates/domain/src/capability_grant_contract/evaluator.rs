use super::{
    ASSESSMENT_API_VERSION, ASSESSMENT_MODE, ASSESSMENT_RESULT, BindingRelation, BudgetRelation,
    CANONICALIZATION, CapabilityGrantContractError, CapabilityRelation, DeclaredAssessment,
    DeclaredAssessmentRequest, DeclaredRelations, EffectRelation, EffectVocabulary,
    NoAuthorizationDecision, NotEvaluated, ReasonCode, ScopeRelation, SubjectRelation,
    TaskRelation, TemporalRelation, codec, invalid, scope_validation, validation,
};

/// Evaluates declared relations without producing an authority decision.
///
/// # Errors
///
/// Returns an error when the vocabulary or request violates the strict contract.
pub fn evaluate_declared_assessment(
    vocabulary: &EffectVocabulary,
    request: &DeclaredAssessmentRequest,
) -> Result<DeclaredAssessment, CapabilityGrantContractError> {
    validation::validate_vocabulary(vocabulary)?;
    validation::validate_assessment_request(request)?;
    let (relations, reason_codes) = evaluate_relations(request);
    let mut assessment = DeclaredAssessment {
        api_version: ASSESSMENT_API_VERSION.into(),
        approval_state: NotEvaluated::NotEvaluated,
        assessment_mode: ASSESSMENT_MODE.into(),
        assessment_sha256: String::new(),
        authority_proof_state: NotEvaluated::NotEvaluated,
        authorization_decision: NoAuthorizationDecision::None,
        canonicalization: CANONICALIZATION.into(),
        effect_attestation: false,
        grant_id: request.grant.grant_id.clone(),
        grant_sha256: request.grant.grant_sha256.clone(),
        permission_attestation: false,
        reason_codes,
        relations,
        request_sha256: request.request_sha256.clone(),
        requested_action_sha256: codec::requested_action_sha256(&request.requested_action)?,
        result: ASSESSMENT_RESULT.into(),
        revocation_state: NotEvaluated::NotEvaluated,
        usage_state: NotEvaluated::NotEvaluated,
    };
    assessment.assessment_sha256 = codec::assessment_sha256(&assessment)?;
    validation::validate_assessment_shape(&assessment)?;
    Ok(assessment)
}

/// Re-evaluates a declared assessment and requires exact equality.
///
/// # Errors
///
/// Returns an error for malformed inputs or any derived-field drift.
pub fn validate_assessment(
    vocabulary: &EffectVocabulary,
    request: &DeclaredAssessmentRequest,
    assessment: &DeclaredAssessment,
) -> Result<(), CapabilityGrantContractError> {
    validation::validate_assessment_shape(assessment)?;
    let expected = evaluate_declared_assessment(vocabulary, request)?;
    if expected == *assessment {
        Ok(())
    } else {
        Err(invalid(
            "assessment differs from fresh authority-neutral declared assessment",
        ))
    }
}

fn evaluate_relations(request: &DeclaredAssessmentRequest) -> (DeclaredRelations, Vec<ReasonCode>) {
    let grant = &request.grant;
    let expected = &request.expected;
    let action = &request.requested_action;
    let binding = relation(
        grant.bindings == expected.bindings,
        BindingRelation::SameDeclaredBinding,
        BindingRelation::BindingMismatch,
    );
    let capability = relation(
        grant.capability == expected.capability,
        CapabilityRelation::SameDeclaredCapability,
        CapabilityRelation::CapabilityMismatch,
    );
    let subject = relation(
        grant.subject == expected.subject,
        SubjectRelation::SameDeclaredSubject,
        SubjectRelation::SubjectMismatch,
    );
    let task = relation(
        grant.task_binding == expected.task_binding,
        TaskRelation::SameDeclaredTask,
        TaskRelation::TaskMismatch,
    );
    let effect = relation(
        grant.scope.effect_id == action.effect_id,
        EffectRelation::SameDeclaredEffect,
        EffectRelation::EffectMismatch,
    );
    let scope = scope_validation::scope_relation(&grant.scope, action);
    let budget = budget_relation(request);
    let temporal = temporal_relation(request);
    let relations = DeclaredRelations {
        binding,
        budget,
        capability,
        effect,
        scope,
        subject,
        task,
        temporal,
    };
    let reasons = expected_reason_codes(&relations);
    (relations, reasons)
}

fn relation<T: Copy>(same: bool, positive: T, negative: T) -> T {
    if same { positive } else { negative }
}

fn budget_relation(request: &DeclaredAssessmentRequest) -> BudgetRelation {
    let usage = &request.requested_action.usage;
    let budget = &request.grant.budget;
    let within = usage.call_count <= budget.max_calls
        && usage.cost_usd_micros <= budget.max_cost_usd_micros
        && usage.input_tokens <= budget.max_input_tokens
        && usage.network_bytes <= budget.max_network_bytes
        && usage.output_bytes <= budget.max_output_bytes
        && usage.output_tokens <= budget.max_output_tokens
        && usage.timeout_ms <= budget.timeout_ms;
    relation(
        within,
        BudgetRelation::AtOrBelowDeclaredCeiling,
        BudgetRelation::ExceedsDeclaredCeiling,
    )
}

fn temporal_relation(request: &DeclaredAssessmentRequest) -> TemporalRelation {
    let validity = &request.grant.validity;
    let inside = request.evaluated_at_unix_ms >= validity.not_before_unix_ms
        && request.evaluated_at_unix_ms < validity.expires_at_unix_ms;
    relation(
        inside,
        TemporalRelation::InsideDeclaredWindow,
        TemporalRelation::OutsideDeclaredWindow,
    )
}

pub(super) fn expected_reason_codes(relations: &DeclaredRelations) -> Vec<ReasonCode> {
    let candidates = [
        (
            relations.binding == BindingRelation::BindingMismatch,
            ReasonCode::BindingMismatch,
        ),
        (
            relations.budget == BudgetRelation::ExceedsDeclaredCeiling,
            ReasonCode::BudgetExceeded,
        ),
        (
            relations.capability == CapabilityRelation::CapabilityMismatch,
            ReasonCode::CapabilityMismatch,
        ),
        (
            relations.scope == ScopeRelation::DeniedByDeclaration,
            ReasonCode::DenyMatched,
        ),
        (
            relations.effect == EffectRelation::EffectMismatch,
            ReasonCode::EffectMismatch,
        ),
        (scope_not_covered(relations), ReasonCode::ScopeNotCovered),
        (
            relations.subject == SubjectRelation::SubjectMismatch,
            ReasonCode::SubjectMismatch,
        ),
        (
            relations.task == TaskRelation::TaskMismatch,
            ReasonCode::TaskMismatch,
        ),
        (
            relations.temporal == TemporalRelation::OutsideDeclaredWindow,
            ReasonCode::TemporalWindowMismatch,
        ),
    ];
    let mut reasons: Vec<_> = candidates
        .into_iter()
        .filter_map(|(include, reason)| include.then_some(reason))
        .collect();
    reasons.sort_unstable_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    reasons
}

fn scope_not_covered(relations: &DeclaredRelations) -> bool {
    relations.effect == EffectRelation::SameDeclaredEffect
        && relations.scope == ScopeRelation::OutsideDeclaredScope
}
