use super::receipt_fixture;
use crate::platform_core_contract::{
    canonical_execution_receipt_json, canonical_verification_receipt_json,
    canonical_verification_request_json, decode_canonical_execution_receipt,
    decode_canonical_verification_receipt, decode_canonical_verification_request,
    execution_receipt_sha256, validate_verification_exchange, verification_receipt_sha256,
    verification_request_sha256,
};

#[test]
fn receipt_golden_digests_and_round_trips_match() {
    let fixture = receipt_fixture();
    assert_eq!(
        fixture.api_version,
        "forge.platform-core-receipt-fixture/v1"
    );
    let execution = canonical_execution_receipt_json(&fixture.execution_receipt).unwrap();
    assert_eq!(
        execution_receipt_sha256(&fixture.execution_receipt).unwrap(),
        fixture.expected.execution
    );
    assert_eq!(
        decode_canonical_execution_receipt(execution.as_bytes()).unwrap(),
        fixture.execution_receipt
    );
    assert_request_and_receipt(&fixture);
}

fn assert_request_and_receipt(fixture: &super::ReceiptGoldenFixture) {
    let request = canonical_verification_request_json(&fixture.verification_request).unwrap();
    assert_eq!(
        verification_request_sha256(&fixture.verification_request).unwrap(),
        fixture.expected.verification_request
    );
    assert_eq!(
        decode_canonical_verification_request(request.as_bytes()).unwrap(),
        fixture.verification_request
    );
    let receipt = canonical_verification_receipt_json(&fixture.verification_receipt).unwrap();
    assert_eq!(
        verification_receipt_sha256(&fixture.verification_receipt).unwrap(),
        fixture.expected.verification_receipt
    );
    assert_eq!(
        decode_canonical_verification_receipt(receipt.as_bytes()).unwrap(),
        fixture.verification_receipt
    );
    validate_verification_exchange(&fixture.verification_request, &fixture.verification_receipt)
        .unwrap();
}
