use rusqlite::Connection;

use crate::{
    SqliteHubStore,
    runtime_domain::{
        HubStoreError, ScheduledGraphControllerEvent, ScheduledGraphControllerEventPayload,
        ScheduledGraphControllerHeader, ScheduledGraphControllerStore,
    },
};

use super::super::super::scheduled_graph_progress::atomicity_fixture::ReadyFixture;
use super::{controller_fixture, event, start};

const TAMPERED_GRAPH_RUN_ID: &str = "tampered-graph-run";

const HEADER_CORRUPTIONS: &[(&str, &str, Option<&str>)] = &[
    (
        "controller_id",
        "UPDATE group_agent_scheduled_graph_controllers SET controller_id='tampered-controller'",
        None,
    ),
    (
        "graph_run_id",
        "UPDATE group_agent_scheduled_graph_controllers SET graph_run_id='tampered-graph-run'",
        Some(TAMPERED_GRAPH_RUN_ID),
    ),
    (
        "schedule_id",
        "UPDATE group_agent_scheduled_graph_controllers SET schedule_id='tampered-schedule'",
        None,
    ),
    (
        "version",
        "UPDATE group_agent_scheduled_graph_controllers SET version=2",
        None,
    ),
    (
        "controller_protocol_version",
        "UPDATE group_agent_scheduled_graph_controllers SET controller_protocol_version=2",
        None,
    ),
    (
        "schedule_version",
        "UPDATE group_agent_scheduled_graph_controllers SET schedule_version=2",
        None,
    ),
    (
        "progress_protocol_version",
        "UPDATE group_agent_scheduled_graph_controllers SET progress_protocol_version=2",
        None,
    ),
    (
        "schedule_sha256",
        "UPDATE group_agent_scheduled_graph_controllers SET schedule_sha256=zeroblob(32)",
        None,
    ),
    (
        "core_bin_sha256",
        "UPDATE group_agent_scheduled_graph_controllers SET core_bin_sha256=zeroblob(32)",
        None,
    ),
    (
        "execution_profile_sha256",
        "UPDATE group_agent_scheduled_graph_controllers SET execution_profile_sha256=zeroblob(32)",
        None,
    ),
    (
        "node_count",
        "UPDATE group_agent_scheduled_graph_controllers SET node_count=1",
        None,
    ),
    (
        "max_effectful_steps",
        "UPDATE group_agent_scheduled_graph_controllers SET max_effectful_steps=1",
        None,
    ),
    (
        "max_total_cost_usd_micros",
        "UPDATE group_agent_scheduled_graph_controllers SET max_total_cost_usd_micros=20001",
        None,
    ),
    (
        "controller_sha256",
        "UPDATE group_agent_scheduled_graph_controllers SET controller_sha256=zeroblob(32)",
        None,
    ),
    (
        "header_blob",
        "UPDATE group_agent_scheduled_graph_controllers SET header_blob=CAST('{}' AS BLOB)",
        None,
    ),
    (
        "created_at_ms",
        "UPDATE group_agent_scheduled_graph_controllers SET created_at_ms=101",
        None,
    ),
];

const EVENT_CORRUPTIONS: &[(&str, &str)] = &[
    (
        "controller_id",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET controller_id='tampered-controller' WHERE sequence=1",
    ),
    (
        "sequence",
        "UPDATE group_agent_scheduled_graph_controller_events SET sequence=4 WHERE sequence=3",
    ),
    (
        "previous_event_sha256 altered",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET previous_event_sha256=zeroblob(32) WHERE sequence=3",
    ),
    (
        "previous_event_sha256 missing after first event",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET previous_event_sha256=NULL WHERE sequence=3",
    ),
    (
        "previous_event_sha256 present on first event",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET previous_event_sha256=zeroblob(32) WHERE sequence=1",
    ),
    (
        "event_sha256",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET event_sha256=zeroblob(32) WHERE sequence=3",
    ),
    (
        "event_kind",
        "UPDATE group_agent_scheduled_graph_controller_events SET event_kind='completed' \
         WHERE sequence=3",
    ),
    (
        "effectful_step_reservation altered for dispatch",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET effectful_step_reservation=2 WHERE sequence=3",
    ),
    (
        "effectful_step_reservation missing for dispatch",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET effectful_step_reservation=NULL WHERE sequence=3",
    ),
    (
        "effectful_step_reservation present for non-dispatch",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET effectful_step_reservation=1 WHERE sequence=1",
    ),
    (
        "reserved_cost_usd_micros altered for dispatch",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET reserved_cost_usd_micros=10001 WHERE sequence=3",
    ),
    (
        "reserved_cost_usd_micros missing for dispatch",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET reserved_cost_usd_micros=NULL WHERE sequence=3",
    ),
    (
        "reserved_cost_usd_micros present for non-dispatch",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET reserved_cost_usd_micros=1 WHERE sequence=1",
    ),
    (
        "event_blob",
        "UPDATE group_agent_scheduled_graph_controller_events \
         SET event_blob=CAST('{}' AS BLOB) WHERE sequence=3",
    ),
    (
        "created_at_ms",
        "UPDATE group_agent_scheduled_graph_controller_events SET created_at_ms=999 \
         WHERE sequence=3",
    ),
    (
        "fresh off-machine consent in event_blob",
        r#"UPDATE group_agent_scheduled_graph_controller_events
           SET event_blob=CAST(replace(CAST(event_blob AS TEXT),
             '"off_machine_consent_observed":true',
             '"off_machine_consent_observed":false') AS BLOB)
           WHERE sequence=3"#,
    ),
];

