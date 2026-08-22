use super::{
    ASSESSMENT_API_VERSION, ASSESSMENT_MODE, ASSESSMENT_RESULT, ApprovalAssessmentRequest,
    ApprovalBindingRelation, ApprovalDeclaredAssessment, ApprovalDeclaredRelations,
    ApprovalDeclaredTarget, ApprovalReasonCode, ApprovalRecordContractError, ApprovalScopeRelation,
    ApprovalSubjectRelation, ApprovalTemporalRelation, ApproverRelation, AuthorityBindingRelation,
    CANONICALIZATION, ConditionsRelation, DecisionRelation, NoDecision, NotEvaluated,
    RevocationRelation, RiskAcceptanceRelation, SeparationOfDutyRelation, codec, invalid,
};

/// Evaluates caller-declared relations without authenticating or authorizing them.
///
/// # Errors
///
/// Returns an error when the request violates the strict pure contract.
pub fn evaluate_declared_assessment(
    request: &ApprovalAssessmentRequest,
) -> Result<ApprovalDeclaredAssessment, ApprovalRecordContractError> {
    super::assessment_validation::validate_request(request, false)?;
    let relations = evaluate_relations(request);
    let reason_codes = expected_reason_codes(&relations);
    let record = &request.approval_record;
    let mut assessment = ApprovalDeclaredAssessment {
        api_version: ASSESSMENT_API_VERSION.into(),
        approval_id: record.approval_id.clone(),
        approval_sha256: record.approval_sha256.clone(),
        approver_identity_state: NotEvaluated::NotEvaluated,
        assessment_mode: ASSESSMENT_MODE.into(),
        assessment_sha256: String::new(),
        authority_proof_state: NotEvaluated::NotEvaluated,
        authorization_decision: NoDecision::None,
        canonicalization: CANONICALIZATION.into(),
        condition_satisfaction_state: NotEvaluated::NotEvaluated,
        effect_attestation: false,
        effective_approval_state: NotEvaluated::NotEvaluated,
        expected_target_sha256: request.expected_target_sha256.clone(),
        permission_attestation: false,
        persistence_attestation: false,
        policy_decision: NoDecision::None,
        reason_codes,
        relations,
        request_sha256: request.request_sha256.clone(),
        result: ASSESSMENT_RESULT.into(),
        revocation_registry_state: NotEvaluated::NotEvaluated,
        risk_acceptance_state: NotEvaluated::NotEvaluated,
        separation_of_duty_proof_state: NotEvaluated::NotEvaluated,
        transition_attestation: false,
    };
    assessment.assessment_sha256 = codec::assessment_sha256_unchecked(&assessment)?;
    super::assessment_validation::validate_assessment_shape(&assessment)?;
    Ok(assessment)
}

/// Re-evaluates and requires an exact authority-neutral assessment.
///
/// # Errors
///
/// Returns an error for malformed inputs or any derived-field drift.
pub fn validate_assessment(
    request: &ApprovalAssessmentRequest,
    assessment: &ApprovalDeclaredAssessment,
) -> Result<(), ApprovalRecordContractError> {
    super::assessment_validation::validate_assessment_shape(assessment)?;
    if evaluate_declared_assessment(request)? == *assessment {
        Ok(())
    } else {
        Err(invalid(
            "assessment is not the exact authority-neutral reassembly",
        ))
    }
}

fn evaluate_relations(request: &ApprovalAssessmentRequest) -> ApprovalDeclaredRelations {
    let actual = super::record_validation::declared_target(&request.approval_record);
    let expected = &request.expected_target;
    let validity = &request.approval_record.validity;
    let evaluated = request.evaluated_at_unix_ms;
    ApprovalDeclaredRelations {
        approver: approver_relation(&actual, expected),
        authority_binding: authority_binding_relation(&actual, expected),
        binding: binding_relation(&actual, expected),
        conditions: conditions_relation(&actual, expected),
        decision: decision_relation(&actual, expected),
        revocation: revocation_relation(validity.revoked_at_unix_ms, evaluated),
        risk_acceptance: risk_acceptance_relation(&actual, expected),
        scope: scope_relation(&actual, expected),
        separation_of_duty: separation_of_duty_relation(&actual, expected),
        subject: subject_relation(&actual, expected),
        temporal: temporal_relation(validity, evaluated),
    }
}

fn approver_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> ApproverRelation {
    relation(
        actual.approver == expected.approver,
        ApproverRelation::SameDeclaredApprover,
        ApproverRelation::ApproverMismatch,
    )
}

fn authority_binding_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> AuthorityBindingRelation {
    relation(
        actual.authority_binding == expected.authority_binding,
        AuthorityBindingRelation::SameDeclaredAuthorityBinding,
        AuthorityBindingRelation::AuthorityBindingMismatch,
    )
}

fn binding_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> ApprovalBindingRelation {
    relation(
        actual.bindings == expected.bindings,
        ApprovalBindingRelation::SameDeclaredBinding,
        ApprovalBindingRelation::BindingMismatch,
    )
}

