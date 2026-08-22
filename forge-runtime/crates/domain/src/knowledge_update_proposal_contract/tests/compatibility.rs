use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::{
    capability_grant_contract::{
        CapabilityGrant, EffectId, GovernanceObjectKind, ScopeClause, ScopeResource, grant_sha256,
    },
    context_package_contract::{
        ContextPackage, ContextPackageBuildRequest, ContextPackageContractError, TokenCounter,
        TokenizerIdentity, assemble,
    },
    governance_contract::GovernanceRecord,
};

use super::{
    super::*,
    support::{fixture, reseal_proposal, reseal_record},
};

#[derive(Deserialize)]
struct GrantGolden {
    grant: CapabilityGrant,
}

#[derive(Deserialize)]
struct ContextGolden {
    expected_package: ContextPackage,
    request: ContextPackageBuildRequest,
}

struct ByteCounter;

impl TokenCounter for ByteCounter {
    fn identity(&self) -> TokenizerIdentity {
        TokenizerIdentity {
            tokenizer_id: "forgeos.token-counter.utf8-bytes/v1".into(),
            tokenizer_sha256: "44799f99769528ecb46bcad483faf2d8ff4ab086bf32b2fe692a18f0eebea3cf"
                .into(),
        }
    }

    fn count(&self, projection: &[u8]) -> Result<u64, ContextPackageContractError> {
        Ok(projection.len() as u64)
    }
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures")
        .join(name)
}

fn grant() -> CapabilityGrant {
    let bytes = fs::read(fixture_path("capability-grant-v1.json")).expect("read Grant fixture");
    serde_json::from_slice::<GrantGolden>(&bytes)
        .expect("decode Grant fixture")
        .grant
}

fn context() -> ContextGolden {
    let bytes = fs::read(fixture_path("context-package-v1.json")).expect("read Context fixture");
    serde_json::from_slice(&bytes).expect("decode Context fixture")
}

fn matching_grant(proposal: &mut KnowledgeUpdateProposal) -> CapabilityGrant {
    let mut grant = grant();
    grant.scope.effect_id = EffectId::KnowledgePropose;
    grant.scope.allow = vec![ScopeClause {
        resources: vec![knowledge_resource(proposal)],
    }];
    grant.scope.deny.clear();
    reseal_grant(&mut grant);
    proposal.capability_grant_ref =
        project_capability_grant_ref(&grant).expect("project resealed Grant ref");
    *proposal = reseal_proposal(proposal).expect("reseal proposal Grant binding");
    grant
}

fn knowledge_resource(proposal: &KnowledgeUpdateProposal) -> ScopeResource {
    ScopeResource::GovernanceObject {
        object_kind: GovernanceObjectKind::Knowledge,
        object_ref: proposal.knowledge_scope.object_ref.clone(),
        object_scope_sha256: proposal.knowledge_scope.object_scope_sha256.clone(),
    }
}

fn reseal_grant(grant: &mut CapabilityGrant) {
    grant.grant_id.clear();
    grant.grant_sha256.clear();
    grant.grant_sha256 = grant_sha256(grant).expect("recompute Grant digest");
    grant.grant_id = format!("capability-grant-{}", grant.grant_sha256);
}

fn matching_context() -> (
    ContextPackageBuildRequest,
    ContextPackage,
    KnowledgeUpdateProposal,
) {
    let mut context = context();
    let mut proposal = fixture().knowledge_update_proposal;
    let bindings = &proposal.bindings;
    context.request.source_binding.policy_sha256 = bindings.policy_sha256.clone();
    context.request.source_binding.source_revision = bindings.source_revision.clone();
    context.request.source_binding.source_tree_sha256 = bindings.source_tree_sha256.clone();
    context.request.task_binding.change_id = proposal.task_binding.change_id.clone();
    context.request.task_binding.node_id = proposal.task_binding.node_id.clone();
    context.request.task_binding.project_id = proposal.task_binding.project_id.clone();
    context.request.task_binding.role = proposal.task_binding.role.clone();
    context.request.task_binding.run_id = proposal.task_binding.run_id.clone();
    context.request.task_binding.task_id = proposal.task_binding.task_id.clone();
    let package = assemble(&context.request, &ByteCounter).expect("assemble matching Context");
    proposal.bindings.context_sha256 = package.context_sha256.clone();
    let (mutations, records) = (&mut proposal.mutations, &mut proposal.records);
    for mutation in mutations {
        let record = records
            .iter_mut()
            .find(|record| record.metadata().record_id == mutation.after_claim_ref.record_id)
            .expect("mutation root");
        let GovernanceRecord::Claim(claim) = record else {
            panic!("mutation root must remain a claim")
        };
        claim.metadata.context_sha256 = package.context_sha256.clone();
        reseal_record(record);
        mutation.after_claim_ref.canonical_sha256 = record.integrity().canonical_sha256.clone();
    }
    let proposal = reseal_proposal(&proposal).expect("reseal Context-bound proposal");
    (context.request, package, proposal)
}

