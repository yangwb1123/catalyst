use crate::{
    approval_record_contract::{ApprovalRecord, approval_ref},
    capability_grant_contract::{ApprovalRef, CapabilityGrant},
};

use super::{
    APPROVAL_COMPATIBILITY_RESULT, ApprovalCompatibilityReason, ApprovalRefSetRelation,
    ApprovalTransitionScopeRelation, CapabilityGrantRef, DeclaredApprovalTransitionCompatibility,
    DeclaredApprovalTransitionRelations, DeclaredGrantTransitionCompatibility,
    DeclaredGrantTransitionRelations, GRANT_COMPATIBILITY_RESULT, GrantActorRelation,
    GrantApprovalRefsRelation, GrantBindingsRelation, GrantCompatibilityReason,
    GrantDeclaredTimeRelation, GrantRefRelation, GrantTaskBindingRelation, TransitionReceipt,
    TransitionReceiptContractError, invalid, primitives,
};

/// Projects the exact three-field ADR-0056 Grant reference declaration.
///
/// # Errors
///
/// Returns an error when the Grant is malformed.
pub fn project_capability_grant_ref(
    grant: &CapabilityGrant,
) -> Result<CapabilityGrantRef, TransitionReceiptContractError> {
    crate::capability_grant_contract::canonical_grant_json(grant)
        .map_err(|error| invalid(error.message))?;
    Ok(CapabilityGrantRef {
        authority_domain: grant.authority_proof.issuer.authority_domain.clone(),
        grant_id: grant.grant_id.clone(),
        grant_sha256: grant.grant_sha256.clone(),
    })
}

/// Compares only caller-declared Grant/Transition fields.
///
/// # Errors
///
/// Returns an error when either input is malformed.
pub fn assess_declared_grant_compatibility(
    grant: &CapabilityGrant,
    receipt: &TransitionReceipt,
) -> Result<DeclaredGrantTransitionCompatibility, TransitionReceiptContractError> {
    let reference = project_capability_grant_ref(grant)?;
    super::validation::validate_receipt(receipt, false)?;
    let relations = DeclaredGrantTransitionRelations {
        actor: relation(
            receipt.actor == grant.subject,
            GrantActorRelation::SameDeclaredActor,
            GrantActorRelation::ActorMismatch,
        ),
        approval_refs: relation(
            receipt.approval_refs == grant.approval_refs,
            GrantApprovalRefsRelation::SameDeclaredApprovalRefs,
            GrantApprovalRefsRelation::ApprovalRefsMismatch,
        ),
        bindings: grant_bindings_relation(grant, receipt),
        declared_time: grant_time_relation(grant, receipt),
        grant_ref: relation(
            receipt.capability_grant_ref == reference,
            GrantRefRelation::SameDeclaredGrantRef,
            GrantRefRelation::GrantRefMismatch,
        ),
        task_binding: relation(
            receipt.task_binding == grant.task_binding,
            GrantTaskBindingRelation::SameDeclaredTaskBinding,
            GrantTaskBindingRelation::TaskBindingMismatch,
        ),
    };
    Ok(grant_compatibility(relations))
}

fn grant_bindings_relation(
    grant: &CapabilityGrant,
    receipt: &TransitionReceipt,
) -> GrantBindingsRelation {
    let left = &receipt.bindings;
    let right = &grant.bindings;
    let same = left.context_sha256 == right.context_sha256
        && left.impact_sha256 == right.impact_sha256
        && left.plan_sha256 == right.plan_sha256
        && left.policy_sha256 == right.policy_sha256
        && left.risk_sha256 == right.risk_sha256
        && left.source_revision == right.source_revision
        && left.source_tree_sha256 == right.source_tree_sha256;
    relation(
        same,
        GrantBindingsRelation::SameDeclaredBindings,
        GrantBindingsRelation::BindingsMismatch,
    )
}

fn grant_time_relation(
    grant: &CapabilityGrant,
    receipt: &TransitionReceipt,
) -> GrantDeclaredTimeRelation {
    let instant = receipt.transition.declared_at_unix_ms;
    relation(
        grant.validity.not_before_unix_ms <= instant && instant < grant.validity.expires_at_unix_ms,
        GrantDeclaredTimeRelation::SameDeclaredTime,
        GrantDeclaredTimeRelation::DeclaredTimeMismatch,
    )
}

