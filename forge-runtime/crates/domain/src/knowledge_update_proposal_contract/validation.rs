use std::collections::BTreeSet;

use super::{
    ASSESSMENT_API_VERSION, ASSESSMENT_MODE, ASSESSMENT_REQUEST_API_VERSION, ASSESSMENT_RESULT,
    CANONICALIZATION, CapabilityGrantRef, ClaimRef, KnowledgeArtifact, KnowledgeMutation,
    KnowledgeUpdateAssessmentRequest, KnowledgeUpdateBindings, KnowledgeUpdateDeclaredAssessment,
    KnowledgeUpdateDeclaredTarget, KnowledgeUpdateProposal, KnowledgeUpdateProposalContractError,
    MAX_ARTIFACTS, MAX_ASSESSMENT_BYTES, MAX_ASSESSMENT_REQUEST_BYTES, MAX_DECLARED_TARGET_BYTES,
    MAX_MUTATION_REASON_CODES, MAX_MUTATIONS, MAX_PROPOSAL_BYTES, MutationOperation,
    PROPOSAL_API_VERSION, PROPOSAL_KIND, canonical, closure, codec, invalid, primitives,
};

/// Projects the exact seven-field, records-free declared assessment target.
///
/// # Errors
///
/// Returns an error when the source proposal is invalid.
pub fn declared_target(
    proposal: &KnowledgeUpdateProposal,
) -> Result<KnowledgeUpdateDeclaredTarget, KnowledgeUpdateProposalContractError> {
    validate_proposal(proposal)?;
    Ok(declared_target_unchecked(proposal))
}

pub(super) fn declared_target_unchecked(
    proposal: &KnowledgeUpdateProposal,
) -> KnowledgeUpdateDeclaredTarget {
    KnowledgeUpdateDeclaredTarget {
        bindings: proposal.bindings.clone(),
        capability_grant_ref: proposal.capability_grant_ref.clone(),
        knowledge_scope: proposal.knowledge_scope.clone(),
        mutations: proposal.mutations.clone(),
        proposer: proposal.proposer.clone(),
        record_set_sha256: proposal.record_set_sha256.clone(),
        task_binding: proposal.task_binding.clone(),
    }
}

/// Validates the exact proposal envelope, identities, closure, and declared lifecycle.
///
/// # Errors
///
/// Returns an error when any bounded wire or semantic invariant is violated.
pub fn validate_proposal(
    proposal: &KnowledgeUpdateProposal,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    validate_proposal_body(proposal)?;
    validate_identity(proposal)
}

pub(super) fn validate_unsealed_proposal(
    proposal: &KnowledgeUpdateProposal,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    if !proposal.proposal_id.is_empty() || !proposal.proposal_sha256.is_empty() {
        return Err(invalid(
            "unsealed proposal_id and proposal_sha256 must both be empty",
        ));
    }
    validate_proposal_body(proposal)
}

fn validate_proposal_body(
    proposal: &KnowledgeUpdateProposal,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    if proposal.api_version != PROPOSAL_API_VERSION
        || proposal.canonicalization != CANONICALIZATION
        || proposal.kind != PROPOSAL_KIND
        || proposal.submitted_at_unix_ms < 0
    {
        return Err(invalid(
            "KnowledgeUpdateProposal envelope does not match v1",
        ));
    }
    canonical::encode(proposal, MAX_PROPOSAL_BYTES, "knowledge update proposal")?;
    validate_bindings(&proposal.bindings)?;
    validate_grant_ref(&proposal.capability_grant_ref)?;
    validate_scope(proposal)?;
    primitives::principal(&proposal.proposer, "proposer")?;
    primitives::task_binding(&proposal.task_binding, "task_binding")?;
    validate_mutation_shapes(&proposal.mutations)?;
    validate_record_set(proposal)?;
    closure::validate(proposal)
}

fn validate_scope(
    proposal: &KnowledgeUpdateProposal,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    primitives::identifier(
        &proposal.knowledge_scope.object_ref,
        "knowledge_scope.object_ref",
    )?;
    primitives::sha256(
        &proposal.knowledge_scope.object_scope_sha256,
        "knowledge_scope.object_scope_sha256",
    )?;
    let common = proposal.records.iter().all(|record| {
        record.metadata().project_id == proposal.task_binding.project_id
            && record.metadata().scope == proposal.knowledge_scope.object_ref
    });
    common
        .then_some(())
        .ok_or_else(|| invalid("records do not share the declared project and knowledge scope"))
}

