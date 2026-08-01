use serde_json::Value;

use super::*;

const PRIVATE_ENDPOINT: &str = "https://private.example.test/v1/responses";
const PRIVATE_MODEL: &str = "private-model";
const PRIVATE_PRICING: &str = "private-pricing-identity";
const PRIVATE_PROMPT: &str = "private prompt text";

#[test]
fn default_prepared_output_is_metadata_only_and_honest() {
    let output = GroupAgentNodeDispatchRequestCliOutput::Prepared {
        v: 1,
        disposition: PrepareGroupAgentNodeDispatchRequestDisposition::Created,
        inspection: inspection(None),
    };
    let value = json_value(&output);
    let inspected = &value["inspection"];
    assert_eq!(inspected["request_prepared"], true);
    assert_eq!(inspected["request_included"], false);
    assert_eq!(inspected["pricing_snapshot_identity_pinned"], true);
    assert_eq!(inspected["pricing_policy_enforced"], false);
    for field in effect_fields() {
        assert_eq!(inspected[field], false, "{field} must remain false");
    }
    assert_private(&value);
}

#[test]
fn explicit_request_round_trips_exact_text_and_human_output_escapes_controls() {
    let request = "exact\nrequest\u{1b}[2J\u{202e}";
    let output = GroupAgentNodeDispatchRequestCliOutput::DispatchRequest {
        v: 1,
        inspection: inspection(Some(request.into())),
    };
    let value = json_value(&output);
    assert_eq!(value["inspection"]["provider_request_body"], request);
    assert_eq!(value["inspection"]["request_included"], true);

    let mut human = Vec::new();
    write_output(&output, false, &mut human).expect("write human output");
    let human = String::from_utf8(human).expect("UTF-8 output");
    assert!(human.contains(r"provider request: exact\nrequest\x1b[2J\u{202e}"));
    assert!(!human.contains('\u{1b}'));
    assert!(!human.contains('\u{202e}'));
}

#[test]
fn list_states_metadata_validation_boundary_and_redacts_private_values() {
    let output = GroupAgentNodeDispatchRequestCliOutput::DispatchRequests {
        v: 1,
        metadata_only: true,
        source_contract_and_request_validated: false,
        returned_requests_present: true,
        request_preparation_validated: false,
        dispatch_authority_released: false,
        fresh_off_machine_consent_obtained: false,
        credential_read: false,
        execution_performed: false,
        model_used: false,
        provider_used: false,
        network_accessed: false,
        workspace_accessed: false,
        tools_used: false,
        result_produced: false,
        conversation_or_prompt_written: false,
        memory_written: false,
        writeback_performed: false,
        pricing_snapshot_identity_validated: false,
        pricing_policy_enforced: false,
        request_included: false,
        requests: vec![metadata()],
    };
    let value = json_value(&output);
    assert_eq!(value["metadata_only"], true);
    assert_eq!(value["source_contract_and_request_validated"], false);
    assert_eq!(value["request_preparation_validated"], false);
    assert_eq!(value["pricing_snapshot_identity_validated"], false);
    assert_eq!(value["request_included"], false);
    assert_private(&value);

    let mut human = Vec::new();
    write_output(&output, false, &mut human).expect("write metadata output");
    let human = String::from_utf8(human).expect("UTF-8 output");
    assert!(human.contains("metadata reports stored request rows"));
    assert!(!human.contains("exact provider request prepared locally"));
    assert!(!human.contains("pricing identity pinned"));
}

#[test]
fn empty_list_does_not_infer_request_or_pricing_state() {
    let output = GroupAgentNodeDispatchRequestCliOutput::list(Vec::new());
    let value = json_value(&output);
    assert_eq!(value["returned_requests_present"], false);
    assert_eq!(value["request_preparation_validated"], false);
    assert_eq!(value["pricing_snapshot_identity_validated"], false);

    let mut human = Vec::new();
    write_output(&output, false, &mut human).expect("write empty output");
    let human = String::from_utf8(human).expect("UTF-8 output");
    assert!(human.contains("no request metadata returned; preparation was not inferred"));
    assert!(!human.contains("exact provider request prepared locally"));
    assert!(!human.contains("pricing identity pinned"));
}

fn inspection(provider_request_body: Option<String>) -> DispatchRequestInspectionView {
    DispatchRequestInspectionView {
        v: 1,
        request_prepared: true,
        source_graph_validated: true,
        contract_and_journal_validated: true,
        request_body_validated: true,
        dispatch_authority_released: false,
        fresh_off_machine_consent_obtained: false,
        credential_read: false,
        execution_performed: false,
        model_selected: true,
        model_used: false,
        provider_used: false,
        network_accessed: false,
        workspace_accessed: false,
        tools_used: false,
        result_produced: false,
        conversation_or_prompt_written: false,
        memory_written: false,
        writeback_performed: false,
        pricing_snapshot_identity_pinned: true,
        pricing_policy_enforced: false,
        request_included: provider_request_body.is_some(),
        record: metadata(),
        provider_request_body,
    }
}

fn metadata() -> DispatchRequestMetadataView {
    DispatchRequestMetadataView {
        v: 1,
        dispatch_request_id: "node-dispatch-request-safe-id".into(),
        graph_run_id: "graph-run-safe-id".into(),
        contract_id: "node-contract-safe-id".into(),
        attempt: 1,
        provider_request_bytes: 42,
        codec_protocol_version: 1,
        created_at_ms: 7,
    }
}

fn json_value(output: &GroupAgentNodeDispatchRequestCliOutput) -> Value {
    serde_json::to_value(output).expect("serialize CLI output")
}

fn effect_fields() -> [&'static str; 12] {
    [
        "dispatch_authority_released",
        "fresh_off_machine_consent_obtained",
        "credential_read",
        "execution_performed",
        "model_used",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "result_produced",
        "memory_written",
        "writeback_performed",
    ]
}

fn assert_private(value: &Value) {
    let encoded = serde_json::to_string(value).expect("encode JSON");
    for secret in [
        PRIVATE_ENDPOINT,
        PRIVATE_MODEL,
        PRIVATE_PRICING,
        PRIVATE_PROMPT,
    ] {
        assert!(!encoded.contains(secret), "default output leaked {secret}");
    }
}
