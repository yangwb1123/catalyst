use super::{
    GroupAgentScheduledNodeDispatchReadinessCliOutput, ScheduledReadinessMetadataView, write_output,
};

#[test]
fn json_is_redacted_and_explicitly_effect_free() {
    let output = GroupAgentScheduledNodeDispatchReadinessCliOutput::verified(1, fixture());
    let value = serde_json::to_value(output).expect("serialize output");
    assert_eq!(value["readiness_validated_against_current_state"], true);
    assert_eq!(value["exact_registered_destination_validated"], true);
    assert_eq!(value["pricing_upper_bound_within_frozen_budget"], true);
    assert_eq!(value["pricing_provenance"], "operator_asserted");
    assert_eq!(value["vendor_attestation_present"], false);
    assert_eq!(value["authorization_decisions_are_future_only"], true);
    assert_eq!(value["all_current_effect_facts_false"], true);
    for decision in value["authorization_decisions"]
        .as_object()
        .unwrap()
        .values()
    {
        assert_eq!(decision, true);
    }
    for field in false_fields() {
        assert_eq!(value[field], false, "{field}");
    }
    let encoded = serde_json::to_string(&value).unwrap();
    for private_field in [
        "pricing_snapshot_sha256",
        "destination_sha256",
        "max_cost_usd_micros",
    ] {
        assert!(!encoded.contains(private_field));
    }
}

#[test]
fn human_output_sanitizes_ids_and_states_the_readiness_boundary() {
    let mut value = fixture();
    value.node_id = "node\u{1b}[2J\u{202e}".into();
    let output = GroupAgentScheduledNodeDispatchReadinessCliOutput::verified(1, value);
    let mut bytes = Vec::new();
    write_output(&output, false, &mut bytes).expect("write output");
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains(r"node=node\x1b[2J\u{202e}"));
    assert!(text.contains("operator-asserted, not vendor-attested"));
    assert!(text.contains("readiness only"));
    assert!(text.contains("standalone digests remain hidden"));
    assert!(!text.contains('\u{1b}'));
}

fn false_fields() -> &'static [&'static str] {
    &[
        "final_effectful_preflight_performed",
        "lifecycle_contract_admitted",
        "execution_authority_released",
        "dispatch_authority_released",
        "fresh_off_machine_consent_obtained",
        "credential_read",
        "credential_preflight_performed",
        "provider_constructed",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "project_lane_claimed",
        "provider_request_sent",
        "execution_performed",
        "progress_observed",
        "terminal_receipt_recorded",
        "successor_advance_authorized",
        "result_produced_or_persisted",
        "database_written",
        "conversation_prompt_or_memory_written",
        "writeback_performed",
        "authorization_bytes_included",
        "pricing_bytes_included",
        "pricing_values_included",
        "endpoint_model_budget_lane_or_standalone_digest_included",
    ]
}

fn fixture() -> ScheduledReadinessMetadataView {
    ScheduledReadinessMetadataView {
        authorization_id: "scheduled-authorization-1".into(),
        graph_run_id: "graph-run-1".into(),
        schedule_id: "schedule-1".into(),
        scheduled_contract_id: "contract-1".into(),
        scheduled_provider_request_id: "request-1".into(),
        execution_ordinal: 0,
        node_id: "node-1".into(),
        attempt: 1,
    }
}
