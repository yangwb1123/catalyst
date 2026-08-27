use super::{
    ScheduledGraphControllerCliOutput, ScheduledGraphControllerOutput,
    ScheduledGraphControllerState, write_output,
};
use crate::runtime_domain::{
    ScheduledGraphControllerEvent, ScheduledGraphControllerEventPayload,
    ScheduledGraphControllerExecutionProfile, ScheduledGraphControllerHeader,
    ScheduledGraphControllerJournal,
};

fn profile() -> ScheduledGraphControllerExecutionProfile {
    ScheduledGraphControllerExecutionProfile {
        endpoint: "https://api.openai.com/v1/responses".into(),
        model: "private-model".into(),
        max_output_tokens: 16,
        max_model_output_bytes: 4096,
        max_model_events: 16,
        timeout_ms: 1_000,
        max_cost_usd_micros: 10_000,
        pricing_snapshot_sha256: "1".repeat(64),
        max_result_bytes: 4096,
        profile_sha256: String::new(),
    }
    .seal()
    .expect("sealed controller profile")
}

fn header() -> ScheduledGraphControllerHeader {
    ScheduledGraphControllerHeader {
        v: 1,
        controller_protocol_version: 1,
        graph_run_id: "graph-run-controller-output".into(),
        schedule_id: format!("graph-execution-schedule-{}", "2".repeat(64)),
        schedule_sha256: "2".repeat(64),
        schedule_version: 1,
        progress_protocol_version: 1,
        core_bin_sha256: "3".repeat(64),
        node_count: 2,
        max_effectful_steps: 2,
        max_total_cost_usd_micros: 20_000,
        execution_profile: profile(),
        created_at_ms: 100,
        controller_id: String::new(),
        controller_sha256: String::new(),
    }
    .seal()
    .expect("sealed controller header")
}

fn event(
    header: &ScheduledGraphControllerHeader,
    sequence: usize,
    previous: Option<String>,
    payload: ScheduledGraphControllerEventPayload,
) -> ScheduledGraphControllerEvent {
    ScheduledGraphControllerEvent {
        v: 1,
        controller_id: header.controller_id.clone(),
        graph_run_id: header.graph_run_id.clone(),
        sequence,
        previous_event_sha256: previous,
        payload,
        created_at_ms: 100 + sequence as u64,
        event_sha256: String::new(),
    }
    .seal()
    .expect("sealed controller event")
}

fn awaiting_output() -> ScheduledGraphControllerOutput {
    let header = header();
    let started = event(
        &header,
        1,
        None,
        ScheduledGraphControllerEventPayload::Started {
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
        },
    );
    let awaiting = awaiting_event(&header, &started.event_sha256);
    let state = awaiting_state(awaiting.event_sha256.clone());
    let journal = ScheduledGraphControllerJournal {
        header,
        events: vec![started, awaiting],
    };
    journal.validate().expect("valid awaiting journal");
    ScheduledGraphControllerOutput {
        journal,
        state,
        invocation: None,
        retryable_failure: None,
        post_invocation_error: None,
        journal_current_observed: true,
    }
}

fn awaiting_event(
    header: &ScheduledGraphControllerHeader,
    previous: &str,
) -> ScheduledGraphControllerEvent {
    event(
        header,
        2,
        Some(previous.into()),
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "provider-request-exact".into(),
            authorization_sha256: "6".repeat(64),
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
            predecessor_content_included: true,
        },
    )
}

fn awaiting_state(awaiting_event_sha256: String) -> ScheduledGraphControllerState {
    ScheduledGraphControllerState::AwaitingFreshConsent {
        awaiting_event_sha256,
        execution_ordinal: 0,
        node_id: "node-a".into(),
        provider_request_id: "provider-request-exact".into(),
        authorization_sha256: "6".repeat(64),
        snapshot_sha256: "4".repeat(64),
        decision_sha256: "5".repeat(64),
        predecessor_content_included: true,
    }
}

