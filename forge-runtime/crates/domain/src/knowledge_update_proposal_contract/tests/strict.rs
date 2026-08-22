use super::{
    super::*,
    support::{fixture, reseal_proposal},
};

#[test]
fn strict_decoder_rejects_noncanonical_and_unknown_input() {
    let proposal = fixture().knowledge_update_proposal;
    let canonical =
        super::super::canonical::encode(&proposal, MAX_PROPOSAL_BYTES, "strict proposal")
            .expect("canonical proposal");
    let mut spaced = b" ".to_vec();
    spaced.extend_from_slice(canonical.as_bytes());
    assert!(decode_canonical_proposal(&spaced).is_err());

    let mut value = serde_json::to_value(&proposal).expect("proposal value");
    value
        .as_object_mut()
        .expect("proposal object")
        .insert("applied".into(), serde_json::Value::Bool(true));
    let bytes = serde_json::to_vec(&value).expect("unknown-field proposal");
    assert!(decode_canonical_proposal(&bytes).is_err());
}

#[test]
fn strict_validation_rejects_digest_and_order_drift() {
    let base = fixture().knowledge_update_proposal;
    let mut digest = base.clone();
    digest.proposal_sha256.replace_range(..1, "0");
    assert!(validate_proposal(&digest).is_err());

    let mut mutations = base.clone();
    mutations.mutations.reverse();
    assert!(reseal_proposal(&mutations).is_err());

    let mut artifacts = base.clone();
    artifacts
        .bindings
        .artifacts
        .push(base.bindings.artifacts[0].clone());
    assert!(reseal_proposal(&artifacts).is_err());
}

#[test]
fn mutation_reasons_reuse_the_exact_adr_0045_identifier_lexicon() {
    let base = fixture().knowledge_update_proposal;
    for reason in ["1_declared", "source:revision/path"] {
        let mut proposal = base.clone();
        proposal.mutations[0].reason_codes = vec![reason.into()];
        reseal_proposal(&proposal).expect("ADR-0045 identifier reason code");
    }

    for reason in ["Uppercase", "space separated"] {
        let mut proposal = base.clone();
        proposal.mutations[0].reason_codes = vec![reason.into()];
        assert!(reseal_proposal(&proposal).is_err());
    }
}

#[test]
fn records_free_target_rejects_duplicate_after_refs_and_supersession_forks() {
    let proposal = fixture().knowledge_update_proposal;
    let target = declared_target(&proposal).expect("golden declared target");

    let mut duplicate_after = target.clone();
    duplicate_after.mutations[1].after_claim_ref =
        duplicate_after.mutations[0].after_claim_ref.clone();
    assert!(canonical_declared_target_json(&duplicate_after).is_err());

    let mut forked_before = target;
    forked_before.mutations[0].operation = MutationOperation::Supersede;
    forked_before.mutations[0].before_claim_ref =
        forked_before.mutations[1].before_claim_ref.clone();
    assert!(canonical_declared_target_json(&forked_before).is_err());
}

#[test]
fn records_free_target_and_request_reject_self_supersede_references() {
    let proposal = fixture().knowledge_update_proposal;
    let mut target = declared_target(&proposal).expect("golden declared target");
    let supersede = target
        .mutations
        .iter_mut()
        .find(|mutation| mutation.operation == MutationOperation::Supersede)
        .expect("supersede mutation");
    supersede.before_claim_ref = Some(supersede.after_claim_ref.clone());

    assert!(canonical_declared_target_json(&target).is_err());
    assert!(seal_assessment_request(&proposal, &target, proposal.submitted_at_unix_ms).is_err());
}

#[test]
fn records_free_target_and_request_reject_cross_mutation_ref_overlap() {
    let proposal = fixture().knowledge_update_proposal;
    let mut target = declared_target(&proposal).expect("golden declared target");
    let create_after = target
        .mutations
        .iter()
        .find(|mutation| mutation.operation == MutationOperation::Create)
        .expect("create mutation")
        .after_claim_ref
        .clone();
    target
        .mutations
        .iter_mut()
        .find(|mutation| mutation.operation == MutationOperation::Supersede)
        .expect("supersede mutation")
        .before_claim_ref = Some(create_after);

    assert!(canonical_declared_target_json(&target).is_err());
    assert!(seal_assessment_request(&proposal, &target, proposal.submitted_at_unix_ms).is_err());
}

#[test]
fn seal_proposal_requires_an_explicitly_unsealed_candidate() {
    let sealed = fixture().knowledge_update_proposal;
    assert!(seal_proposal(&sealed).is_err());
    let mut candidate = sealed;
    candidate.proposal_id.clear();
    candidate.proposal_sha256.clear();
    assert!(seal_proposal(&candidate).is_ok());
}

#[test]
fn aliases_and_apply_shaped_mutations_fail_closed() {
    let proposal = fixture().knowledge_update_proposal;
    let mut value = serde_json::to_value(&proposal).expect("proposal value");
    value["mutations"][0]["operation"] = serde_json::Value::String("apply".into());
    let bytes = serde_json::to_vec(&value).expect("alias proposal");
    assert!(decode_canonical_proposal(&bytes).is_err());
}
