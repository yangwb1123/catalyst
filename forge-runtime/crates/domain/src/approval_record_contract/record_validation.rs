use crate::capability_grant_contract::{AuthorityClass, EffectId, EnvironmentClass};

use super::{
    APPROVAL_API_VERSION, APPROVAL_KIND, ApprovalArtifact, ApprovalAuthorityBinding,
    ApprovalAuthorityProof, ApprovalAuthoritySource, ApprovalBindings, ApprovalCondition,
    ApprovalDecision, ApprovalDecisionBasis, ApprovalDeclaredTarget, ApprovalRecord,
    ApprovalRecordContractError, ApprovalRequiredDistinction, ApprovalScope, ApprovalScopeType,
    ApprovalSeparationOfDuty, ApprovalSeparationOfDutyDeclaration, ApprovalValidity,
    CANONICALIZATION, EFFECT_VOCABULARY_SHA256, MAX_RECORD_BYTES, MAX_VALIDITY_MS,
    MaterialityLevel, RiskAcceptanceRef, codec, invalid, primitives,
};

pub(super) fn validate_record(
    value: &ApprovalRecord,
    skip_identity: bool,
) -> Result<(), ApprovalRecordContractError> {
    if value.api_version != APPROVAL_API_VERSION
        || value.kind != APPROVAL_KIND
        || value.canonicalization != CANONICALIZATION
        || value.effect_vocabulary_sha256 != EFFECT_VOCABULARY_SHA256
    {
        return Err(invalid(
            "ApprovalRecord envelope or vocabulary binding drifted",
        ));
    }
    validate_record_parts(value)?;
    let encoded = codec::canonical_unbounded(value)?;
    if encoded.len() > MAX_RECORD_BYTES {
        return Err(invalid("ApprovalRecord exceeds its canonical byte limit"));
    }
    if skip_identity && value.approval_id.is_empty() && value.approval_sha256.is_empty() {
        return Ok(());
    }
    if !skip_identity || !value.approval_id.is_empty() || !value.approval_sha256.is_empty() {
        primitives::sha256(&value.approval_sha256, "approval_sha256")?;
        if value.approval_id != format!("approval-record-{}", value.approval_sha256)
            || codec::approval_sha256_unchecked(value)? != value.approval_sha256
        {
            return Err(invalid(
                "ApprovalRecord identity or self digest does not match",
            ));
        }
    }
    Ok(())
}

fn validate_record_parts(value: &ApprovalRecord) -> Result<(), ApprovalRecordContractError> {
    primitives::approver(&value.approver, "approver")?;
    primitives::principal(&value.subject, "subject")?;
    validate_authority_proof(&value.authority_proof)?;
    validate_bindings(&value.bindings, "bindings")?;
    validate_conditions(&value.conditions, "conditions")?;
    validate_decision_basis(&value.decision_basis)?;
    validate_risk_refs(&value.risk_acceptance_refs, "risk_acceptance_refs")?;
    validate_scope(&value.scope, "scope")?;
    validate_sod(&value.separation_of_duty, value)?;
    validate_validity(&value.validity)
}

pub(super) fn validate_target(
    value: &ApprovalDeclaredTarget,
) -> Result<(), ApprovalRecordContractError> {
    primitives::approver(&value.approver, "declared target.approver")?;
    primitives::principal(&value.subject, "declared target.subject")?;
    validate_authority_binding(
        &value.authority_binding,
        "declared target.authority_binding",
    )?;
    validate_bindings(&value.bindings, "declared target.bindings")?;
    validate_conditions(&value.conditions, "declared target.conditions")?;
    validate_risk_refs(
        &value.risk_acceptance_refs,
        "declared target.risk_acceptance_refs",
    )?;
    validate_scope(&value.scope, "declared target.scope")?;
    validate_sod_declaration(
        &value.separation_of_duty_declaration,
        "declared target.separation_of_duty_declaration",
    )?;
    validate_consistency(
        &value.approver,
        &value.subject,
        &value.authority_binding.authority_source,
        &value.risk_acceptance_refs,
        &value.scope,
        &value.separation_of_duty_declaration,
        value.decision,
    )?;
    let encoded = codec::canonical_unbounded(value)?;
    if encoded.len() > super::MAX_DECLARED_TARGET_BYTES {
        Err(invalid("declared target exceeds its canonical byte limit"))
    } else {
        Ok(())
    }
}

