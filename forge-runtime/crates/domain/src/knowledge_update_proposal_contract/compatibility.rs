use crate::{
    capability_grant_contract::{
        CapabilityGrant, EffectId, GovernanceObjectKind, ScopeResource, canonical_grant_json,
    },
    context_package_contract::{
        ContextPackage, ContextPackageBuildRequest, TokenCounter, validate_package,
    },
};

use super::{
    CapabilityGrantRef, ContextDigestRelation, ContextFreshnessRelation,
    ContextKnowledgeUpdateCompatibility, ContextKnowledgeUpdateReason,
    ContextKnowledgeUpdateRelations, ContextPolicyRelation, ContextSourceRelation,
    ContextTaskRelation, GrantBindingsRelation, GrantDeclaredTimeRelation, GrantEffectRelation,
    GrantKnowledgeUpdateCompatibility, GrantKnowledgeUpdateReason, GrantKnowledgeUpdateRelations,
    GrantProposerRelation, GrantReferenceRelation, GrantScopeRelation, GrantTaskRelation,
    KnowledgeUpdateProposal, KnowledgeUpdateProposalContractError, invalid, validation,
};

pub const GRANT_COMPATIBILITY_RESULT: &str = "ASSESSED_GRANT_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no issuer, policy, Approval, revocation, usage, authorization, permission, persistence, apply, receipt or effect attestation)";
pub const CONTEXT_COMPATIBILITY_RESULT: &str = "ASSESSED_CONTEXT_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no source authentication, freshness, truth, instruction, permission, adoption, persistence, apply or effect attestation)";

/// Projects the exact declared ADR-0056 Grant reference without authenticating it.
///
/// # Errors
///
/// Returns an error when the caller-provided Grant is invalid.
pub fn project_capability_grant_ref(
    grant: &CapabilityGrant,
) -> Result<CapabilityGrantRef, KnowledgeUpdateProposalContractError> {
    canonical_grant_json(grant)
        .map_err(|error| invalid(format!("CapabilityGrant: {}", error.message)))?;
    Ok(CapabilityGrantRef {
        authority_domain: grant.authority_proof.issuer.authority_domain.clone(),
        grant_id: grant.grant_id.clone(),
        grant_sha256: grant.grant_sha256.clone(),
    })
}

/// Compares a strict Grant with a proposal without producing permission or apply authority.
///
/// # Errors
///
/// Returns an error when either caller-provided declaration is invalid.
pub fn assess_declared_grant_compatibility(
    grant: &CapabilityGrant,
    proposal: &KnowledgeUpdateProposal,
) -> Result<GrantKnowledgeUpdateCompatibility, KnowledgeUpdateProposalContractError> {
    canonical_grant_json(grant)
        .map_err(|error| invalid(format!("CapabilityGrant: {}", error.message)))?;
    validation::validate_proposal(proposal)?;
    let relation = grant_relations(grant, proposal)?;
    let mut reasons = grant_reason_codes(&relation);
    reasons.sort();
    reasons.dedup();
    Ok(GrantKnowledgeUpdateCompatibility {
        reason_codes: reasons,
        relations: relation,
        result: GRANT_COMPATIBILITY_RESULT.into(),
    })
}

fn grant_reason_codes(
    relations: &GrantKnowledgeUpdateRelations,
) -> Vec<GrantKnowledgeUpdateReason> {
    let mut reasons = Vec::new();
    if relations.bindings == GrantBindingsRelation::BindingsMismatch {
        reasons.push(GrantKnowledgeUpdateReason::BindingsMismatch);
    }
    if relations.declared_time == GrantDeclaredTimeRelation::DeclaredTimeMismatch {
        reasons.push(GrantKnowledgeUpdateReason::DeclaredTimeMismatch);
    }
    if relations.effect == GrantEffectRelation::EffectMismatch {
        reasons.push(GrantKnowledgeUpdateReason::EffectMismatch);
    }
    if relations.grant_ref == GrantReferenceRelation::GrantRefMismatch {
        reasons.push(GrantKnowledgeUpdateReason::GrantRefMismatch);
    }
    if relations.proposer == GrantProposerRelation::ProposerMismatch {
        reasons.push(GrantKnowledgeUpdateReason::ProposerMismatch);
    }
    if relations.task_binding == GrantTaskRelation::TaskBindingMismatch {
        reasons.push(GrantKnowledgeUpdateReason::TaskBindingMismatch);
    }
    if relations.effect == GrantEffectRelation::SameDeclaredEffect {
        grant_scope_reason(relations.scope, &mut reasons);
    }
    reasons
}