#[test]
fn corruption_matrices_name_every_persisted_column() {
    let (fixture, _, _) = controller_fixture();
    let connection = fixture.graph.connection();
    let header_labels = HEADER_CORRUPTIONS
        .iter()
        .map(|(label, _, _)| *label)
        .collect::<Vec<_>>();
    let event_labels = EVENT_CORRUPTIONS
        .iter()
        .map(|(label, _)| *label)
        .collect::<Vec<_>>();
    assert_matrix_covers_table(
        &connection,
        "group_agent_scheduled_graph_controllers",
        &header_labels,
    );
    assert_matrix_covers_table(
        &connection,
        "group_agent_scheduled_graph_controller_events",
        &event_labels,
    );
}

#[test]
fn persisted_header_corruption_matrix_fails_closed_as_corrupt() {
    for &(label, sql, lookup_graph_run_id) in HEADER_CORRUPTIONS {
        let (fixture, header, started) = controller_fixture();
        start(&fixture, &header, &started);
        mutate(&fixture.graph.connection(), sql, label);
        assert_corrupt(
            &fixture.graph.store,
            lookup_graph_run_id.unwrap_or(&header.graph_run_id),
            label,
        );
    }
}

#[test]
fn persisted_event_corruption_matrix_fails_closed_as_corrupt() {
    for &(label, sql) in EVENT_CORRUPTIONS {
        let (fixture, header, dispatch) = dispatch_fixture();
        assert_eq!(dispatch.sequence, 3);
        mutate(&fixture.graph.connection(), sql, label);
        assert_corrupt(&fixture.graph.store, &header.graph_run_id, label);
    }
}

fn dispatch_fixture() -> (
    ReadyFixture,
    ScheduledGraphControllerHeader,
    ScheduledGraphControllerEvent,
) {
    let (fixture, header, started) = controller_fixture();
    start(&fixture, &header, &started);
    let awaiting = awaiting_event(&header, &started);
    fixture
        .graph
        .store
        .append_scheduled_graph_controller_event(&awaiting)
        .expect("append awaiting-consent event");
    let dispatch = dispatch_event(&header, &awaiting);
    fixture
        .graph
        .store
        .append_scheduled_graph_controller_event(&dispatch)
        .expect("append dispatch plan");
    (fixture, header, dispatch)
}

fn awaiting_event(
    header: &ScheduledGraphControllerHeader,
    started: &ScheduledGraphControllerEvent,
) -> ScheduledGraphControllerEvent {
    event(
        header,
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
    )
}

fn dispatch_event(
    header: &ScheduledGraphControllerHeader,
    awaiting: &ScheduledGraphControllerEvent,
) -> ScheduledGraphControllerEvent {
    event(
        header,
        3,
        Some(awaiting.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::DispatchPlanned {
            execution_ordinal: 0,
            node_id: "frontend".into(),
            provider_request_id: "scheduled-provider-request-controller".into(),
            authorization_sha256: "5".repeat(64),
            snapshot_sha256: "3".repeat(64),
            decision_sha256: "4".repeat(64),
            effectful_step_reservation: 1,
            reserved_cost_usd_micros: 10_000,
            off_machine_consent_observed: true,
            predecessor_content_consent_observed: false,
        },
    )
}

fn mutate(connection: &Connection, sql: &str, label: &str) {
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable relational guard for at-rest corruption fixture");
    connection
        .pragma_update(None, "ignore_check_constraints", "ON")
        .expect("disable shape guard for at-rest corruption fixture");
    assert_eq!(
        connection.execute(sql, []).expect("apply corruption"),
        1,
        "corruption fixture must change one row: {label}"
    );
}

fn assert_matrix_covers_table(connection: &Connection, table: &str, labels: &[&str]) {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("read controller table columns");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query controller table columns")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect controller table columns");
    for column in columns {
        assert!(
            labels.iter().any(|label| {
                *label == column
                    || label
                        .strip_prefix(&column)
                        .is_some_and(|rest| rest.starts_with(' '))
            }),
            "corruption matrix does not cover persisted column {table}.{column}"
        );
    }
}

fn assert_corrupt(store: &SqliteHubStore, graph_run_id: &str, label: &str) {
    assert!(
        matches!(
            store.inspect_scheduled_graph_controller(graph_run_id),
            Err(HubStoreError::Corrupt { .. })
        ),
        "persisted corruption must fail closed: {label}"
    );
}