pub(super) fn declared_target(value: &ApprovalRecord) -> ApprovalDeclaredTarget {
    let proof = &value.authority_proof;
    let sod = &value.separation_of_duty;
    ApprovalDeclaredTarget {
        approver: value.approver.clone(),
        authority_binding: ApprovalAuthorityBinding {
            authority_source: proof.authority_source.clone(),
            key_id: proof.key_id.clone(),
            proof_kind: proof.proof_kind,
            proof_profile_id: proof.proof_profile_id.clone(),
            proof_profile_sha256: proof.proof_profile_sha256.clone(),
            trust_domain: proof.trust_domain.clone(),
            trust_epoch: proof.trust_epoch,
        },
        bindings: value.bindings.clone(),
        conditions: value.conditions.clone(),
        decision: value.decision,
        risk_acceptance_refs: value.risk_acceptance_refs.clone(),
        scope: value.scope.clone(),
        separation_of_duty_declaration: ApprovalSeparationOfDutyDeclaration {
            implementers: sod.implementers.clone(),
            proof_profile_id: sod.proof_profile_id.clone(),
            proof_profile_sha256: sod.proof_profile_sha256.clone(),
            requester: sod.requester.clone(),
            required_distinctions: sod.required_distinctions.clone(),
        },
        subject: value.subject.clone(),
    }
}

fn validate_authority_source(
    value: &ApprovalAuthoritySource,
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    primitives::short_text(
        &value.authority_domain,
        &format!("{label}.authority_domain"),
    )?;
    primitives::short_text(&value.principal_id, &format!("{label}.principal_id"))?;
    primitives::authority_source(value.authority_class, value.principal_type)
}

fn validate_authority_binding(
    value: &ApprovalAuthorityBinding,
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    validate_authority_source(
        &value.authority_source,
        &format!("{label}.authority_source"),
    )?;
    primitives::short_text(&value.key_id, &format!("{label}.key_id"))?;
    primitives::short_text(
        &value.proof_profile_id,
        &format!("{label}.proof_profile_id"),
    )?;
    primitives::sha256(
        &value.proof_profile_sha256,
        &format!("{label}.proof_profile_sha256"),
    )?;
    primitives::short_text(&value.trust_domain, &format!("{label}.trust_domain"))?;
    primitives::nonnegative(value.trust_epoch, &format!("{label}.trust_epoch"))
}

fn validate_authority_proof(
    value: &ApprovalAuthorityProof,
) -> Result<(), ApprovalRecordContractError> {
    let binding = ApprovalAuthorityBinding {
        authority_source: value.authority_source.clone(),
        key_id: value.key_id.clone(),
        proof_kind: value.proof_kind,
        proof_profile_id: value.proof_profile_id.clone(),
        proof_profile_sha256: value.proof_profile_sha256.clone(),
        trust_domain: value.trust_domain.clone(),
        trust_epoch: value.trust_epoch,
    };
    validate_authority_binding(&binding, "authority_proof")?;
    primitives::base64url(&value.proof_base64url, "authority_proof.proof_base64url")
}

fn validate_bindings(
    value: &ApprovalBindings,
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    if value.artifacts.is_empty() || value.artifacts.len() > 32 {
        return Err(invalid(format!(
            "{label}.artifacts must contain 1..32 items"
        )));
    }
    for artifact in &value.artifacts {
        validate_artifact(artifact, &format!("{label}.artifacts"))?;
    }
    primitives::strictly_sorted(&value.artifacts, &format!("{label}.artifacts"))?;
    for (field, digest) in [
        ("context_sha256", &value.context_sha256),
        ("impact_sha256", &value.impact_sha256),
        ("plan_sha256", &value.plan_sha256),
        ("policy_sha256", &value.policy_sha256),
        ("risk_sha256", &value.risk_sha256),
        ("source_tree_sha256", &value.source_tree_sha256),
    ] {
        primitives::sha256(digest, &format!("{label}.{field}"))?;
    }
    primitives::short_text(&value.source_revision, &format!("{label}.source_revision"))
}

fn validate_artifact(
    value: &ApprovalArtifact,
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    primitives::short_text(&value.artifact_kind, &format!("{label}.artifact_kind"))?;
    primitives::text(&value.artifact_ref, 4_096, &format!("{label}.artifact_ref"))?;
    primitives::sha256(&value.artifact_sha256, &format!("{label}.artifact_sha256"))
}