fn grant_scope_reason(relation: GrantScopeRelation, reasons: &mut Vec<GrantKnowledgeUpdateReason>) {
    match relation {
        GrantScopeRelation::DeniedByDeclaration => {
            reasons.push(GrantKnowledgeUpdateReason::DenyMatched);
        }
        GrantScopeRelation::OutsideDeclaredScope => {
            reasons.push(GrantKnowledgeUpdateReason::ScopeNotCovered);
        }
        GrantScopeRelation::CoveredByDeclaration => {}
    }
}

fn grant_relations(
    grant: &CapabilityGrant,
    proposal: &KnowledgeUpdateProposal,
) -> Result<GrantKnowledgeUpdateRelations, KnowledgeUpdateProposalContractError> {
    let effect_matches = grant.scope.effect_id == EffectId::KnowledgePropose;
    let scope = grant_scope_relation(grant, proposal, effect_matches);
    Ok(GrantKnowledgeUpdateRelations {
        bindings: relation(
            grant_bindings_match(grant, proposal),
            GrantBindingsRelation::SameDeclaredBindings,
            GrantBindingsRelation::BindingsMismatch,
        ),
        declared_time: relation(
            grant.validity.not_before_unix_ms <= proposal.submitted_at_unix_ms
                && proposal.submitted_at_unix_ms < grant.validity.expires_at_unix_ms,
            GrantDeclaredTimeRelation::SameDeclaredTime,
            GrantDeclaredTimeRelation::DeclaredTimeMismatch,
        ),
        effect: relation(
            effect_matches,
            GrantEffectRelation::SameDeclaredEffect,
            GrantEffectRelation::EffectMismatch,
        ),
        grant_ref: relation(
            proposal.capability_grant_ref == project_capability_grant_ref(grant)?,
            GrantReferenceRelation::SameDeclaredGrantRef,
            GrantReferenceRelation::GrantRefMismatch,
        ),
        proposer: relation(
            grant.subject == proposal.proposer,
            GrantProposerRelation::SameDeclaredProposer,
            GrantProposerRelation::ProposerMismatch,
        ),
        scope,
        task_binding: relation(
            grant.task_binding == proposal.task_binding,
            GrantTaskRelation::SameDeclaredTaskBinding,
            GrantTaskRelation::TaskBindingMismatch,
        ),
    })
}

fn grant_bindings_match(grant: &CapabilityGrant, proposal: &KnowledgeUpdateProposal) -> bool {
    let left = &grant.bindings;
    let right = &proposal.bindings;
    left.context_sha256 == right.context_sha256
        && left.impact_sha256 == right.impact_sha256
        && left.plan_sha256 == right.plan_sha256
        && left.policy_sha256 == right.policy_sha256
        && left.risk_sha256 == right.risk_sha256
        && left.source_revision == right.source_revision
        && left.source_tree_sha256 == right.source_tree_sha256
}

fn grant_scope_relation(
    grant: &CapabilityGrant,
    proposal: &KnowledgeUpdateProposal,
    effect_matches: bool,
) -> GrantScopeRelation {
    if !effect_matches {
        return GrantScopeRelation::OutsideDeclaredScope;
    }
    let requested = ScopeResource::GovernanceObject {
        object_kind: GovernanceObjectKind::Knowledge,
        object_ref: proposal.knowledge_scope.object_ref.clone(),
        object_scope_sha256: proposal.knowledge_scope.object_scope_sha256.clone(),
    };
    if grant.scope.deny.contains(&requested) {
        GrantScopeRelation::DeniedByDeclaration
    } else if grant
        .scope
        .allow
        .iter()
        .any(|clause| clause.resources.contains(&requested))
    {
        GrantScopeRelation::CoveredByDeclaration
    } else {
        GrantScopeRelation::OutsideDeclaredScope
    }
}

