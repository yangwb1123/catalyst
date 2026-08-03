use forge_runtime_application::VerifiedGroupAgentScheduledNodeDispatchAuthorization;

use super::{GroupAgentScheduledNodeDispatchAuthorizationCliOutput, write_output};

#[test]
fn json_separates_decisions_from_false_effects_and_hides_private_metadata() {
    let output = GroupAgentScheduledNodeDispatchAuthorizationCliOutput::verified(fixture());
    let mut bytes = Vec::new();
    write_output(&output, true, &mut bytes).expect("write JSON");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("output JSON");
    assert_eq!(
        value["authorization_decisions"]["dispatch_authority_release_authorized"],
        true
    );
    assert_eq!(value["all_current_effect_facts_false"], true);
    for fact in value["current_effect_facts"].as_object().unwrap().values() {
        assert_eq!(fact, false);
    }
    assert_eq!(
        value["endpoint_model_budget_lane_pricing_or_standalone_digest_included"],
        false
    );
    assert_eq!(
        value["authorization"]["authorization_id"],
        "scheduled-node-dispatch-authorization-safe-content-id"
    );
    let text = String::from_utf8(bytes).unwrap();
    for private in ["digest-secret", "endpoint-secret", "model-secret"] {
        assert!(!text.contains(private));
    }
}

#[test]
fn human_output_sanitizes_identifiers_and_states_the_no_effect_boundary() {
    let mut value = fixture();
    value.node_id = "node\u{1b}[2J\u{202e}".into();
    let output = GroupAgentScheduledNodeDispatchAuthorizationCliOutput::verified(value);
    let mut bytes = Vec::new();
    write_output(&output, false, &mut bytes).expect("write human output");
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains('\u{202e}'));
    assert!(text.contains("authorization decisions"));
    assert!(text.contains("current effect facts: all false"));
    assert!(text.contains("standalone digests remain hidden"));
    assert!(text.contains("content-addressed IDs remain visible"));
}

fn fixture() -> VerifiedGroupAgentScheduledNodeDispatchAuthorization {
    VerifiedGroupAgentScheduledNodeDispatchAuthorization {
        v: 1,
        authorization_id: "scheduled-node-dispatch-authorization-safe-content-id".into(),
        authorization_sha256: "digest-secret".into(),
        graph_run_id: "run-1".into(),
        release_control_snapshot_sha256: "digest-secret".into(),
        schedule_id: "schedule-1".into(),
        scheduled_contract_id: "contract-1".into(),
        scheduled_provider_request_id: "request-1".into(),
        execution_ordinal: 0,
        node_id: "node-1".into(),
        attempt: 1,
        project_id: "project-1".into(),
        project_lane_sha256: "digest-secret".into(),
        destination_sha256: "endpoint-secret".into(),
        pricing_snapshot_sha256: "digest-secret".into(),
        request_body_sha256: "model-secret".into(),
        request_body_bytes: 321,
    }
}
