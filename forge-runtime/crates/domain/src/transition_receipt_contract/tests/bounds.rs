use super::super::{
    MAX_ASSESSMENT_REQUEST_BYTES, canonical_declared_target_json,
    decode_canonical_assessment_request,
};
use super::fixture;

#[test]
fn programmatic_document_limits_apply_before_digest_or_evaluation() {
    let mut target = fixture().assessment_request.expected_target;
    target.reason_codes = (0..257).map(|index| format!("reason_{index:03}")).collect();
    assert!(canonical_declared_target_json(&target).is_err());
    let mut target = fixture().assessment_request.expected_target;
    target.preconditions = vec![target.preconditions[0].clone(); 65];
    assert!(canonical_declared_target_json(&target).is_err());
    let mut target = fixture().assessment_request.expected_target;
    target.bindings.artifacts = vec![target.bindings.artifacts[0].clone(); 33];
    assert!(canonical_declared_target_json(&target).is_err());
}

#[test]
fn byte_decoder_limits_fail_before_deserialization() {
    let oversized = vec![b' '; MAX_ASSESSMENT_REQUEST_BYTES + 1];
    assert!(decode_canonical_assessment_request(&oversized).is_err());
    let deep = format!("{}0{}", "[".repeat(2_000), "]".repeat(2_000));
    assert!(decode_canonical_assessment_request(deep.as_bytes()).is_err());
}

#[test]
fn programmatic_byte_measurement_rejects_before_canonical_buffer() {
    let oversized = vec!["\\".repeat(16_384); 256];
    let result = super::super::codec::bounded(&oversized, 1_048_576, "oversized fixture");
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("byte ceiling"));
}