fn validate_record_set(
    proposal: &KnowledgeUpdateProposal,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    crate::governance_contract::validate_record_set(&proposal.records)
        .map_err(|error| invalid(format!("proposal records: {}", error.message)))?;
    primitives::sha256(&proposal.record_set_sha256, "record_set_sha256")?;
    if codec::record_set_sha256(proposal)? == proposal.record_set_sha256 {
        Ok(())
    } else {
        Err(invalid(
            "record_set_sha256 does not bind exact proposal records",
        ))
    }
}

fn validate_identity(
    proposal: &KnowledgeUpdateProposal,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    primitives::sha256(&proposal.proposal_sha256, "proposal_sha256")?;
    let expected = codec::proposal_sha256_unchecked(proposal)?;
    if proposal.proposal_sha256 != expected
        || proposal.proposal_id != format!("knowledge-update-proposal-{expected}")
    {
        Err(invalid(
            "proposal identity does not match its canonical content",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn validate_target(
    target: &KnowledgeUpdateDeclaredTarget,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    canonical::encode(
        target,
        MAX_DECLARED_TARGET_BYTES,
        "knowledge update declared target",
    )?;
    validate_bindings(&target.bindings)?;
    validate_grant_ref(&target.capability_grant_ref)?;
    primitives::identifier(
        &target.knowledge_scope.object_ref,
        "target knowledge object_ref",
    )?;
    primitives::sha256(
        &target.knowledge_scope.object_scope_sha256,
        "target object_scope_sha256",
    )?;
    validate_mutation_shapes(&target.mutations)?;
    primitives::principal(&target.proposer, "target proposer")?;
    primitives::sha256(&target.record_set_sha256, "target record_set_sha256")?;
    primitives::task_binding(&target.task_binding, "target task_binding")
}

pub(super) fn validate_assessment_request(
    request: &KnowledgeUpdateAssessmentRequest,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    if request.api_version != ASSESSMENT_REQUEST_API_VERSION
        || request.canonicalization != CANONICALIZATION
        || request.evaluated_at_unix_ms < 0
    {
        return Err(invalid(
            "knowledge update assessment request does not match v1",
        ));
    }
    canonical::encode(
        request,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "knowledge update assessment request",
    )?;
    validate_proposal(&request.knowledge_update_proposal)?;
    validate_target(&request.expected_target)?;
    primitives::sha256(&request.expected_target_sha256, "expected_target_sha256")?;
    primitives::sha256(&request.request_sha256, "request_sha256")?;
    if codec::declared_target_sha256(&request.expected_target)? != request.expected_target_sha256 {
        return Err(invalid("expected target digest does not match"));
    }
    if codec::assessment_request_sha256_unchecked(request)? != request.request_sha256 {
        return Err(invalid("assessment request self digest does not match"));
    }
    Ok(())
}

pub(super) fn validate_assessment_shape(
    assessment: &KnowledgeUpdateDeclaredAssessment,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    let neutral = assessment.api_version == ASSESSMENT_API_VERSION
        && assessment.assessment_mode == ASSESSMENT_MODE
        && assessment.canonicalization == CANONICALIZATION
        && assessment.result == ASSESSMENT_RESULT
        && !assessment.truth_attestation
        && !assessment.knowledge_adoption_attestation
        && !assessment.permission_attestation
        && !assessment.persistence_attestation
        && !assessment.execution_attestation
        && !assessment.effect_attestation;
    if !neutral {
        return Err(invalid(
            "knowledge update assessment authority-neutral envelope drifted",
        ));
    }
    canonical::encode(
        assessment,
        MAX_ASSESSMENT_BYTES,
        "knowledge update declared assessment",
    )?;
    for (label, digest) in [
        ("assessment_sha256", assessment.assessment_sha256.as_str()),
        (
            "expected_target_sha256",
            assessment.expected_target_sha256.as_str(),
        ),
        ("proposal_sha256", assessment.proposal_sha256.as_str()),
        ("request_sha256", assessment.request_sha256.as_str()),
    ] {
        primitives::sha256(digest, label)?;
    }
    if assessment.proposal_id != format!("knowledge-update-proposal-{}", assessment.proposal_sha256)
    {
        return Err(invalid("assessment proposal identity is inconsistent"));
    }
    validate_assessment_reasons(assessment)?;
    if codec::assessment_sha256(assessment)? != assessment.assessment_sha256 {
        return Err(invalid("declared assessment self digest does not match"));
    }
    Ok(())
}

fn validate_assessment_reasons(
    assessment: &KnowledgeUpdateDeclaredAssessment,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    let reasons = &assessment.reason_codes;
    if reasons.len() > 8
        || !reasons
            .windows(2)
            .all(|pair| pair[0].as_str().as_bytes() < pair[1].as_str().as_bytes())
        || *reasons != super::assessment::expected_reason_codes(&assessment.relations)
    {
        Err(invalid(
            "assessment reason_codes do not match declared relations",
        ))
    } else {
        Ok(())
    }
}

fn validate_bindings(
    bindings: &KnowledgeUpdateBindings,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    if bindings.artifacts.len() > MAX_ARTIFACTS {
        return Err(invalid("bindings artifacts exceed 32 entries"));
    }
    for artifact in &bindings.artifacts {
        validate_artifact(artifact)?;
    }
    primitives::sorted_nodes(&bindings.artifacts, "bindings artifacts")?;
    for (label, digest) in [
        ("context_sha256", bindings.context_sha256.as_str()),
        ("policy_sha256", bindings.policy_sha256.as_str()),
        ("source_tree_sha256", bindings.source_tree_sha256.as_str()),
    ] {
        primitives::sha256(digest, label)?;
    }
    primitives::optional_sha256(bindings.impact_sha256.as_deref(), "impact_sha256")?;
    primitives::optional_sha256(bindings.plan_sha256.as_deref(), "plan_sha256")?;
    primitives::optional_sha256(bindings.risk_sha256.as_deref(), "risk_sha256")?;
    primitives::short_text(&bindings.source_revision, "source_revision")
}

fn validate_artifact(
    artifact: &KnowledgeArtifact,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    primitives::short_text(&artifact.artifact_kind, "artifact_kind")?;
    primitives::reference_text(&artifact.artifact_ref, "artifact_ref")?;
    primitives::sha256(&artifact.artifact_sha256, "artifact_sha256")
}

fn validate_grant_ref(
    reference: &CapabilityGrantRef,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    primitives::short_text(&reference.authority_domain, "grant authority_domain")?;
    primitives::sha256(&reference.grant_sha256, "grant_sha256")?;
    if reference.grant_id == format!("capability-grant-{}", reference.grant_sha256) {
        Ok(())
    } else {
        Err(invalid("grant_id is not derived from grant_sha256"))
    }
}

fn validate_mutation_shapes(
    mutations: &[KnowledgeMutation],
) -> Result<(), KnowledgeUpdateProposalContractError> {
    if mutations.is_empty() || mutations.len() > MAX_MUTATIONS {
        return Err(invalid("mutations must contain 1..64 entries"));
    }
    let sorted = mutations.windows(2).all(|pair| {
        pair[0].target_aggregate_id.as_bytes() < pair[1].target_aggregate_id.as_bytes()
    });
    if !sorted {
        return Err(invalid(
            "mutations must be strictly target_aggregate_id sorted and unique",
        ));
    }
    let mut after_record_ids = BTreeSet::new();
    let mut before_record_ids = BTreeSet::new();
    for mutation in mutations {
        validate_mutation_shape(mutation)?;
        if !after_record_ids.insert(mutation.after_claim_ref.record_id.as_str()) {
            return Err(invalid("mutation after_claim_ref values must be unique"));
        }
        if let Some(reference) = &mutation.before_claim_ref
            && !before_record_ids.insert(reference.record_id.as_str())
        {
            return Err(invalid(
                "mutation before_claim_ref values must not declare a supersession fork",
            ));
        }
    }
    if !after_record_ids.is_disjoint(&before_record_ids) {
        return Err(invalid(
            "mutation after_claim_ref and before_claim_ref record sets must be disjoint",
        ));
    }
    Ok(())
}

fn validate_mutation_shape(
    mutation: &KnowledgeMutation,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    primitives::identifier(&mutation.target_aggregate_id, "target_aggregate_id")?;
    validate_claim_ref(&mutation.after_claim_ref, "after_claim_ref")?;
    if let Some(reference) = &mutation.before_claim_ref {
        validate_claim_ref(reference, "before_claim_ref")?;
    }
    let ref_shape = matches!(
        (&mutation.operation, &mutation.before_claim_ref),
        (MutationOperation::Create, None) | (MutationOperation::Supersede, Some(_))
    );
    if !ref_shape {
        return Err(invalid("mutation operation and before_claim_ref disagree"));
    }
    if mutation
        .before_claim_ref
        .as_ref()
        .is_some_and(|before| before.record_id == mutation.after_claim_ref.record_id)
    {
        return Err(invalid(
            "supersede before_claim_ref and after_claim_ref must name distinct records",
        ));
    }
    primitives::text(&mutation.rationale, 4_096, "mutation rationale")?;
    primitives::sorted_reasons(
        &mutation.reason_codes,
        MAX_MUTATION_REASON_CODES,
        "mutation reason_codes",
    )
}

fn validate_claim_ref(
    reference: &ClaimRef,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    primitives::identifier(&reference.record_id, &format!("{label}.record_id"))?;
    primitives::sha256(
        &reference.canonical_sha256,
        &format!("{label}.canonical_sha256"),
    )
}
