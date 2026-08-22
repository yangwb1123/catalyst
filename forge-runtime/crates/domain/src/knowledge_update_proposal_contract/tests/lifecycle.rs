use crate::governance_contract::{ClaimObjectValue, ClaimState, GovernanceRecord};

use super::{
    super::*,
    support::{fixture, reseal_proposal, reseal_record},
};

#[test]
fn golden_covers_create_and_authority_free_supersede() {
    let proposal = fixture().knowledge_update_proposal;
    assert_eq!(proposal.mutations[0].operation, MutationOperation::Create);
    assert_eq!(
        proposal.mutations[1].operation,
        MutationOperation::Supersede
    );
    validate_proposal(&proposal).expect("create and supersede lifecycle");
}

#[test]
fn supersede_rejects_stable_semantic_identity_drift() {
    let mut proposal = fixture().knowledge_update_proposal;
    let record = proposal
        .records
        .iter_mut()
        .find(|record| record.metadata().record_id == "claim-knowledge-update-after")
        .expect("after claim");
    let GovernanceRecord::Claim(claim) = record else {
        unreachable!();
    };
    claim.spec.object_value = ClaimObjectValue::String("drifted-object".into());
    reseal_record(record);
    update_after_ref(&mut proposal, "claim-kup-revise");
    assert!(reseal_proposal(&proposal).is_err());
}

#[test]
fn proposal_cannot_smuggle_an_authoritative_claim_state() {
    let mut proposal = fixture().knowledge_update_proposal;
    let record = proposal
        .records
        .iter_mut()
        .find(|record| record.metadata().record_id == "claim-knowledge-update-after")
        .expect("after claim");
    let GovernanceRecord::Claim(claim) = record else {
        unreachable!();
    };
    claim.status.state = ClaimState::Confirmed;
    reseal_record(record);
    update_after_ref(&mut proposal, "claim-kup-revise");
    assert!(reseal_proposal(&proposal).is_err());
}

#[test]
fn supersede_may_include_history_but_must_include_the_immediate_predecessor() {
    let proposal = proposal_with_multi_ancestor_supersede();
    validate_proposal(&proposal).expect("ADR-0045 permits additional prior history");

    let mut missing_immediate = proposal;
    let after = claim_mut(&mut missing_immediate, "claim-knowledge-update-after");
    claim_mut_record(after)
        .metadata
        .supersedes_record_ids
        .retain(|id| id != "claim-knowledge-update-before");
    reseal_record(after);
    update_after_ref(&mut missing_immediate, "claim-kup-revise");
    assert!(reseal_proposal(&missing_immediate).is_err());
}

fn proposal_with_multi_ancestor_supersede() -> KnowledgeUpdateProposal {
    let mut proposal = fixture().knowledge_update_proposal;
    let mut oldest = proposal
        .records
        .iter()
        .find(|record| record.metadata().record_id == "claim-knowledge-update-before")
        .expect("before claim")
        .clone();
    claim_mut_record(&mut oldest).metadata.record_id = "claim-knowledge-update-oldest".into();
    reseal_record(&mut oldest);
    let before = claim_mut(&mut proposal, "claim-knowledge-update-before");
    claim_mut_record(before).metadata.sequence = 2;
    claim_mut_record(before).metadata.supersedes_record_ids =
        vec!["claim-knowledge-update-oldest".into()];
    reseal_record(before);
    let before_digest = before.integrity().canonical_sha256.clone();
    let after = claim_mut(&mut proposal, "claim-knowledge-update-after");
    claim_mut_record(after).metadata.sequence = 3;
    claim_mut_record(after).metadata.supersedes_record_ids = vec![
        "claim-knowledge-update-before".into(),
        "claim-knowledge-update-oldest".into(),
    ];
    reseal_record(after);
    proposal.records.push(oldest);
    proposal
        .records
        .sort_by(|left, right| left.metadata().record_id.cmp(&right.metadata().record_id));
    let mutation = proposal
        .mutations
        .iter_mut()
        .find(|mutation| mutation.target_aggregate_id == "claim-kup-revise")
        .expect("supersede mutation");
    mutation
        .before_claim_ref
        .as_mut()
        .expect("before ref")
        .canonical_sha256 = before_digest;
    update_after_ref(&mut proposal, "claim-kup-revise");
    reseal_proposal(&proposal).expect("seal multi-ancestor proposal")
}

fn claim_mut<'a>(
    proposal: &'a mut KnowledgeUpdateProposal,
    record_id: &str,
) -> &'a mut GovernanceRecord {
    proposal
        .records
        .iter_mut()
        .find(|record| record.metadata().record_id == record_id)
        .expect("claim record")
}

fn claim_mut_record(
    record: &mut GovernanceRecord,
) -> &mut crate::governance_contract::KnowledgeClaim {
    let GovernanceRecord::Claim(claim) = record else {
        panic!("record must be a KnowledgeClaim")
    };
    claim
}

fn update_after_ref(proposal: &mut KnowledgeUpdateProposal, aggregate_id: &str) {
    let record_id = proposal
        .mutations
        .iter()
        .find(|mutation| mutation.target_aggregate_id == aggregate_id)
        .expect("mutation")
        .after_claim_ref
        .record_id
        .clone();
    let record = proposal
        .records
        .iter()
        .find(|record| record.metadata().record_id == record_id)
        .expect("after record");
    let digest = record.integrity().canonical_sha256.clone();
    let mutation = proposal
        .mutations
        .iter_mut()
        .find(|mutation| mutation.target_aggregate_id == aggregate_id)
        .expect("mutation");
    mutation.after_claim_ref.canonical_sha256 = digest;
}
