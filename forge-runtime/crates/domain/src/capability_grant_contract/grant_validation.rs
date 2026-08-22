use super::{
    ApprovalRef, AuthorityClass, AuthorityProof, CANONICALIZATION, CAPABILITY_GRANT_API_VERSION,
    CAPABILITY_GRANT_KIND, CapabilityGrant, CapabilityGrantContractError, CapabilityIdentity,
    ConsumptionMode, EffectId, GrantBindings, GrantBudget, GrantTaskBinding, GrantValidity,
    IssuancePhase, Principal, PrincipalType, RequestedUsage, RequiredDistinction, codec, invalid,
    primitives, scope_validation,
};

const VOCABULARY_SHA256: &str = "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f";

pub(super) fn validate(grant: &CapabilityGrant) -> Result<(), CapabilityGrantContractError> {
    if grant.api_version != CAPABILITY_GRANT_API_VERSION
        || grant.canonicalization != CANONICALIZATION
        || grant.kind != CAPABILITY_GRANT_KIND
        || grant.effect_vocabulary_sha256 != VOCABULARY_SHA256
    {
        return Err(invalid("CapabilityGrant envelope does not match v1"));
    }
    validate_approvals(&grant.approval_refs)?;
    validate_authority_proof(&grant.authority_proof)?;
    validate_bindings(&grant.bindings)?;
    validate_phase_bindings(grant)?;
    validate_budget(&grant.budget)?;
    validate_capability(&grant.capability)?;
    validate_principal(&grant.subject, "grant subject")?;
    validate_task_binding(&grant.task_binding)?;
    validate_separation_of_duty(grant)?;
    validate_usage_policy(grant)?;
    validate_validity(&grant.validity)?;
    scope_validation::validate_scope(&grant.scope)?;
    validate_effect_constraints(grant)?;
    validate_identity(grant)
}

fn validate_phase_bindings(grant: &CapabilityGrant) -> Result<(), CapabilityGrantContractError> {
    if grant.issuance_phase == IssuancePhase::PlanFinalization
        && (grant.bindings.impact_sha256.is_none()
            || grant.bindings.plan_sha256.is_none()
            || grant.bindings.risk_sha256.is_none())
    {
        Err(invalid(
            "plan_finalization requires impact, plan, and risk bindings",
        ))
    } else {
        Ok(())
    }
}

fn validate_approvals(values: &[ApprovalRef]) -> Result<(), CapabilityGrantContractError> {
    if values.len() > 32 {
        return Err(invalid("approval_refs exceed 32 entries"));
    }
    let mut encoded = Vec::with_capacity(values.len());
    for value in values {
        primitives::text(&value.approval_id, 160, "approval_id")?;
        primitives::sha256(&value.approval_sha256, "approval_sha256")?;
        primitives::text(&value.authority_domain, 160, "approval authority_domain")?;
        encoded.push(super::canonical::encode(
            value,
            super::MAX_GRANT_BYTES,
            "approval_ref",
        )?);
    }
    if encoded
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
    {
        Ok(())
    } else {
        Err(invalid("approval_refs must be strictly ordered and unique"))
    }
}

fn validate_authority_proof(value: &AuthorityProof) -> Result<(), CapabilityGrantContractError> {
    primitives::text(
        &value.issuer.authority_domain,
        160,
        "issuer authority_domain",
    )?;
    primitives::text(&value.issuer.principal_id, 160, "issuer principal_id")?;
    if value.issuer.principal_type == PrincipalType::Agent {
        return Err(invalid("CapabilityGrant issuer cannot be an agent"));
    }
    if (value.issuer.authority_class == AuthorityClass::ForgeosKernel
        && value.issuer.principal_type != PrincipalType::Service)
        || (value.issuer.authority_class == AuthorityClass::ExternalOperator
            && !matches!(
                value.issuer.principal_type,
                PrincipalType::Human | PrincipalType::Operator
            ))
    {
        return Err(invalid(
            "issuer principal type does not match authority_class",
        ));
    }
    primitives::text(&value.key_id, 160, "authority key_id")?;
    primitives::base64url(&value.proof_base64url)?;
    primitives::text(&value.proof_profile_id, 160, "proof_profile_id")?;
    primitives::sha256(&value.proof_profile_sha256, "proof_profile_sha256")?;
    primitives::text(&value.trust_domain, 160, "trust_domain")?;
    primitives::integer(value.trust_epoch, 0, i64::MAX, "trust_epoch")
}

