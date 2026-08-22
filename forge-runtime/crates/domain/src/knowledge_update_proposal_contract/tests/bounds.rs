use super::{
    super::*,
    support::{fixture, reseal_proposal},
};

#[test]
fn programmatic_paths_enforce_mutation_and_artifact_bounds() {
    let base = fixture().knowledge_update_proposal;
    let mut too_many_mutations = base.clone();
    too_many_mutations.mutations = vec![base.mutations[0].clone(); MAX_MUTATIONS + 1];
    assert!(reseal_proposal(&too_many_mutations).is_err());

    let mut too_many_artifacts = base.clone();
    too_many_artifacts.bindings.artifacts =
        vec![base.bindings.artifacts[0].clone(); MAX_ARTIFACTS + 1];
    assert!(reseal_proposal(&too_many_artifacts).is_err());
}

#[test]
fn programmatic_paths_enforce_utf8_text_bounds() {
    let mut proposal = fixture().knowledge_update_proposal;
    proposal.mutations[0].rationale = "界".repeat(1_366);
    assert!(proposal.mutations[0].rationale.len() > 4_096);
    assert!(reseal_proposal(&proposal).is_err());
}

#[test]
fn strict_decoders_reject_outer_byte_overflow_before_parsing() {
    assert!(decode_canonical_proposal(&vec![b' '; MAX_PROPOSAL_BYTES + 1]).is_err());
    assert!(decode_canonical_declared_target(&vec![b' '; MAX_DECLARED_TARGET_BYTES + 1]).is_err());
    assert!(
        decode_canonical_assessment_request(&vec![b' '; MAX_ASSESSMENT_REQUEST_BYTES + 1]).is_err()
    );
    assert!(decode_canonical_assessment(&vec![b' '; MAX_ASSESSMENT_BYTES + 1]).is_err());
}