#[test]
fn passive_output_is_metadata_only_and_preserves_exact_consent_anchors() {
    let raw = awaiting_output();
    let awaiting_event_sha256 = raw.journal.head().event_sha256.clone();
    let output = ScheduledGraphControllerCliOutput::new(raw, false, false, true);
    let value = serde_json::to_value(output).expect("serialize controller output");
    assert_eq!(value["type"], "scheduled_graph_controller");
    assert_eq!(value["metadata_only"], true);
    assert!(value["invocation"].is_null());
    assert_eq!(value["state"], "awaiting_fresh_consent");
    let awaiting = &value["awaiting_fresh_consent"];
    assert_eq!(awaiting["awaiting_event_sha256"], awaiting_event_sha256);
    assert_eq!(
        awaiting["awaiting_event_sha256"],
        value["head_event_sha256"]
    );
    assert_eq!(awaiting["provider_request_id"], "provider-request-exact");
    assert_eq!(awaiting["authorization_sha256"], "6".repeat(64));
    assert_eq!(awaiting["snapshot_sha256"], "4".repeat(64));
    assert_eq!(awaiting["decision_sha256"], "5".repeat(64));
    assert_eq!(awaiting["predecessor_content_included"], true);
}

#[test]
fn metadata_output_exposes_budgets_but_not_profile_journal_or_result() {
    let output = ScheduledGraphControllerCliOutput::new(awaiting_output(), false, false, true);
    let value = serde_json::to_value(&output).expect("serialize controller output");
    assert_eq!(value["effectful_steps_reserved"], 0);
    assert_eq!(value["max_effectful_steps"], 2);
    assert_eq!(value["cost_usd_micros_reserved"], 0);
    assert_eq!(value["max_total_cost_usd_micros"], 20_000);
    for excluded in ["execution_profile", "journal", "events", "result_text"] {
        assert!(value.get(excluded).is_none(), "{excluded}");
    }
    let encoded = serde_json::to_string(&output).expect("encode controller output");
    for private in ["private-model", &"1".repeat(64)] {
        assert!(!encoded.contains(private), "leaked {private}");
    }
}

#[test]
fn show_output_exposes_the_bounded_redacted_event_chain() {
    let output = ScheduledGraphControllerCliOutput::from_show(awaiting_output());
    let value = serde_json::to_value(&output).expect("serialize show output");
    assert_eq!(value["events"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["events"][0]["payload"]["kind"], "started");
    let encoded = serde_json::to_string(&output).expect("encode show output");
    for private in ["private-model", &"1".repeat(64)] {
        assert!(!encoded.contains(private), "leaked {private}");
    }
}

#[test]
fn metadata_output_does_not_claim_containment_retry_or_effect_attestation() {
    let output = ScheduledGraphControllerCliOutput::new(awaiting_output(), false, false, true);
    let value = serde_json::to_value(output).expect("serialize controller output");
    assert_eq!(value["automatic_retry_or_resend_performed"], false);
    let trust = &value["core_trust_boundary"];
    assert_eq!(trust["same_user_code"], true);
    assert_eq!(trust["operator_trust_required"], true);
    assert_eq!(trust["effect_containment_enforced"], false);
    assert_eq!(trust["effect_attestation_present"], false);
    assert_eq!(trust["empty_environment"], true);
    assert_eq!(trust["filesystem_isolation_enforced"], false);
    assert_eq!(trust["network_isolation_enforced"], false);
}

#[test]
fn read_only_output_does_not_claim_unobserved_core_validation() {
    let output = ScheduledGraphControllerCliOutput::new(awaiting_output(), false, false, false);
    let value = serde_json::to_value(output).expect("serialize read-only controller output");
    let trust = &value["core_trust_boundary"];
    for field in [
        "binary_identity_validated",
        "reconcile_handshake_validated",
        "materialization_handshake_validated",
        "ready_release_handshake_validated",
        "terminal_protocol_handshake_validated",
    ] {
        assert_eq!(trust[field], false, "{field}");
    }
}

#[test]
fn human_metadata_output_contains_only_public_identity_and_anchors() {
    let output = ScheduledGraphControllerCliOutput::new(awaiting_output(), false, false, true);
    let mut bytes = Vec::new();
    write_output(&output, false, &mut bytes).expect("write human controller output");
    let human = String::from_utf8(bytes).expect("UTF-8 human output");
    assert!(human.contains("provider-request-exact"));
    assert!(human.contains(&"6".repeat(64)));
    for private in ["private-model", &"1".repeat(64)] {
        assert!(!human.contains(private), "leaked {private}");
    }
}
