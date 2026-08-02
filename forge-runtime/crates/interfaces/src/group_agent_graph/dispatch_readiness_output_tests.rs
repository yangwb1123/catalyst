use super::*;
use forge_runtime_application::{
    GroupAgentNodePricingQuote, VerifiedGroupAgentNodeDispatchAuthorization,
    VerifiedGroupAgentNodeDispatchReadiness,
};

#[test]
fn output_is_redacted_and_explicitly_effect_free() {
    let output = GroupAgentNodeDispatchReadinessCliOutput::verified(verified());
    let value = serde_json::to_value(&output).expect("serialize output");
    assert_eq!(value["readiness_validated"], true);
    assert_eq!(value["destination_registered"], true);
    assert_eq!(value["pricing_upper_bound_within_budget"], true);
    assert_eq!(value["vendor_attestation_present"], false);
    for field in [
        "final_effectful_preflight_performed",
        "dispatch_authority_released",
        "consent_obtained",
        "credential_read",
        "credential_preflight_performed",
        "provider_constructed",
        "provider_used",
        "network_accessed",
        "project_lane_claimed",
        "execution_performed",
        "result_produced",
        "result_persisted",
        "graph_advanced",
        "database_written",
        "authorization_bytes_included",
        "pricing_bytes_included",
        "pricing_values_included",
        "destination_included",
        "model_included",
    ] {
        assert_eq!(value[field], false, "{field}");
    }
    let encoded = serde_json::to_string(&value).unwrap();
    for private in [
        "pricing-secret",
        "destination-secret",
        "model-secret",
        "840960",
    ] {
        assert!(!encoded.contains(private));
    }
}

#[test]
fn human_output_escapes_metadata_and_states_the_pricing_limit() {
    let mut value = verified();
    value.authorization.node_id = "node\u{1b}[2J\u{202e}".into();
    let output = GroupAgentNodeDispatchReadinessCliOutput::verified(value);
    let mut bytes = Vec::new();
    write_output(&output, false, &mut bytes).expect("write output");
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains(r"node=node\x1b[2J\u{202e}"));
    assert!(text.contains("operator-asserted, not vendor-attested"));
    assert!(text.contains("readiness only"));
    assert!(!text.contains('\u{1b}'));
}

fn verified() -> VerifiedGroupAgentNodeDispatchReadiness {
    VerifiedGroupAgentNodeDispatchReadiness {
        v: 1,
        authorization: VerifiedGroupAgentNodeDispatchAuthorization {
            v: 1,
            authorization_id: "authorization-1".into(),
            authorization_sha256: digest('a'),
            graph_run_id: "graph-run-1".into(),
            release_control_snapshot_sha256: digest('b'),
            contract_id: "contract-1".into(),
            dispatch_request_id: "dispatch-request-1".into(),
            node_id: "node-1".into(),
            attempt: 1,
            project_id: "project-1".into(),
            project_lane_sha256: digest('c'),
            destination_sha256: "destination-secret".into(),
            pricing_snapshot_sha256: "pricing-secret".into(),
            request_body_sha256: digest('d'),
            request_body_bytes: 50,
        },
        quote: GroupAgentNodePricingQuote {
            pricing_snapshot_sha256: "pricing-secret".into(),
            destination_sha256: "destination-secret".into(),
            max_input_tokens: 400_000,
            max_output_tokens: 4_096,
            max_cost_usd_micros: 840_960,
        },
    }
}

fn digest(value: char) -> String {
    std::iter::repeat_n(value, 64).collect()
}