fn validate_conditions(
    values: &[ApprovalCondition],
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    if values.len() > 32 {
        return Err(invalid(format!("{label} may contain at most 32 items")));
    }
    for value in values {
        primitives::short_text(&value.condition_id, &format!("{label}.condition_id"))?;
        primitives::text(
            &value.condition_ref,
            4_096,
            &format!("{label}.condition_ref"),
        )?;
        primitives::sha256(
            &value.condition_sha256,
            &format!("{label}.condition_sha256"),
        )?;
    }
    primitives::strictly_sorted(values, label)
}

fn validate_risk_refs(
    values: &[RiskAcceptanceRef],
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    if values.len() > 32 {
        return Err(invalid(format!("{label} may contain at most 32 items")));
    }
    for value in values {
        primitives::short_text(
            &value.authority_domain,
            &format!("{label}.authority_domain"),
        )?;
        primitives::short_text(
            &value.risk_acceptance_id,
            &format!("{label}.risk_acceptance_id"),
        )?;
        primitives::sha256(
            &value.risk_acceptance_sha256,
            &format!("{label}.risk_acceptance_sha256"),
        )?;
    }
    primitives::strictly_sorted(values, label)
}

fn validate_scope(value: &ApprovalScope, label: &str) -> Result<(), ApprovalRecordContractError> {
    for (field, text) in [
        ("change_id", &value.change_id),
        ("environment_id", &value.environment_id),
        ("project_id", &value.project_id),
    ] {
        primitives::short_text(text, &format!("{label}.{field}"))?;
    }
    match (value.scope_type, value.effect_id, value.gate_id.as_deref()) {
        (ApprovalScopeType::Effect, Some(_), None) => Ok(()),
        (ApprovalScopeType::Gate, None, Some(gate)) => {
            primitives::short_text(gate, &format!("{label}.gate_id"))
        }
        _ => Err(invalid(format!(
            "{label} effect_id/gate_id contradict scope_type"
        ))),
    }
}

fn validate_decision_basis(
    value: &ApprovalDecisionBasis,
) -> Result<(), ApprovalRecordContractError> {
    primitives::text(&value.rationale_ref, 4_096, "decision_basis.rationale_ref")?;
    primitives::sha256(&value.rationale_sha256, "decision_basis.rationale_sha256")?;
    if value.reason_codes.is_empty() || value.reason_codes.len() > 16 {
        return Err(invalid(
            "decision_basis.reason_codes must contain 1..16 items",
        ));
    }
    for reason in &value.reason_codes {
        primitives::stable_identifier(reason, "decision_basis.reason_codes")?;
    }
    primitives::strictly_sorted_strings(&value.reason_codes, "decision_basis.reason_codes")
}

fn validate_sod_declaration(
    value: &ApprovalSeparationOfDutyDeclaration,
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    if value.implementers.len() > 32 || value.required_distinctions.is_empty() {
        return Err(invalid(format!("{label} has invalid cardinality")));
    }
    primitives::principal(&value.requester, &format!("{label}.requester"))?;
    for principal in &value.implementers {
        primitives::principal(principal, &format!("{label}.implementers"))?;
    }
    primitives::strictly_sorted(&value.implementers, &format!("{label}.implementers"))?;
    validate_distinction_order(&value.required_distinctions, label)?;
    primitives::short_text(
        &value.proof_profile_id,
        &format!("{label}.proof_profile_id"),
    )?;
    primitives::sha256(
        &value.proof_profile_sha256,
        &format!("{label}.proof_profile_sha256"),
    )
}

fn validate_sod(
    value: &ApprovalSeparationOfDuty,
    record: &ApprovalRecord,
) -> Result<(), ApprovalRecordContractError> {
    let declaration = ApprovalSeparationOfDutyDeclaration {
        implementers: value.implementers.clone(),
        proof_profile_id: value.proof_profile_id.clone(),
        proof_profile_sha256: value.proof_profile_sha256.clone(),
        requester: value.requester.clone(),
        required_distinctions: value.required_distinctions.clone(),
    };
    validate_sod_declaration(&declaration, "separation_of_duty")?;
    primitives::base64url(&value.proof_base64url, "separation_of_duty.proof_base64url")?;
    validate_declared_distinctions(record)
}

fn validate_distinction_order(
    values: &[ApprovalRequiredDistinction],
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    if values.len() > 3
        || !values
            .windows(2)
            .all(|pair| pair[0].as_str().as_bytes() < pair[1].as_str().as_bytes())
    {
        Err(invalid(format!(
            "{label}.required_distinctions must be strictly sorted and unique"
        )))
    } else {
        Ok(())
    }
}

