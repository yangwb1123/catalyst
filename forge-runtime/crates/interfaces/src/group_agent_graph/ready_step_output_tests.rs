use super::{
    CoreTrustBoundaryView, GroupAgentGraphRunStatus, GroupAgentNodeTerminalClassification,
    GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledReadyNodeInvocationEffects,
    GroupAgentScheduledReadyNodeOwnerCleanup, RuntimeEffectFactsView,
    ScheduledReadyNodeStepCliOutput, write_output,
};

#[allow(clippy::fn_params_excessive_bools)]
const fn effects(
    preclaim: bool,
    claimed: bool,
    polled: bool,
    receipt: bool,
) -> GroupAgentScheduledReadyNodeInvocationEffects {
    GroupAgentScheduledReadyNodeInvocationEffects {
        preclaim_effects_performed: preclaim,
        project_lane_claimed: claimed,
        provider_stream_polled: polled,
        logical_hub_mutated: claimed,
        terminal_receipt_recorded: receipt,
        result_persisted: claimed,
        owner_sidecar_cleanup: if preclaim {
            GroupAgentScheduledReadyNodeOwnerCleanup::Succeeded
        } else {
            GroupAgentScheduledReadyNodeOwnerCleanup::NotApplicable
        },
    }
}

fn output(result: Option<&str>) -> ScheduledReadyNodeStepCliOutput {
    ScheduledReadyNodeStepCliOutput {
        kind: "group_agent_scheduled_ready_node_step",
        v: 2,
        invocation_disposition: "terminalized",
        metadata_only: result.is_none(),
        result_included: result.is_some(),
        graph_run_id: "graph-run-1".into(),
        provider_request_id: "provider-request-1".into(),
        dispatch_id: "dispatch-1".into(),
        node_id: "node-1".into(),
        attempt: 1,
        graph_status: GroupAgentGraphRunStatus::Completed,
        lifecycle_status: GroupAgentScheduledNodeLifecycleStatus::Terminalized,
        classification: Some(GroupAgentNodeTerminalClassification::Completed),
        provider_poll_started: true,
        terminal_seen: true,
        stream_eof_seen: true,
        lane_active: false,
        retry_authorized: false,
        core_trust_boundary: CoreTrustBoundaryView::validated(true),
        runtime_effect_facts: RuntimeEffectFactsView::new(effects(true, true, true, true), false),
        result_text: result.map(str::to_owned),
    }
}

#[test]
fn default_shape_is_metadata_only_and_has_honest_effect_facts() {
    let value = serde_json::to_value(output(None)).expect("serialize output");
    assert_eq!(value["metadata_only"], true);
    assert_eq!(value["invocation_disposition"], "terminalized");
    assert_eq!(value["result_included"], false);
    assert!(value.get("result_text").is_none());
    let effects = &value["runtime_effect_facts"];
    assert_eq!(effects["provider_stream_polled_this_invocation"], true);
    assert_eq!(
        effects["remote_provider_request_observation"],
        "not_attested"
    );
    assert_eq!(
        effects["automatic_recovery_retry_or_resend_performed"],
        false
    );
    assert_eq!(effects["workspace_accessed"], false);
}

#[test]
fn result_text_requires_explicit_inclusion_and_is_terminal_escaped() {
    let secret = "private result\nsecond line";
    let mut json = Vec::new();
    write_output(&output(Some(secret)), true, &mut json).expect("write JSON");
    let decoded: serde_json::Value = serde_json::from_slice(&json).expect("decode JSON");
    assert_eq!(decoded["result_text"], secret);

    let mut human = Vec::new();
    write_output(&output(Some(secret)), false, &mut human).expect("write human output");
    let human = String::from_utf8(human).expect("UTF-8 output");
    assert!(human.contains("result: private result\\nsecond line"));
    assert!(!human.contains("private result\nsecond line"));
}

#[test]
fn already_claimed_does_not_invent_preclaim_effect_facts() {
    let mut output = output(None);
    output.invocation_disposition = "already_claimed";
    output.runtime_effect_facts =
        RuntimeEffectFactsView::new(effects(false, false, false, false), false);
    let value = serde_json::to_value(output).expect("serialize output");
    let effects = &value["runtime_effect_facts"];
    assert_eq!(effects["credential_read_this_invocation"], false);
    assert_eq!(effects["preclaim_effects_observation"], "not_performed");
    assert_eq!(effects["schema_migration_observation"], "not_observed");
    assert_eq!(effects["provider_constructed_this_invocation"], false);
    assert_eq!(effects["owner_sidecar_created_this_invocation"], false);
    assert_eq!(effects["provider_stream_polled_this_invocation"], false);
    assert_eq!(
        effects["remote_provider_request_observation"],
        "not_attested"
    );
    assert_eq!(
        effects["owner_sidecar_left_active_by_this_invocation"],
        false
    );
    assert_eq!(effects["catchable_signal_cancellation_armed"], true);
}

#[test]
fn claim_race_loss_reports_preclaim_effects_without_claim_or_poll() {
    let mut output = output(None);
    output.invocation_disposition = "claim_race_lost";
    output.runtime_effect_facts =
        RuntimeEffectFactsView::new(effects(true, false, false, false), false);
    let value = serde_json::to_value(output).expect("serialize output");
    let facts = &value["runtime_effect_facts"];
    assert_eq!(facts["preclaim_effects_observation"], "performed");
    assert_eq!(facts["credential_read_this_invocation"], true);
    assert_eq!(facts["owner_sidecar_created_this_invocation"], true);
    assert_eq!(facts["project_lane_claimed_this_invocation"], false);
    assert_eq!(facts["provider_stream_polled_this_invocation"], false);
}

#[test]
fn durable_quarantine_reports_poll_and_write_without_receipt() {
    let mut output = output(None);
    output.invocation_disposition = "quarantined";
    output.lifecycle_status = GroupAgentScheduledNodeLifecycleStatus::Quarantined;
    output.core_trust_boundary = CoreTrustBoundaryView::validated(false);
    output.runtime_effect_facts =
        RuntimeEffectFactsView::new(effects(true, true, true, false), false);
    let value = serde_json::to_value(output).expect("serialize output");
    assert_eq!(value["invocation_disposition"], "quarantined");
    assert_eq!(
        value["core_trust_boundary"]["terminal_protocol_handshake_validated"],
        true
    );
    assert_eq!(
        value["core_trust_boundary"]["stored_terminal_receipt_validated"],
        false
    );
    let facts = &value["runtime_effect_facts"];
    assert_eq!(facts["logical_hub_mutated_this_invocation"], true);
    assert_eq!(facts["terminal_receipt_recorded_this_invocation"], false);
}

#[test]
fn cleanup_failure_preserves_effect_facts_and_reports_unknown_presence() {
    let mut output = output(None);
    output.runtime_effect_facts = RuntimeEffectFactsView::new(
        GroupAgentScheduledReadyNodeInvocationEffects {
            owner_sidecar_cleanup: GroupAgentScheduledReadyNodeOwnerCleanup::Failed,
            ..effects(true, true, true, true)
        },
        false,
    );
    let value = serde_json::to_value(output).expect("serialize output");
    let facts = &value["runtime_effect_facts"];
    assert_eq!(facts["logical_hub_mutated_this_invocation"], true);
    assert_eq!(facts["terminal_receipt_recorded_this_invocation"], true);
    assert_eq!(facts["owner_sidecar_cleanup_observation"], "failed");
    assert!(facts["owner_sidecar_left_active_by_this_invocation"].is_null());
}
