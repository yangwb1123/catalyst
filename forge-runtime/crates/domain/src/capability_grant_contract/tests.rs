use serde::Deserialize;

use super::*;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/capability-grant-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GoldenFixture {
    pub(super) assessment_request: DeclaredAssessmentRequest,
    pub(super) effect_vocabulary: EffectVocabulary,
    pub(super) expected_assessment: DeclaredAssessment,
    pub(super) grant: CapabilityGrant,
}

pub(super) fn fixture() -> GoldenFixture {
    serde_json::from_str(FIXTURE).expect("CapabilityGrant golden fixture")
}

pub(super) fn reseal_grant(grant: &mut CapabilityGrant) {
    grant.grant_sha256 = grant_sha256(grant).expect("reseal grant");
    grant.grant_id = format!("capability-grant-{}", grant.grant_sha256);
}

pub(super) fn reseal_request(request: &mut DeclaredAssessmentRequest) {
    request.request_sha256 = assessment_request_sha256(request).expect("reseal request");
}

mod bounds;
mod qualifier;
mod secret;
mod strict;

#[test]
fn golden_fixture_digests_and_assessment_match() {
    let fixture = fixture();
    assert_eq!(fixture.grant, fixture.assessment_request.grant);
    assert_eq!(
        effect_vocabulary_sha256(&fixture.effect_vocabulary).expect("vocabulary digest"),
        "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f"
    );
    assert_eq!(
        grant_sha256(&fixture.grant).expect("grant digest"),
        "892fd08c827835a3d7e742bda656cd3abf78e7757248e1ac84583715146250c3"
    );
    assert_eq!(
        requested_action_sha256(&fixture.assessment_request.requested_action)
            .expect("action digest"),
        "6b5e12d76919b3ed5aab0f235f7b5bd569232d376fa9e6498f80f569c6ab7f11"
    );
    assert_eq!(
        assessment_request_sha256(&fixture.assessment_request).expect("request digest"),
        "192d46339703d90b8b19fc8dcd08ded549236cd83ad942019922218d71576f8b"
    );
    let assessment =
        evaluate_declared_assessment(&fixture.effect_vocabulary, &fixture.assessment_request)
            .expect("evaluate declarations");
    assert_eq!(assessment, fixture.expected_assessment);
    assert_eq!(
        assessment.assessment_sha256,
        "ae8784d3f2cbe296e5968f9e4adbd7d696e956b7424dfc7abf75ba838540f94d"
    );
}

#[test]
fn canonical_documents_round_trip() {
    let fixture = fixture();
    let vocabulary =
        canonical_effect_vocabulary_json(&fixture.effect_vocabulary).expect("canonical vocabulary");
    assert_eq!(
        decode_canonical_effect_vocabulary(vocabulary.as_bytes()).expect("decode vocabulary"),
        fixture.effect_vocabulary
    );
    let grant = canonical_grant_json(&fixture.grant).expect("canonical grant");
    assert_eq!(
        decode_canonical_grant(grant.as_bytes()).expect("decode grant"),
        fixture.grant
    );
    let request =
        canonical_assessment_request_json(&fixture.assessment_request).expect("canonical request");
    assert_eq!(
        decode_canonical_assessment_request(request.as_bytes()).expect("decode request"),
        fixture.assessment_request
    );
    let assessment =
        canonical_assessment_json(&fixture.expected_assessment).expect("canonical assessment");
    assert_eq!(
        decode_canonical_assessment(assessment.as_bytes()).expect("decode assessment"),
        fixture.expected_assessment
    );
}
