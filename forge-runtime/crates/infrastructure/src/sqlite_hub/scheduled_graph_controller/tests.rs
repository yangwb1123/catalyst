use crate::runtime_domain::{
    AppendScheduledGraphControllerDisposition, HubStoreError, ScheduledGraphControllerEvent,
    ScheduledGraphControllerEventPayload, ScheduledGraphControllerExecutionProfile,
    ScheduledGraphControllerHeader, ScheduledGraphControllerStore,
};

use super::super::scheduled_graph_progress::atomicity_fixture::{ReadyFixture, ready_fixture};
use super::write;

mod concurrency;
mod corruption;

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
    .expect("seal controller profile")
}

fn header(
    schedule_id: String,
    schedule_sha256: String,
    graph_run_id: String,
) -> ScheduledGraphControllerHeader {
    ScheduledGraphControllerHeader {
        v: 1,
        controller_protocol_version: 1,
        graph_run_id,
        schedule_id,
        schedule_sha256,
        schedule_version: 1,
        progress_protocol_version: 1,
        core_bin_sha256: "2".repeat(64),
        node_count: 2,
        max_effectful_steps: 2,
        max_total_cost_usd_micros: 20_000,
        execution_profile: profile(),
        created_at_ms: 100,
        controller_id: String::new(),
        controller_sha256: String::new(),
    }
    .seal()
    .expect("seal controller header")
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
    .expect("seal controller event")
}

fn controller_fixture() -> (
    ReadyFixture,
    ScheduledGraphControllerHeader,
    ScheduledGraphControllerEvent,
) {
    let fixture = ready_fixture();
    let schedule = fixture
        .claim_request()
        .release_control
        .schedule_record
        .clone();
    let header = header(
        schedule.schedule_id,
        schedule.schedule_sha256,
        schedule.graph_run_id,
    );
    let started = event(
        &header,
        1,
        None,
        ScheduledGraphControllerEventPayload::Started {
            snapshot_sha256: "3".repeat(64),
            decision_sha256: "4".repeat(64),
        },
    );
    (fixture, header, started)
}

fn start(
    fixture: &ReadyFixture,
    header: &ScheduledGraphControllerHeader,
    started: &ScheduledGraphControllerEvent,
) -> AppendScheduledGraphControllerDisposition {
    fixture
        .graph
        .store
        .start_scheduled_graph_controller(header, started)
        .expect("start controller")
        .disposition
}

#[test]
fn start_append_replay_and_reopen_preserve_exact_controller_journal() {
    let (fixture, header, started) = controller_fixture();
    assert_eq!(
        start(&fixture, &header, &started),
        AppendScheduledGraphControllerDisposition::Stored
    );
    assert_eq!(
        start(&fixture, &header, &started),
        AppendScheduledGraphControllerDisposition::Replayed
    );

    let awaiting = event(
        &header,
        2,
        Some(started.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent {
            execution_ordinal: 0,
            node_id: "frontend".into(),
            provider_request_id: "scheduled-provider-request-controller".into(),
            authorization_sha256: "5".repeat(64),
            snapshot_sha256: "3".repeat(64),
            decision_sha256: "4".repeat(64),
            predecessor_content_included: false,
        },
    );
    let appended = fixture
        .graph
        .store
        .append_scheduled_graph_controller_event(&awaiting)
        .expect("append controller event");
    assert_eq!(appended.journal.events.len(), 2);
    assert_eq!(
        fixture
            .graph
            .store
            .inspect_scheduled_graph_controller(&header.graph_run_id)
            .expect("reopen controller journal"),
        appended.journal
    );
}

#[test]
fn competing_event_for_the_same_sequence_is_rejected_without_rewrite() {
    let (fixture, header, started) = controller_fixture();
    start(&fixture, &header, &started);
    let first = event(
        &header,
        2,
        Some(started.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::Completed {
            snapshot_sha256: "5".repeat(64),
            decision_sha256: "6".repeat(64),
        },
    );
    fixture
        .graph
        .store
        .append_scheduled_graph_controller_event(&first)
        .expect("append winner");
    let competing = event(
        &header,
        2,
        Some(started.event_sha256),
        ScheduledGraphControllerEventPayload::Completed {
            snapshot_sha256: "7".repeat(64),
            decision_sha256: "8".repeat(64),
        },
    );
    assert!(matches!(
        fixture
            .graph
            .store
            .append_scheduled_graph_controller_event(&competing),
        Err(HubStoreError::Conflict { .. })
    ));
    assert_eq!(
        fixture
            .graph
            .store
            .inspect_scheduled_graph_controller(&header.graph_run_id)
            .expect("winner remains")
            .head(),
        &first
    );
}

#[test]
fn same_graph_run_replays_exact_start_and_rejects_a_divergent_controller() {
    let (fixture, header, started) = controller_fixture();
    assert_eq!(
        start(&fixture, &header, &started),
        AppendScheduledGraphControllerDisposition::Stored
    );
    assert_eq!(
        start(&fixture, &header, &started),
        AppendScheduledGraphControllerDisposition::Replayed
    );
    let divergent_header = divergent_header(&header);
    let divergent_started = event(
        &divergent_header,
        1,
        None,
        ScheduledGraphControllerEventPayload::Started {
            snapshot_sha256: "3".repeat(64),
            decision_sha256: "4".repeat(64),
        },
    );
    assert_ne!(divergent_header.controller_id, header.controller_id);
    assert!(matches!(
        fixture
            .graph
            .store
            .start_scheduled_graph_controller(&divergent_header, &divergent_started),
        Err(HubStoreError::Conflict { .. })
    ));
    let durable = fixture
        .graph
        .store
        .inspect_scheduled_graph_controller(&header.graph_run_id)
        .expect("inspect original controller");
    assert_eq!(durable.header, header);
    assert_eq!(durable.events, vec![started]);
}

#[test]
fn started_insert_failure_rolls_header_and_event_tables_back_together() {
    let (fixture, header, started) = controller_fixture();
    let mut connection = fixture.graph.connection();
    connection
        .execute_batch(
            "CREATE TEMP TRIGGER force_controller_started_failure
             BEFORE INSERT ON group_agent_scheduled_graph_controller_events
             WHEN NEW.sequence=1
             BEGIN SELECT RAISE(ABORT,'forced Started failure'); END;",
        )
        .expect("install connection-local Started failure");

    write::start(&mut connection, &header, &started)
        .expect_err("Started insert failure must roll the transaction back");
    assert_eq!(
        row_count(&connection, "group_agent_scheduled_graph_controllers"),
        0
    );
    assert_eq!(
        row_count(&connection, "group_agent_scheduled_graph_controller_events"),
        0
    );
}

fn divergent_header(header: &ScheduledGraphControllerHeader) -> ScheduledGraphControllerHeader {
    let mut divergent = header.clone();
    divergent.core_bin_sha256 = "9".repeat(64);
    divergent.controller_id.clear();
    divergent.controller_sha256.clear();
    divergent.seal().expect("seal divergent controller header")
}

fn row_count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("controller table row count")
}
