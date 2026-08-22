use crate::governance_contract::GovernanceRecord;

use super::{
    super::*,
    support::{fixture, reseal_proposal, reseal_record},
};

#[test]
fn exact_closure_rejects_orphan_records() {
    let mut proposal = fixture().knowledge_update_proposal;
    let mut orphan = proposal
        .records
        .iter()
        .find(|record| matches!(record, GovernanceRecord::Evidence(_)))
        .expect("fixture evidence")
        .clone();
    let GovernanceRecord::Evidence(value) = &mut orphan else {
        unreachable!();
    };
    value.metadata.record_id = "evidence-knowledge-update-orphan".into();
    value.metadata.aggregate_id = "evidence-kup-orphan".into();
    reseal_record(&mut orphan);
    proposal.records.push(orphan);
    proposal
        .records
        .sort_by(|left, right| left.metadata().record_id.cmp(&right.metadata().record_id));
    assert!(reseal_proposal(&proposal).is_err());
}

#[test]
fn mutation_targets_must_be_exact_claim_refs() {
    let mut proposal = fixture().knowledge_update_proposal;
    let evidence = proposal
        .records
        .iter()
        .find(|record| matches!(record, GovernanceRecord::Evidence(_)))
        .expect("fixture evidence");
    proposal.mutations[0].after_claim_ref = ClaimRef {
        canonical_sha256: evidence.integrity().canonical_sha256.clone(),
        record_id: evidence.metadata().record_id.clone(),
    };
    assert!(reseal_proposal(&proposal).is_err());

    let mut digest = fixture().knowledge_update_proposal;
    digest.mutations[0].after_claim_ref.canonical_sha256 = "0".repeat(64);
    assert!(reseal_proposal(&digest).is_err());
}

#[test]
fn embedded_claim_references_must_resolve_in_the_closed_set() {
    let mut proposal = fixture().knowledge_update_proposal;
    let record = proposal
        .records
        .iter_mut()
        .find(|record| record.metadata().record_id == "claim-knowledge-update-create")
        .expect("created claim");
    let GovernanceRecord::Claim(claim) = record else {
        unreachable!();
    };
    claim.spec.supporting_evidence_record_ids = vec!["evidence-not-present".into()];
    reseal_record(record);
    let digest = record.integrity().canonical_sha256.clone();
    proposal
        .mutations
        .iter_mut()
        .find(|mutation| mutation.target_aggregate_id == "claim-kup-create")
        .expect("create mutation")
        .after_claim_ref
        .canonical_sha256 = digest;
    assert!(reseal_proposal(&proposal).is_err());
}
