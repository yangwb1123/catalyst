use super::*;

#[path = "tests/validation_matrix.rs"]
mod validation_matrix;

fn profile() -> ScheduledGraphControllerExecutionProfile {
    ScheduledGraphControllerExecutionProfile {
        endpoint: "https://api.openai.com/v1/responses".into(),
        model: "gpt-test".into(),
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
    .expect("sealed profile")
}

fn header() -> ScheduledGraphControllerHeader {
    ScheduledGraphControllerHeader {
        v: 1,
        controller_protocol_version: 1,
        graph_run_id: "graph-run-controller".into(),
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
    .expect("sealed header")
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
    .expect("sealed event")
}

#[test]
fn exact_controller_journal_round_trips_and_counts_reserved_steps() {
    let (header, journal) = reserved_journal(1);
    journal.validate().expect("valid controller journal");
    assert_eq!(journal.effectful_steps_reserved(), 1);
    assert_eq!(journal.cost_usd_micros_reserved(), 10_000);
    assert!(!journal.is_terminal());
    assert_eq!(
        ScheduledGraphControllerHeader::decode_exact(&header.canonical_json().unwrap()).unwrap(),
        header
    );
}

#[test]
fn journal_rejects_reused_or_skipped_effectful_reservation() {
    let (_, journal) = reserved_journal(2);
    assert!(journal.validate().is_err());
}

#[test]
fn profile_rejects_an_unregistered_or_secret_bearing_endpoint() {
    let mut value = profile();
    value.endpoint = "https://private.example/v1/responses?token=secret".into();
    value.profile_sha256.clear();
    assert!(value.seal().is_err());
}

#[test]
fn late_completion_repairs_an_awaiting_head_only_with_a_pending_dispatch() {
    let (_, mut journal) = reserved_journal(1);
    let dispatch = journal.head().clone();
    let retryable = event(
        &journal.header,
        4,
        Some(dispatch.event_sha256),
        ScheduledGraphControllerEventPayload::RetryablePreclaimFailure {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            reason: ScheduledGraphControllerRetryableFailure::ProviderUnavailable,
        },
    );
    let awaiting = event(
        &journal.header,
        5,
        Some(retryable.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            authorization_sha256: "6".repeat(64),
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
            predecessor_content_included: false,
        },
    );
    let completed = event(
        &journal.header,
        6,
        Some(awaiting.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::NodeCompleted {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            terminal_receipt_sha256: "7".repeat(64),
        },
    );
    journal.events.extend([retryable, awaiting, completed]);
    journal.validate().expect("late lifecycle repair is valid");
}

#[test]
fn completion_without_a_pending_dispatch_is_rejected() {
    let (_, mut journal) = reserved_journal(1);
    journal.events.pop();
    let awaiting = journal.head().clone();
    journal.events.push(event(
        &journal.header,
        3,
        Some(awaiting.event_sha256),
        ScheduledGraphControllerEventPayload::NodeCompleted {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            terminal_receipt_sha256: "7".repeat(64),
        },
    ));
    assert!(journal.validate().is_err());
}

#[test]
fn graph_completion_requires_controller_completion_of_the_last_node() {
    let (_, mut journal) = reserved_journal(1);
    let dispatch = journal.head().clone();
    let node_completed = event(
        &journal.header,
        4,
        Some(dispatch.event_sha256),
        ScheduledGraphControllerEventPayload::NodeCompleted {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            terminal_receipt_sha256: "7".repeat(64),
        },
    );
    let completed = event(
        &journal.header,
        5,
        Some(node_completed.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::Completed {
            snapshot_sha256: "8".repeat(64),
            decision_sha256: "9".repeat(64),
        },
    );
    journal.events.extend([node_completed, completed]);
    assert!(journal.validate().is_err());
}

#[test]
fn unanchored_lifecycle_stop_requires_its_pending_dispatch() {
    let header = header();
    let started = started_event(&header);
    let stopped = event(
        &header,
        2,
        Some(started.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::Stopped {
            reason: ScheduledGraphControllerStopReason::Failed,
            provider_request_id: Some("request-a".into()),
            snapshot_sha256: None,
            decision_sha256: None,
        },
    );
    let journal = ScheduledGraphControllerJournal {
        header,
        events: vec![started, stopped],
    };
    assert!(journal.validate().is_err());
}

#[test]
fn predecessor_content_requires_an_independent_dispatch_consent_fact() {
    let header = header();
    let started = started_event(&header);
    let awaiting = awaiting_event(&header, &started, true);
    let denied = dispatch_event(&header, &awaiting, 1, false);
    let mut journal = ScheduledGraphControllerJournal {
        header: header.clone(),
        events: vec![started.clone(), awaiting.clone(), denied],
    };
    assert!(journal.validate().is_err());

    journal.events[2] = dispatch_event(&header, &awaiting, 1, true);
    journal
        .validate()
        .expect("independent predecessor-content consent is valid");
}

fn reserved_journal(
    reservation: u16,
) -> (
    ScheduledGraphControllerHeader,
    ScheduledGraphControllerJournal,
) {
    let header = header();
    let started = started_event(&header);
    let awaiting = awaiting_event(&header, &started, false);
    let dispatch = dispatch_event(&header, &awaiting, reservation, false);
    let journal = ScheduledGraphControllerJournal {
        header: header.clone(),
        events: vec![started, awaiting, dispatch],
    };
    (header, journal)
}

fn started_event(header: &ScheduledGraphControllerHeader) -> ScheduledGraphControllerEvent {
    event(
        header,
        1,
        None,
        ScheduledGraphControllerEventPayload::Started {
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
        },
    )
}

fn awaiting_event(
    header: &ScheduledGraphControllerHeader,
    started: &ScheduledGraphControllerEvent,
    predecessor_content_included: bool,
) -> ScheduledGraphControllerEvent {
    event(
        header,
        2,
        Some(started.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            authorization_sha256: "6".repeat(64),
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
            predecessor_content_included,
        },
    )
}

fn dispatch_event(
    header: &ScheduledGraphControllerHeader,
    awaiting: &ScheduledGraphControllerEvent,
    reservation: u16,
    predecessor_content_consent_observed: bool,
) -> ScheduledGraphControllerEvent {
    event(
        header,
        3,
        Some(awaiting.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::DispatchPlanned {
            execution_ordinal: 0,
            node_id: "node-a".into(),
            provider_request_id: "request-a".into(),
            authorization_sha256: "6".repeat(64),
            snapshot_sha256: "4".repeat(64),
            decision_sha256: "5".repeat(64),
            effectful_step_reservation: reservation,
            reserved_cost_usd_micros: 10_000,
            off_machine_consent_observed: true,
            predecessor_content_consent_observed,
        },
    )
}