pub(super) fn validate_capability(
    value: &CapabilityIdentity,
) -> Result<(), CapabilityGrantContractError> {
    primitives::sha256(
        &value.capability_contract_sha256,
        "capability_contract_sha256",
    )?;
    primitives::text(&value.capability_id, 160, "capability_id")?;
    primitives::text(&value.capability_version, 160, "capability_version")
}

pub(super) fn validate_bindings(value: &GrantBindings) -> Result<(), CapabilityGrantContractError> {
    for (label, digest) in [
        ("context_sha256", value.context_sha256.as_str()),
        ("grant_request_sha256", value.grant_request_sha256.as_str()),
        ("policy_sha256", value.policy_sha256.as_str()),
        ("source_tree_sha256", value.source_tree_sha256.as_str()),
    ] {
        primitives::sha256(digest, label)?;
    }
    primitives::optional_sha256(value.impact_sha256.as_deref(), "impact_sha256")?;
    primitives::optional_sha256(value.plan_sha256.as_deref(), "plan_sha256")?;
    primitives::optional_sha256(value.risk_sha256.as_deref(), "risk_sha256")?;
    primitives::text(&value.source_revision, 160, "source_revision")
}

fn validate_budget(value: &GrantBudget) -> Result<(), CapabilityGrantContractError> {
    primitives::integer(value.max_calls, 1, 1_000_000_000, "max_calls")?;
    primitives::integer(
        value.max_cost_usd_micros,
        0,
        1_000_000_000_000_000,
        "max_cost",
    )?;
    primitives::integer(value.max_input_tokens, 0, 1_000_000_000, "max_input_tokens")?;
    primitives::integer(
        value.max_network_bytes,
        0,
        1_073_741_824,
        "max_network_bytes",
    )?;
    primitives::integer(value.max_output_bytes, 0, 1_073_741_824, "max_output_bytes")?;
    primitives::integer(
        value.max_output_tokens,
        0,
        1_000_000_000,
        "max_output_tokens",
    )?;
    primitives::integer(value.timeout_ms, 1, 86_400_000, "timeout_ms")
}

pub(super) fn validate_requested_usage(
    value: &RequestedUsage,
) -> Result<(), CapabilityGrantContractError> {
    primitives::integer(value.call_count, 1, 1_000_000_000, "call_count")?;
    primitives::integer(value.cost_usd_micros, 0, 1_000_000_000_000_000, "cost")?;
    primitives::integer(value.input_tokens, 0, 1_000_000_000, "input_tokens")?;
    primitives::integer(value.network_bytes, 0, 1_073_741_824, "network_bytes")?;
    primitives::integer(value.output_bytes, 0, 1_073_741_824, "output_bytes")?;
    primitives::integer(value.output_tokens, 0, 1_000_000_000, "output_tokens")?;
    primitives::integer(value.timeout_ms, 1, 86_400_000, "requested timeout_ms")
}

pub(super) fn validate_principal(
    value: &Principal,
    label: &str,
) -> Result<(), CapabilityGrantContractError> {
    primitives::text(
        &value.authority_domain,
        160,
        &format!("{label} authority_domain"),
    )?;
    primitives::text(&value.principal_id, 160, &format!("{label} principal_id"))
}

pub(super) fn validate_task_binding(
    value: &GrantTaskBinding,
) -> Result<(), CapabilityGrantContractError> {
    primitives::optional_text(value.attempt_id.as_deref(), 160, "attempt_id")?;
    primitives::optional_text(value.target_id.as_deref(), 160, "target_id")?;
    for (label, text) in [
        ("change_id", value.change_id.as_str()),
        ("environment_id", value.environment_id.as_str()),
        ("node_id", value.node_id.as_str()),
        ("project_id", value.project_id.as_str()),
        ("role", value.role.as_str()),
        ("run_id", value.run_id.as_str()),
        ("task_id", value.task_id.as_str()),
    ] {
        primitives::text(text, 160, label)?;
    }
    Ok(())
}

