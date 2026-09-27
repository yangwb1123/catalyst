use super::*;
use serde_json::Value;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-prompt-append-receipt-v1.json");

#[test]
fn prompt_append_receipt_fixture_recomputes_content_free_digests() {
    let expected: PromptAppendReceiptObservation =
        serde_json::from_str(FIXTURE).expect("strict Prompt append receipt fixture");
    expected.validate().expect("valid Prompt append receipt");
    let actual = observe(PromptAppendReceiptInput {
        owner: expected.owner.clone(),
        conversation_id: expected.request.conversation_id.clone(),
        expected_version: expected.request.expected_version,
        role: expected.request.role.clone(),
        content: "send this from another client".into(),
        idempotency_key: "prompt-key-003".into(),
        prompt_id: expected.receipt.prompt_id.clone(),
        created_at_ms: expected.receipt.created_at_ms,
        replayed: expected.receipt.replayed,
    })
    .expect("project Prompt append receipt");
    let expected_json: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let actual_json = serde_json::to_value(actual).expect("receipt JSON");
    assert_eq!(actual_json, expected_json);
}

#[test]
fn prompt_append_receipt_rejects_wire_binding_and_authority_drift() {
    let duplicate = FIXTURE.replace(
        "  \"schema_version\": \"forge.prompt-append-receipt/v1\",\n",
        "  \"schema_version\": \"forge.prompt-append-receipt/v1\",\n  \"schema_version\": \"forge.prompt-append-receipt/v1\",\n",
    );
    assert!(serde_json::from_str::<PromptAppendReceiptObservation>(&duplicate).is_err());
    let unknown = FIXTURE.replace(
        "  \"owner\": {\n",
        "  \"unexpected\": true,\n  \"owner\": {\n",
    );
    assert!(serde_json::from_str::<PromptAppendReceiptObservation>(&unknown).is_err());
    assert!(
        serde_json::from_str::<PromptAppendReceiptObservation>(&format!("{FIXTURE} {{}}")).is_err()
    );

    let mut value: PromptAppendReceiptObservation = serde_json::from_str(FIXTURE).unwrap();
    value.receipt.aggregate_version += 1;
    assert!(value.validate().is_err());
    let mut value: PromptAppendReceiptObservation = serde_json::from_str(FIXTURE).unwrap();
    value.authority.audit_published = true;
    assert!(value.validate().is_err());
    let mut value: PromptAppendReceiptObservation = serde_json::from_str(FIXTURE).unwrap();
    value.receipt.content_included = true;
    assert!(value.validate().is_err());
}
