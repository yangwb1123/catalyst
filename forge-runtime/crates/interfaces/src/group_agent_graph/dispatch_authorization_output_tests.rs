use super::*;

const PRIVATE_PRICING: &str = "private-pricing-must-not-appear";

#[test]
fn verified_output_is_redacted_and_states_every_effect_boundary() {
    let output = GroupAgentNodeDispatchAuthorizationCliOutput::verified(verified("node-1"));
    let value = serde_json::to_value(&output).expect("serialize output");
    assert_eq!(
        value["type"],
        "group_agent_node_dispatch_authorization_verified"
    );
    assert_eq!(value["metadata_only"], true);
    assert_eq!(value["authorization_validated"], true);
    assert_eq!(value["dispatch_authority_release_authorized"], true);
    for field in false_fields() {
        assert_eq!(value[field], false, "{field} must remain false");
    }
    assert_eq!(
        value["authorization"]["dispatch_request_id"],
        "dispatch-request-1"
    );
    let encoded = serde_json::to_string(&value).expect("output JSON");
    assert!(!encoded.contains(PRIVATE_PRICING));
    let top = value.as_object().expect("top-level object");
    let metadata = value["authorization"]
        .as_object()
        .expect("authorization metadata object");
    for private_field in [
        "endpoint",
        "model",
        "pricing_snapshot_sha256",
        "release_control_snapshot_sha256",
        "project_id",
        "project_lane_sha256",
        "destination_sha256",
        "request_body_sha256",
        "request_body_bytes",
        "provider_request_body",
        "prompt",
        "authorization_json",
        "release_requirements",
        "idempotency_key",
    ] {
        assert!(!top.contains_key(private_field));
        assert!(!metadata.contains_key(private_field));
    }
}

#[test]
fn human_output_escapes_terminal_controls_and_reports_passive_verification() {
    let output =
        GroupAgentNodeDispatchAuthorizationCliOutput::verified(verified("node\u{1b}[2J\u{202e}"));
    let mut bytes = Vec::new();
    write_output(&output, false, &mut bytes).expect("write human output");
    let human = String::from_utf8(bytes).expect("UTF-8 output");
    assert!(human.contains(r"node=node\x1b[2J\u{202e}"));
    assert!(human.contains("artifact authorizes a future authority release"));
    assert!(human.contains("dispatch authority not released"));
    assert!(!human.contains('\u{1b}'));
    assert!(!human.contains('\u{202e}'));
    assert!(!human.contains(PRIVATE_PRICING));
}

fn verified(node_id: &str) -> VerifiedGroupAgentNodeDispatchAuthorization {
    VerifiedGroupAgentNodeDispatchAuthorization {
        v: 1,
        authorization_id: "authorization-1".into(),
        authorization_sha256: digest('a'),
        graph_run_id: "graph-run-1".into(),
        release_control_snapshot_sha256: digest('b'),
        contract_id: "contract-1".into(),
        dispatch_request_id: "dispatch-request-1".into(),
        node_id: node_id.into(),
        attempt: 1,
        project_id: "project-1".into(),
        project_lane_sha256: digest('c'),
        destination_sha256: digest('d'),
        pricing_snapshot_sha256: PRIVATE_PRICING.into(),
        request_body_sha256: digest('e'),
        request_body_bytes: 123,
    }
}

fn digest(value: char) -> String {
    std::iter::repeat_n(value, 64).collect()
}

fn false_fields() -> [&'static str; 26] {
    [
        "dispatch_authority_released",
        "consent_obtained",
        "fresh_off_machine_consent_obtained",
        "credential_read",
        "credential_preflight_performed",
        "execution_performed",
        "model_used",
        "provider_constructed",
        "provider_used",
        "network_invoked",
        "network_accessed",
        "project_lane_claimed",
        "graph_advanced",
        "workspace_accessed",
        "tools_used",
        "result_produced",
        "result_persisted",
        "conversation_or_prompt_written",
        "memory_written",
        "writeback_performed",
        "authorization_bytes_included",
        "release_control_included",
        "request_body_included",
        "destination_included",
        "model_included",
        "pricing_included",
    ]
}