fn validate_separation_of_duty(
    grant: &CapabilityGrant,
) -> Result<(), CapabilityGrantContractError> {
    validate_principal(&grant.separation_of_duty.requester, "requester")?;
    let distinctions = &grant.separation_of_duty.required_distinctions;
    if distinctions.is_empty()
        || distinctions.len() > 5
        || !distinctions
            .windows(2)
            .all(|pair| pair[0].as_str().as_bytes() < pair[1].as_str().as_bytes())
    {
        return Err(invalid("separation_of_duty distinctions are invalid"));
    }
    validate_declared_distinctions(grant)
}

fn validate_declared_distinctions(
    grant: &CapabilityGrant,
) -> Result<(), CapabilityGrantContractError> {
    let distinctions = &grant.separation_of_duty.required_distinctions;
    let issuer = issuer_key(&grant.authority_proof.issuer);
    let requester = principal_key(&grant.separation_of_duty.requester);
    let subject = principal_key(&grant.subject);
    if distinctions.contains(&RequiredDistinction::IssuerNotRequester) && issuer == requester {
        return Err(invalid("issuer and requester violate separation_of_duty"));
    }
    if distinctions.contains(&RequiredDistinction::IssuerNotSubject) && issuer == subject {
        return Err(invalid("issuer and subject violate separation_of_duty"));
    }
    Ok(())
}

fn principal_key(value: &Principal) -> (&str, &str, PrincipalType) {
    (
        &value.authority_domain,
        &value.principal_id,
        value.principal_type,
    )
}

fn issuer_key(value: &super::Issuer) -> (&str, &str, PrincipalType) {
    (
        &value.authority_domain,
        &value.principal_id,
        value.principal_type,
    )
}

fn validate_usage_policy(grant: &CapabilityGrant) -> Result<(), CapabilityGrantContractError> {
    if !grant.usage_policy.atomic_reservation_required
        || !grant.usage_policy.usage_ledger_required
        || (grant.usage_policy.consumption_mode == ConsumptionMode::SingleUse
            && grant.budget.max_calls != 1)
    {
        return Err(invalid(
            "usage_policy does not preserve reservation semantics",
        ));
    }
    Ok(())
}

fn validate_validity(value: &GrantValidity) -> Result<(), CapabilityGrantContractError> {
    if value.transferable
        || value.issued_at_unix_ms < 0
        || value.not_before_unix_ms < value.issued_at_unix_ms
        || value.expires_at_unix_ms <= value.not_before_unix_ms
        || value.expires_at_unix_ms - value.issued_at_unix_ms > 86_400_000
    {
        return Err(invalid("grant validity window is invalid"));
    }
    Ok(())
}

fn validate_effect_constraints(
    grant: &CapabilityGrant,
) -> Result<(), CapabilityGrantContractError> {
    let effect = grant.scope.effect_id;
    if !matches!(effect, EffectId::MigrationApply | EffectId::ReleaseExecute)
        || !scope_contains_production(&grant.scope)
    {
        return Ok(());
    }
    if grant.authority_proof.issuer.authority_class != AuthorityClass::ExternalOperator
        || grant.approval_refs.is_empty()
    {
        return Err(invalid(
            "production apply/execute requires external_operator and approval_refs",
        ));
    }
    Ok(())
}

fn scope_contains_production(scope: &super::GrantScope) -> bool {
    scope.allow.iter().any(|clause| {
        clause.resources.iter().any(|resource| {
            matches!(
                resource,
                super::ScopeResource::Environment {
                    environment_class: super::EnvironmentClass::Production,
                    ..
                }
            )
        })
    })
}

fn validate_identity(grant: &CapabilityGrant) -> Result<(), CapabilityGrantContractError> {
    primitives::sha256(&grant.grant_sha256, "grant_sha256")?;
    let expected = codec::grant_sha256(grant)?;
    if grant.grant_sha256 != expected || grant.grant_id != format!("capability-grant-{expected}") {
        return Err(invalid("CapabilityGrant self identity does not match"));
    }
    Ok(())
}