#[test]
fn matching_knowledge_propose_grant_is_only_declaration_compatible() {
    let mut proposal = fixture().knowledge_update_proposal;
    let grant = matching_grant(&mut proposal);
    let compatibility = assess_declared_grant_compatibility(&grant, &proposal)
        .expect("declared Grant compatibility");
    assert!(compatibility.reason_codes.is_empty());
    assert_eq!(compatibility.result, GRANT_COMPATIBILITY_RESULT);
    assert_eq!(
        compatibility.relations.scope,
        GrantScopeRelation::CoveredByDeclaration
    );
}

#[test]
fn matching_grant_scope_uses_declared_deny_precedence() {
    let mut proposal = fixture().knowledge_update_proposal;
    let mut grant = matching_grant(&mut proposal);
    grant.scope.deny = vec![knowledge_resource(&proposal)];
    reseal_grant(&mut grant);
    proposal.capability_grant_ref =
        project_capability_grant_ref(&grant).expect("project denied Grant ref");
    proposal = reseal_proposal(&proposal).expect("reseal denied Grant binding");
    let compatibility = assess_declared_grant_compatibility(&grant, &proposal)
        .expect("declared deny compatibility");
    assert_eq!(
        compatibility.relations.scope,
        GrantScopeRelation::DeniedByDeclaration
    );
    assert_eq!(
        compatibility.reason_codes,
        vec![GrantKnowledgeUpdateReason::DenyMatched]
    );
}

#[test]
fn effect_mismatch_omits_redundant_scope_reason() {
    let proposal = fixture().knowledge_update_proposal;
    let compatibility = assess_declared_grant_compatibility(&grant(), &proposal)
        .expect("mismatched Grant compatibility");
    assert_eq!(
        compatibility.relations.effect,
        GrantEffectRelation::EffectMismatch
    );
    assert_eq!(
        compatibility.relations.scope,
        GrantScopeRelation::OutsideDeclaredScope
    );
    assert!(
        compatibility
            .reason_codes
            .contains(&GrantKnowledgeUpdateReason::EffectMismatch)
    );
    assert!(
        !compatibility
            .reason_codes
            .contains(&GrantKnowledgeUpdateReason::ScopeNotCovered)
    );
}

#[test]
fn context_compatibility_reassembles_before_comparison() {
    let fixture = fixture();
    let mut context = context();
    let proposal = fixture.knowledge_update_proposal;
    context.expected_package.context_sha256 = proposal.bindings.context_sha256.clone();
    assert!(
        assess_declared_context_compatibility(
            &context.request,
            &context.expected_package,
            &ByteCounter,
            &proposal,
        )
        .is_err(),
        "a rehashed-looking Context field cannot bypass deterministic reassembly"
    );
}

#[test]
fn matching_reassembled_context_is_only_declaration_compatible() {
    let (request, package, proposal) = matching_context();
    let compatibility =
        assess_declared_context_compatibility(&request, &package, &ByteCounter, &proposal)
            .expect("declared Context compatibility");
    assert!(compatibility.reason_codes.is_empty());
    assert_eq!(compatibility.result, CONTEXT_COMPATIBILITY_RESULT);
    assert_eq!(
        compatibility.relations.context,
        ContextDigestRelation::SameDeclaredContext
    );
    assert_eq!(
        compatibility.relations.freshness,
        ContextFreshnessRelation::InsideDeclaredFreshness
    );
}

#[test]
fn context_expiry_is_a_declared_mismatch_without_freshness_attestation() {
    let (request, package, mut proposal) = matching_context();
    proposal.submitted_at_unix_ms = package
        .freshness
        .expires_at_unix_ms
        .expect("fixture Context expiry");
    proposal = reseal_proposal(&proposal).expect("reseal expired Context declaration");
    let compatibility =
        assess_declared_context_compatibility(&request, &package, &ByteCounter, &proposal)
            .expect("expired Context compatibility");
    assert_eq!(
        compatibility.relations.freshness,
        ContextFreshnessRelation::OutsideDeclaredFreshness
    );
    assert_eq!(
        compatibility.reason_codes,
        vec![ContextKnowledgeUpdateReason::FreshnessMismatch]
    );
}

#[test]
fn artifact_projection_is_pure_and_exact() {
    let fixture = fixture();
    assert_eq!(
        project_artifact_resources(&fixture.knowledge_update_proposal)
            .expect("pure artifact projection"),
        fixture.expected_artifact_resources
    );
}