/// Compares a fully reassembled `ContextPackage` with a proposal's declared bindings.
///
/// # Errors
///
/// Returns an error when Context reassembly fails or the proposal is invalid.
pub fn assess_declared_context_compatibility(
    request: &ContextPackageBuildRequest,
    package: &ContextPackage,
    counter: &impl TokenCounter,
    proposal: &KnowledgeUpdateProposal,
) -> Result<ContextKnowledgeUpdateCompatibility, KnowledgeUpdateProposalContractError> {
    validate_package(request, package, counter)
        .map_err(|error| invalid(format!("ContextPackage: {}", error.message)))?;
    validation::validate_proposal(proposal)?;
    let relations = context_relations(package, proposal);
    let mut reasons = context_reason_codes(&relations);
    reasons.sort();
    reasons.dedup();
    Ok(ContextKnowledgeUpdateCompatibility {
        reason_codes: reasons,
        relations,
        result: CONTEXT_COMPATIBILITY_RESULT.into(),
    })
}

fn context_reason_codes(
    relations: &ContextKnowledgeUpdateRelations,
) -> Vec<ContextKnowledgeUpdateReason> {
    let mut reasons = Vec::new();
    if relations.context == ContextDigestRelation::ContextMismatch {
        reasons.push(ContextKnowledgeUpdateReason::ContextMismatch);
    }
    if relations.freshness == ContextFreshnessRelation::OutsideDeclaredFreshness {
        reasons.push(ContextKnowledgeUpdateReason::FreshnessMismatch);
    }
    if relations.policy == ContextPolicyRelation::PolicyMismatch {
        reasons.push(ContextKnowledgeUpdateReason::PolicyMismatch);
    }
    if relations.source == ContextSourceRelation::SourceMismatch {
        reasons.push(ContextKnowledgeUpdateReason::SourceMismatch);
    }
    if relations.task_binding == ContextTaskRelation::TaskBindingMismatch {
        reasons.push(ContextKnowledgeUpdateReason::TaskBindingMismatch);
    }
    reasons
}

fn context_relations(
    package: &ContextPackage,
    proposal: &KnowledgeUpdateProposal,
) -> ContextKnowledgeUpdateRelations {
    let source = &package.source_binding;
    let task = &package.task_binding;
    ContextKnowledgeUpdateRelations {
        context: relation(
            package.context_sha256 == proposal.bindings.context_sha256,
            ContextDigestRelation::SameDeclaredContext,
            ContextDigestRelation::ContextMismatch,
        ),
        freshness: relation(
            package.freshness.evaluated_at_unix_ms <= proposal.submitted_at_unix_ms
                && package
                    .freshness
                    .expires_at_unix_ms
                    .is_none_or(|expiry| proposal.submitted_at_unix_ms < expiry),
            ContextFreshnessRelation::InsideDeclaredFreshness,
            ContextFreshnessRelation::OutsideDeclaredFreshness,
        ),
        policy: relation(
            source.policy_sha256 == proposal.bindings.policy_sha256,
            ContextPolicyRelation::SameDeclaredPolicy,
            ContextPolicyRelation::PolicyMismatch,
        ),
        source: relation(
            source.source_revision == proposal.bindings.source_revision
                && source.source_tree_sha256 == proposal.bindings.source_tree_sha256,
            ContextSourceRelation::SameDeclaredSource,
            ContextSourceRelation::SourceMismatch,
        ),
        task_binding: relation(
            context_task_matches(task, proposal),
            ContextTaskRelation::SameDeclaredTaskBinding,
            ContextTaskRelation::TaskBindingMismatch,
        ),
    }
}

fn context_task_matches(
    task: &crate::context_package_contract::TaskBinding,
    proposal: &KnowledgeUpdateProposal,
) -> bool {
    let other = &proposal.task_binding;
    task.change_id == other.change_id
        && task.node_id == other.node_id
        && task.project_id == other.project_id
        && task.role == other.role
        && task.run_id == other.run_id
        && task.task_id == other.task_id
}

/// Projects proposal artifacts into ADR-0056 artifact resources without reading them.
///
/// # Errors
///
/// Returns an error when the proposal is invalid.
pub fn project_artifact_resources(
    proposal: &KnowledgeUpdateProposal,
) -> Result<Vec<ScopeResource>, KnowledgeUpdateProposalContractError> {
    validation::validate_proposal(proposal)?;
    Ok(proposal
        .bindings
        .artifacts
        .iter()
        .map(|artifact| ScopeResource::Artifact {
            artifact_kind: artifact.artifact_kind.clone(),
            artifact_ref: artifact.artifact_ref.clone(),
            artifact_sha256: artifact.artifact_sha256.clone(),
        })
        .collect())
}

fn relation<T: Copy>(matches: bool, positive: T, negative: T) -> T {
    if matches { positive } else { negative }
}
