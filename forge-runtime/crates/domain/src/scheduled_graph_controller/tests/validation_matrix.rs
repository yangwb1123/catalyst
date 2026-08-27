use super::*;

#[test]
fn every_event_payload_shape_seals_with_bounded_metadata() {
    let header = header();
    for (index, payload) in representative_payloads(&header).into_iter().enumerate() {
        event(&header, index + 1, None, payload);
    }
}

#[test]
fn transition_families_accept_only_their_bound_successors() {
    let header = header();
    let payloads = representative_payloads(&header);
    let started = &payloads[0];
    assert!(super::super::transitions::allowed_transition(
        started,
        &payloads[1]
    ));
    assert!(!super::super::transitions::allowed_transition(
        started,
        &payloads[2]
    ));
    assert!(super::super::transitions::allowed_transition(
        &payloads[1],
        &payloads[2]
    ));
    assert!(super::super::transitions::allowed_transition(
        &payloads[3],
        &payloads[4]
    ));
    assert!(super::super::transitions::allowed_transition(
        &payloads[5],
        &payloads[6]
    ));
    assert!(super::super::transitions::allowed_transition(
        &payloads[6],
        &payloads[8]
    ));
    assert!(!super::super::transitions::allowed_transition(
        &payloads[9],
        started
    ));
    assert!(!super::super::transitions::allowed_transition(
        &payloads[10],
        started
    ));
}

#[test]
fn only_a_retryable_preclaim_failure_can_reopen_fresh_consent() {
    let dispatch = dispatch_payload();
    let retryable = ScheduledGraphControllerEventPayload::RetryablePreclaimFailure {
        execution_ordinal: 0,
        node_id: "node-a".into(),
        provider_request_id: "request-a".into(),
        reason: ScheduledGraphControllerRetryableFailure::ProviderUnavailable,
    };
    let awaiting = awaiting_payload();

    assert!(!super::super::transitions::allowed_transition(
        &dispatch, &awaiting
    ));
    assert!(super::super::transitions::allowed_transition(
        &dispatch, &retryable
    ));
    assert!(super::super::transitions::allowed_transition(
        &retryable, &awaiting
    ));
}

#[test]
fn planned_idempotency_keys_are_bound_to_controller_and_ordinal() {
    for (prepare, suffix) in [(false, "materialize"), (true, "prepare")] {
        let header = header();
        let started = started_event(&header);
        let exact = format!("controller-{}-{suffix}-0", header.controller_sha256);
        let valid = planned_event(&header, &started, prepare, exact);
        ScheduledGraphControllerJournal {
            header: header.clone(),
            events: vec![started.clone(), valid],
        }
        .validate()
        .expect("exact planned key validates");
        let wrong = planned_event(&header, &started, prepare, "controller-wrong-0".into());
        assert!(
            ScheduledGraphControllerJournal {
                header,
                events: vec![started, wrong],
            }
            .validate()
            .is_err()
        );
    }
}

#[test]
fn timestamps_counts_and_codec_byte_bounds_fail_closed() {
    let header = header();
    let started = started_event(&header);
    let mut regressed = awaiting_event(&header, &started, false);
    regressed.created_at_ms = started.created_at_ms - 1;
    regressed.event_sha256.clear();
    let regressed = regressed.seal().expect("shape-valid regressed event");
    assert!(
        ScheduledGraphControllerJournal {
            header: header.clone(),
            events: vec![started.clone(), regressed],
        }
        .validate()
        .is_err()
    );

    let mut unrepresentable = started.clone();
    unrepresentable.created_at_ms = u64::MAX;
    unrepresentable.event_sha256.clear();
    assert!(unrepresentable.seal().is_err());
    assert!(
        ScheduledGraphControllerJournal {
            header,
            events: vec![started; MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS + 1],
        }
        .validate()
        .is_err()
    );
    assert!(
        ScheduledGraphControllerEvent::decode_exact(
            &" ".repeat(MAX_SCHEDULED_GRAPH_CONTROLLER_EVENT_BYTES + 1)
        )
        .is_err()
    );
}

fn planned_event(
    header: &ScheduledGraphControllerHeader,
    started: &ScheduledGraphControllerEvent,
    prepare: bool,
    idempotency_key: String,
) -> ScheduledGraphControllerEvent {
    let payload = if prepare {
        ScheduledGraphControllerEventPayload::PreparePlanned {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            contract_id: "contract-a".into(),
            idempotency_key,
        }
    } else {
        ScheduledGraphControllerEventPayload::MaterializePlanned {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
            idempotency_key,
        }
    };
    event(header, 2, Some(started.event_sha256.clone()), payload)
}

fn representative_payloads(
    header: &ScheduledGraphControllerHeader,
) -> Vec<ScheduledGraphControllerEventPayload> {
    let mut payloads = passive_payloads(header);
    payloads.extend(effect_payloads());
    payloads
}

fn passive_payloads(
    header: &ScheduledGraphControllerHeader,
) -> Vec<ScheduledGraphControllerEventPayload> {
    use ScheduledGraphControllerEventPayload as Payload;
    vec![
        Payload::Started {
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
        },
        Payload::MaterializePlanned {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
            idempotency_key: format!("controller-{}-materialize-0", header.controller_sha256),
        },
        Payload::MaterializeObserved {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            contract_id: "contract-a".into(),
        },
        Payload::PreparePlanned {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            contract_id: "contract-a".into(),
            idempotency_key: format!("controller-{}-prepare-0", header.controller_sha256),
        },
        Payload::PrepareObserved {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            contract_id: "contract-a".into(),
            provider_request_id: "request-a".into(),
        },
    ]
}

fn effect_payloads() -> Vec<ScheduledGraphControllerEventPayload> {
    use ScheduledGraphControllerEventPayload as Payload;
    vec![
        awaiting_payload(),
        dispatch_payload(),
        Payload::RetryablePreclaimFailure {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            reason: ScheduledGraphControllerRetryableFailure::ProviderUnavailable,
        },
        Payload::NodeCompleted {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            terminal_receipt_sha256: "7".repeat(64),
        },
        Payload::Stopped {
            reason: ScheduledGraphControllerStopReason::IncompatibleSchedule,
            provider_request_id: None,
            snapshot_sha256: None,
            decision_sha256: None,
        },
        Payload::Completed {
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
        },
    ]
}

fn awaiting_payload() -> ScheduledGraphControllerEventPayload {
    ScheduledGraphControllerEventPayload::AwaitingFreshConsent {
        execution_ordinal: 0,
        node_id: "node-a".into(),
        provider_request_id: "request-a".into(),
        authorization_sha256: "6".repeat(64),
        snapshot_sha256: "4".repeat(64),
        decision_sha256: "5".repeat(64),
        predecessor_content_included: false,
    }
}

fn dispatch_payload() -> ScheduledGraphControllerEventPayload {
    ScheduledGraphControllerEventPayload::DispatchPlanned {
        execution_ordinal: 0,
        node_id: "node-a".into(),
        provider_request_id: "request-a".into(),
        authorization_sha256: "6".repeat(64),
        snapshot_sha256: "4".repeat(64),
        decision_sha256: "5".repeat(64),
        effectful_step_reservation: 1,
        reserved_cost_usd_micros: 10_000,
        off_machine_consent_observed: true,
        predecessor_content_consent_observed: false,
    }
}