fn validate_declared_distinctions(
    record: &ApprovalRecord,
) -> Result<(), ApprovalRecordContractError> {
    let sod = &record.separation_of_duty;
    let declaration = ApprovalSeparationOfDutyDeclaration {
        implementers: sod.implementers.clone(),
        proof_profile_id: sod.proof_profile_id.clone(),
        proof_profile_sha256: sod.proof_profile_sha256.clone(),
        requester: sod.requester.clone(),
        required_distinctions: sod.required_distinctions.clone(),
    };
    validate_consistency(
        &record.approver,
        &record.subject,
        &record.authority_proof.authority_source,
        &record.risk_acceptance_refs,
        &record.scope,
        &declaration,
        record.decision,
    )
}

fn validate_consistency(
    approver: &crate::capability_grant_contract::Principal,
    subject: &crate::capability_grant_contract::Principal,
    authority_source: &ApprovalAuthoritySource,
    risks: &[RiskAcceptanceRef],
    scope: &ApprovalScope,
    sod: &ApprovalSeparationOfDutyDeclaration,
    decision: ApprovalDecision,
) -> Result<(), ApprovalRecordContractError> {
    for distinction in &sod.required_distinctions {
        let contradicted = match distinction {
            ApprovalRequiredDistinction::ApproverNotImplementer => {
                sod.implementers.contains(approver)
            }
            ApprovalRequiredDistinction::ApproverNotRequester => sod.requester == *approver,
            ApprovalRequiredDistinction::ApproverNotSubject => *subject == *approver,
        };
        if contradicted {
            return Err(invalid("declared separation-of-duty identities contradict"));
        }
    }
    validate_materiality_sod(scope.materiality_level, risks, sod)?;
    validate_production_declaration(scope, decision, authority_source.authority_class)
}

fn validate_materiality_sod(
    materiality: MaterialityLevel,
    risks: &[RiskAcceptanceRef],
    sod: &ApprovalSeparationOfDutyDeclaration,
) -> Result<(), ApprovalRecordContractError> {
    if matches!(materiality, MaterialityLevel::L3 | MaterialityLevel::L4)
        && (sod.implementers.is_empty()
            || sod.required_distinctions
                != [
                    ApprovalRequiredDistinction::ApproverNotImplementer,
                    ApprovalRequiredDistinction::ApproverNotRequester,
                    ApprovalRequiredDistinction::ApproverNotSubject,
                ])
    {
        return Err(invalid(
            "L3/L4 requires implementers and all SoD distinctions",
        ));
    }
    if !risks.is_empty()
        && !sod
            .required_distinctions
            .contains(&ApprovalRequiredDistinction::ApproverNotRequester)
    {
        return Err(invalid(
            "RiskAcceptance refs require approver_not_requester",
        ));
    }
    Ok(())
}

fn validate_validity(value: &ApprovalValidity) -> Result<(), ApprovalRecordContractError> {
    for (label, instant) in [
        ("issued_at_unix_ms", value.issued_at_unix_ms),
        ("not_before_unix_ms", value.not_before_unix_ms),
        ("expires_at_unix_ms", value.expires_at_unix_ms),
    ] {
        primitives::nonnegative(instant, &format!("validity.{label}"))?;
    }
    if value.issued_at_unix_ms > value.not_before_unix_ms
        || value.not_before_unix_ms >= value.expires_at_unix_ms
        || value.expires_at_unix_ms - value.issued_at_unix_ms > MAX_VALIDITY_MS
        || value.transferable
    {
        return Err(invalid("ApprovalRecord validity window is invalid"));
    }
    if value.revoked_at_unix_ms.is_some_and(|revoked| {
        revoked < value.issued_at_unix_ms || revoked >= value.expires_at_unix_ms
    }) {
        return Err(invalid("declared revocation time is outside validity"));
    }
    Ok(())
}

fn validate_production_declaration(
    scope: &ApprovalScope,
    decision: ApprovalDecision,
    authority_class: AuthorityClass,
) -> Result<(), ApprovalRecordContractError> {
    let restricted_effect = matches!(
        scope.effect_id,
        Some(EffectId::MigrationApply | EffectId::ReleaseExecute)
    );
    let restricted = scope.scope_type == ApprovalScopeType::Effect
        && restricted_effect
        && scope.environment_class == EnvironmentClass::Production
        && decision == ApprovalDecision::Approve;
    if restricted && authority_class != AuthorityClass::ExternalOperator {
        Err(invalid(
            "production apply/execute approval requires external_operator",
        ))
    } else {
        Ok(())
    }
}
