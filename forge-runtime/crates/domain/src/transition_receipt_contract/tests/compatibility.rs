use crate::{approval_record_contract::ApprovalRecord, capability_grant_contract::CapabilityGrant};

use super::super::{
    ApprovalRefSetRelation, ApprovalTransitionScopeRelation, GrantActorRelation,
    GrantApprovalRefsRelation, GrantBindingsRelation, GrantDeclaredTimeRelation, GrantRefRelation,
    GrantTaskBindingRelation, assess_declared_approval_compatibility,
    assess_declared_grant_compatibility, project_approval_refs, project_capability_grant_ref,
};
use super::{fixture, reseal_receipt};

fn grant() -> CapabilityGrant {
    let bytes = include_str!("../../../../../../docs/contracts/fixtures/capability-grant-v1.json");
    let value: serde_json::Value = serde_json::from_str(bytes).unwrap();
    serde_json::from_value(value["grant"].clone()).unwrap()
}

fn approval() -> ApprovalRecord {
    let bytes = include_str!("../../../../../../docs/contracts/fixtures/approval-record-v1.json");
    let value: serde_json::Value = serde_json::from_str(bytes).unwrap();
    serde_json::from_value(value["approval_record"].clone()).unwrap()
}

#[test]
fn grant_projection_matches_golden_without_new_transition_effect() {
    let golden = fixture();
    assert_eq!(
        project_capability_grant_ref(&grant()).unwrap(),
        golden.expected_capability_grant_ref
    );
    let grant_json = crate::capability_grant_contract::canonical_grant_json(&grant()).unwrap();
    assert!(!grant_json.contains("lifecycle.transition"));
}

#[test]
fn matching_grant_relations_remain_authority_neutral() {
    let grant = grant();
    let mut receipt = fixture().transition_receipt;
    receipt.approval_refs.clear();
    reseal_receipt(&mut receipt);
    let result = assess_declared_grant_compatibility(&grant, &receipt).unwrap();
    assert!(result.reason_codes.is_empty());
    assert_eq!(
        result.relations.actor,
        GrantActorRelation::SameDeclaredActor
    );
    assert_eq!(
        result.relations.approval_refs,
        GrantApprovalRefsRelation::SameDeclaredApprovalRefs
    );
    assert_eq!(
        result.relations.bindings,
        GrantBindingsRelation::SameDeclaredBindings
    );
    assert_eq!(
        result.relations.declared_time,
        GrantDeclaredTimeRelation::SameDeclaredTime
    );
    assert_eq!(
        result.relations.grant_ref,
        GrantRefRelation::SameDeclaredGrantRef
    );
    assert_eq!(
        result.relations.task_binding,
        GrantTaskBindingRelation::SameDeclaredTaskBinding
    );
    assert!(
        result
            .result
            .contains("no permission or transition authority")
    );
}

#[test]
fn every_grant_mutation_has_only_a_declared_mismatch() {
    let grant = grant();
    let mut receipt = fixture().transition_receipt;
    receipt.approval_refs.clear();
    receipt.actor.principal_id = "other".into();
    receipt.bindings.context_sha256 = "d".repeat(64);
    receipt.transition.declared_at_unix_ms = grant.validity.expires_at_unix_ms;
    receipt.capability_grant_ref.grant_sha256 = "d".repeat(64);
    receipt.capability_grant_ref.grant_id = format!("capability-grant-{}", "d".repeat(64));
    receipt.task_binding.task_id = "other-task".into();
    reseal_receipt(&mut receipt);
    let result = assess_declared_grant_compatibility(&grant, &receipt).unwrap();
    assert_eq!(result.relations.actor, GrantActorRelation::ActorMismatch);
    assert_eq!(
        result.relations.bindings,
        GrantBindingsRelation::BindingsMismatch
    );
    assert_eq!(
        result.relations.declared_time,
        GrantDeclaredTimeRelation::DeclaredTimeMismatch
    );
    assert_eq!(
        result.relations.grant_ref,
        GrantRefRelation::GrantRefMismatch
    );
    assert_eq!(
        result.relations.task_binding,
        GrantTaskBindingRelation::TaskBindingMismatch
    );
    assert!(!result.result.to_lowercase().contains("authorized"));
}

#[test]
fn approval_projection_and_scope_are_declarations_only() {
    let record = approval();
    let projected = project_approval_refs(std::slice::from_ref(&record)).unwrap();
    assert_eq!(projected, fixture().expected_approval_refs);
    let result =
        assess_declared_approval_compatibility(&[record], &fixture().transition_receipt).unwrap();
    assert_eq!(
        result.relations.ref_set,
        ApprovalRefSetRelation::SameDeclaredRefSet
    );
    assert_eq!(
        result.relations.scope,
        ApprovalTransitionScopeRelation::ScopeMismatch
    );
    assert!(
        result
            .result
            .contains("no effective approval or transition authority")
    );
}

#[test]
fn duplicate_approval_projection_fails_closed() {
    let record = approval();
    assert!(project_approval_refs(&[record.clone(), record]).is_err());
}
