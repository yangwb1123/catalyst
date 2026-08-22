use sha2::{Digest, Sha256};

use super::super::{
    assessment_sha256, canonical_assessment_json, canonical_assessment_request_json,
    canonical_declared_target_json, canonical_receipt_json, canonical_vocabulary_json,
    decode_canonical_assessment, decode_canonical_assessment_request,
    decode_canonical_declared_target, decode_canonical_receipt, decode_canonical_vocabulary,
    evaluate_declared_assessment, transition_vocabulary, validate_assessment, vocabulary_sha256,
};
use super::{FIXTURE, fixture, repo_root};

const VOCABULARY_HASH: &str = "cc354fb2b440d81514045b50266d41d3964b6440ed9d40afa17f5991519d7d0d";
const RECEIPT_HASH: &str = "3d80d9578051338e447f674eedbb856455cd1e672247d88fbba8c51dab9bcb5d";
const TARGET_HASH: &str = "8be69d5504d243bdb7fedc418c48559055d6639a33edb9aa9b4cb08c3f948d9a";
const REQUEST_HASH: &str = "20e3378571ef708b211ae145dbd285356a1ac05f6dae68784b71562fd95eed7f";
const ASSESSMENT_HASH: &str = "5e4d62eedecaf2abd9c7f2030466ebc158cefbaa6f01ec21cfebd33db129eb6a";

#[test]
fn test_frozen_golden_schema_and_all_five_hashes() {
    let golden = fixture();
    assert_eq!(
        vocabulary_sha256(&golden.transition_vocabulary).unwrap(),
        VOCABULARY_HASH
    );
    assert_eq!(
        super::super::receipt_sha256(&golden.transition_receipt).unwrap(),
        RECEIPT_HASH
    );
    assert_eq!(
        super::super::declared_target_sha256(&golden.assessment_request.expected_target).unwrap(),
        TARGET_HASH
    );
    assert_eq!(
        super::super::assessment_request_sha256(&golden.assessment_request).unwrap(),
        REQUEST_HASH
    );
    assert_eq!(
        assessment_sha256(&golden.expected_assessment).unwrap(),
        ASSESSMENT_HASH
    );
}

#[test]
fn golden_round_trips_every_strict_document() {
    let golden = fixture();
    let vocabulary = canonical_vocabulary_json(&golden.transition_vocabulary).unwrap();
    let receipt = canonical_receipt_json(&golden.transition_receipt).unwrap();
    let target =
        canonical_declared_target_json(&golden.assessment_request.expected_target).unwrap();
    let request = canonical_assessment_request_json(&golden.assessment_request).unwrap();
    let assessment = canonical_assessment_json(&golden.expected_assessment).unwrap();
    assert_eq!(
        decode_canonical_vocabulary(vocabulary.as_bytes()).unwrap(),
        golden.transition_vocabulary
    );
    assert_eq!(
        decode_canonical_receipt(receipt.as_bytes()).unwrap(),
        golden.transition_receipt
    );
    assert_eq!(
        decode_canonical_declared_target(target.as_bytes()).unwrap(),
        golden.assessment_request.expected_target
    );
    assert_eq!(
        decode_canonical_assessment_request(request.as_bytes()).unwrap(),
        golden.assessment_request
    );
    assert_eq!(
        decode_canonical_assessment(assessment.as_bytes()).unwrap(),
        golden.expected_assessment
    );
}

#[test]
fn exact_assessment_and_projections_match_golden() {
    let golden = fixture();
    let assessment = evaluate_declared_assessment(&golden.assessment_request).unwrap();
    assert_eq!(assessment, golden.expected_assessment);
    validate_assessment(&golden.assessment_request, &assessment).unwrap();
    assert_eq!(
        golden.transition_receipt.approval_refs,
        golden.expected_approval_refs
    );
    assert_eq!(
        golden.transition_receipt.capability_grant_ref,
        golden.expected_capability_grant_ref
    );
    assert_eq!(transition_vocabulary(), golden.transition_vocabulary);
}

#[test]
fn schema_and_fixture_file_pins_are_exact() {
    let schema =
        std::fs::read(repo_root().join("docs/contracts/transition-receipt-v1.schema.json"))
            .expect("read schema");
    let fixture =
        std::fs::read(repo_root().join("docs/contracts/fixtures/transition-receipt-v1.json"))
            .expect("read fixture");
    assert_eq!(
        format!("{:x}", Sha256::digest(schema)),
        "94962069c93f55129506b9d4b45f1f9db6d9425ecbdbaef9c06fcbe155e43cbf"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(fixture)),
        "dac0b6d8921aaecaf138c5b62924c8a3b9ac8f9c531a67f2be358d47c1c30da9"
    );
    assert!(FIXTURE.ends_with('\n'));
}