fn conditions_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> ConditionsRelation {
    relation(
        actual.conditions == expected.conditions,
        ConditionsRelation::SameDeclaredConditions,
        ConditionsRelation::ConditionsMismatch,
    )
}

fn decision_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> DecisionRelation {
    relation(
        actual.decision == expected.decision,
        DecisionRelation::SameDeclaredDecision,
        DecisionRelation::DecisionMismatch,
    )
}

fn risk_acceptance_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> RiskAcceptanceRelation {
    relation(
        actual.risk_acceptance_refs == expected.risk_acceptance_refs,
        RiskAcceptanceRelation::SameDeclaredRiskAcceptanceRefs,
        RiskAcceptanceRelation::RiskAcceptanceMismatch,
    )
}

fn scope_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> ApprovalScopeRelation {
    relation(
        actual.scope == expected.scope,
        ApprovalScopeRelation::SameDeclaredScope,
        ApprovalScopeRelation::ScopeMismatch,
    )
}

fn separation_of_duty_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> SeparationOfDutyRelation {
    relation(
        actual.separation_of_duty_declaration == expected.separation_of_duty_declaration,
        SeparationOfDutyRelation::SameDeclaredSeparationOfDuty,
        SeparationOfDutyRelation::SeparationOfDutyMismatch,
    )
}

fn subject_relation(
    actual: &ApprovalDeclaredTarget,
    expected: &ApprovalDeclaredTarget,
) -> ApprovalSubjectRelation {
    relation(
        actual.subject == expected.subject,
        ApprovalSubjectRelation::SameDeclaredSubject,
        ApprovalSubjectRelation::SubjectMismatch,
    )
}

fn relation<T: Copy>(same: bool, positive: T, negative: T) -> T {
    if same { positive } else { negative }
}

fn revocation_relation(revoked: Option<i64>, evaluated: i64) -> RevocationRelation {
    if revoked.is_some_and(|instant| evaluated >= instant) {
        RevocationRelation::DeclaredRevocationTimeReached
    } else {
        RevocationRelation::DeclaredRevocationTimeNotReached
    }
}

fn temporal_relation(
    validity: &super::ApprovalValidity,
    evaluated: i64,
) -> ApprovalTemporalRelation {
    if validity.not_before_unix_ms <= evaluated && evaluated < validity.expires_at_unix_ms {
        ApprovalTemporalRelation::InsideDeclaredWindow
    } else {
        ApprovalTemporalRelation::OutsideDeclaredWindow
    }
}

pub(super) fn expected_reason_codes(
    relations: &ApprovalDeclaredRelations,
) -> Vec<ApprovalReasonCode> {
    let mut reasons: Vec<_> = primary_reason_candidates(relations)
        .into_iter()
        .chain(secondary_reason_candidates(relations))
        .filter_map(|(include, reason)| include.then_some(reason))
        .collect();
    reasons.sort_unstable_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    reasons
}

fn primary_reason_candidates(
    relations: &ApprovalDeclaredRelations,
) -> [(bool, ApprovalReasonCode); 6] {
    [
        (
            relations.approver == ApproverRelation::ApproverMismatch,
            ApprovalReasonCode::ApproverMismatch,
        ),
        (
            relations.authority_binding == AuthorityBindingRelation::AuthorityBindingMismatch,
            ApprovalReasonCode::AuthorityBindingMismatch,
        ),
        (
            relations.binding == ApprovalBindingRelation::BindingMismatch,
            ApprovalReasonCode::BindingMismatch,
        ),
        (
            relations.conditions == ConditionsRelation::ConditionsMismatch,
            ApprovalReasonCode::ConditionsMismatch,
        ),
        (
            relations.revocation == RevocationRelation::DeclaredRevocationTimeReached,
            ApprovalReasonCode::DeclaredRevocationTimeReached,
        ),
        (
            relations.decision == DecisionRelation::DecisionMismatch,
            ApprovalReasonCode::DecisionMismatch,
        ),
    ]
}

fn secondary_reason_candidates(
    relations: &ApprovalDeclaredRelations,
) -> [(bool, ApprovalReasonCode); 5] {
    [
        (
            relations.risk_acceptance == RiskAcceptanceRelation::RiskAcceptanceMismatch,
            ApprovalReasonCode::RiskAcceptanceMismatch,
        ),
        (
            relations.scope == ApprovalScopeRelation::ScopeMismatch,
            ApprovalReasonCode::ScopeMismatch,
        ),
        (
            relations.separation_of_duty == SeparationOfDutyRelation::SeparationOfDutyMismatch,
            ApprovalReasonCode::SeparationOfDutyMismatch,
        ),
        (
            relations.subject == ApprovalSubjectRelation::SubjectMismatch,
            ApprovalReasonCode::SubjectMismatch,
        ),
        (
            relations.temporal == ApprovalTemporalRelation::OutsideDeclaredWindow,
            ApprovalReasonCode::TemporalWindowMismatch,
        ),
    ]
}