fn grant_compatibility(
    relations: DeclaredGrantTransitionRelations,
) -> DeclaredGrantTransitionCompatibility {
    let mut reasons = Vec::new();
    if relations.actor == GrantActorRelation::ActorMismatch {
        reasons.push(GrantCompatibilityReason::ActorMismatch);
    }
    if relations.approval_refs == GrantApprovalRefsRelation::ApprovalRefsMismatch {
        reasons.push(GrantCompatibilityReason::ApprovalRefsMismatch);
    }
    if relations.bindings == GrantBindingsRelation::BindingsMismatch {
        reasons.push(GrantCompatibilityReason::BindingsMismatch);
    }
    if relations.declared_time == GrantDeclaredTimeRelation::DeclaredTimeMismatch {
        reasons.push(GrantCompatibilityReason::DeclaredTimeMismatch);
    }
    if relations.grant_ref == GrantRefRelation::GrantRefMismatch {
        reasons.push(GrantCompatibilityReason::GrantRefMismatch);
    }
    if relations.task_binding == GrantTaskBindingRelation::TaskBindingMismatch {
        reasons.push(GrantCompatibilityReason::TaskBindingMismatch);
    }
    DeclaredGrantTransitionCompatibility {
        reason_codes: reasons,
        relations,
        result: GRANT_COMPATIBILITY_RESULT.into(),
    }
}

/// Projects complete strict ADR-0059 records without validating authority.
///
/// # Errors
///
/// Returns an error for malformed records, excess cardinality, duplicates, or wrong order.
pub fn project_approval_refs(
    records: &[ApprovalRecord],
) -> Result<Vec<ApprovalRef>, TransitionReceiptContractError> {
    if records.len() > 32 {
        return Err(invalid("ApprovalRecord projection count exceeds 32"));
    }
    let projected = records
        .iter()
        .map(|record| approval_ref(record).map_err(|error| invalid(error.message)))
        .collect::<Result<Vec<_>, _>>()?;
    primitives::sorted_nodes(&projected, "projected ApprovalRefs")?;
    Ok(projected)
}

/// Compares strict `ApprovalRecord` projections and declared scope only.
///
/// # Errors
///
/// Returns an error when a record or receipt is malformed.
pub fn assess_declared_approval_compatibility(
    records: &[ApprovalRecord],
    receipt: &TransitionReceipt,
) -> Result<DeclaredApprovalTransitionCompatibility, TransitionReceiptContractError> {
    super::validation::validate_receipt(receipt, false)?;
    let refs = project_approval_refs(records)?;
    let relations = DeclaredApprovalTransitionRelations {
        ref_set: relation(
            receipt.approval_refs == refs,
            ApprovalRefSetRelation::SameDeclaredRefSet,
            ApprovalRefSetRelation::RefSetMismatch,
        ),
        scope: relation(
            records.iter().all(|record| scope_matches(record, receipt)),
            ApprovalTransitionScopeRelation::SameDeclaredScope,
            ApprovalTransitionScopeRelation::ScopeMismatch,
        ),
    };
    Ok(approval_compatibility(relations))
}

fn scope_matches(record: &ApprovalRecord, receipt: &TransitionReceipt) -> bool {
    let scope = &record.scope;
    let task = &receipt.task_binding;
    scope.project_id == task.project_id
        && scope.change_id == task.change_id
        && scope.environment_class == task.environment_class
        && scope.environment_id == task.environment_id
        && scope.gate_id == receipt.transition.gate_id
}

fn approval_compatibility(
    relations: DeclaredApprovalTransitionRelations,
) -> DeclaredApprovalTransitionCompatibility {
    let mut reasons = Vec::new();
    if relations.ref_set == ApprovalRefSetRelation::RefSetMismatch {
        reasons.push(ApprovalCompatibilityReason::RefSetMismatch);
    }
    if relations.scope == ApprovalTransitionScopeRelation::ScopeMismatch {
        reasons.push(ApprovalCompatibilityReason::ScopeMismatch);
    }
    DeclaredApprovalTransitionCompatibility {
        reason_codes: reasons,
        relations,
        result: APPROVAL_COMPATIBILITY_RESULT.into(),
    }
}

fn relation<T: Copy>(matches: bool, positive: T, negative: T) -> T {
    if matches { positive } else { negative }
}
