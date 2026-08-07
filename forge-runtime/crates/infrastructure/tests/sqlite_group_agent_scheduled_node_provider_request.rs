#![allow(dead_code)]

#[path = "sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
mod sqlite_group_agent_graph_execution_schedule_support;
#[path = "sqlite_group_agent_graph_run_support/mod.rs"]
mod sqlite_group_agent_graph_run_support;
#[path = "sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_contract_support;
#[path = "sqlite_group_agent_scheduled_node_provider_request_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_provider_request_support;

use std::{
    fs,
    sync::{Arc, Barrier},
};

use forge_runtime_domain::{
    GroupAgentGraphRunStore, GroupAgentScheduledNodeProviderRequestStore, HubStoreError,
    PrepareGroupAgentScheduledNodeProviderRequestDisposition,
};

use sqlite_group_agent_scheduled_node_provider_request_support::prepared_fixture;

const TABLE: &str = "group_agent_graph_scheduled_node_provider_requests";
const DUPLICATE_CORRUPT_ROW_SQL: &str =
    "INSERT INTO group_agent_graph_scheduled_node_provider_requests
     SELECT 'corrupt-provider-request','graph-run-corrupt','schedule-corrupt',
       'contract-corrupt',provider_request_version,codec_protocol_version,
       execution_ordinal,'node-corrupt',attempt,scheduled_contract_sha256,
       'logical-request-corrupt',logical_request_sha256,schedule_sha256,
       project_lane_sha256,provider_kind,endpoint,model,destination_sha256,
       pricing_snapshot_sha256,provider_request_blob,provider_request_bytes,
       provider_request_sha256,prepared_request_sha256,expected_last_event_seq,
       expected_last_event_sha256,provider_request_prepared,provider_request_sent,
       lifecycle_contract_admitted,execution_authority_released,
       dispatch_authority_released,project_lane_claimed,progress_observed,
       successor_advance_authorized,'corrupt-provider-key',created_at_ms
     FROM group_agent_graph_scheduled_node_provider_requests WHERE id=?1";

#[test]
fn prepare_replay_inspect_and_list_preserve_the_pristine_run_and_sources() {
    let (fixture, request) = prepared_fixture();
    let before = source_snapshot(&fixture.connection());

    let created = fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("prepare scheduled provider request");
    assert_eq!(
        created.disposition,
        PrepareGroupAgentScheduledNodeProviderRequestDisposition::Created
    );
    assert_eq!(
        created.inspection.provider_request_body,
        request.provider_request_body
    );
    assert!(!created.inspection.provider_request_body.ends_with(b"\n"));
    assert_eq!(source_snapshot(&fixture.connection()), before);

    let mut replay = request.clone();
    replay.prepared_at_ms = 999;
    let replayed = fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&replay)
        .expect("exact request replay");
    assert_eq!(
        replayed.disposition,
        PrepareGroupAgentScheduledNodeProviderRequestDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_eq!(source_snapshot(&fixture.connection()), before);
    assert_eq!(fixture.row_count(TABLE), 1);

    let inspected = fixture
        .store
        .inspect_group_agent_scheduled_node_provider_request(&request.provider_request_id)
        .expect("inspect exact request");
    assert_eq!(inspected, created.inspection);
    let listed = fixture
        .store
        .list_group_agent_scheduled_node_provider_requests(Some("graph-run-1"), 10)
        .expect("list request metadata");
    assert_eq!(listed, [created.inspection.record]);

    assert_pristine_run(&fixture.store);
}

fn assert_pristine_run(store: &forge_runtime_infrastructure::SqliteHubStore) {
    let run = store
        .inspect_group_agent_graph_run("graph-run-1")
        .expect("deep Graph Run validation includes provider sidecar");
    assert_eq!(run.run.v, 1);
    assert_eq!(run.run.last_event_seq, 1);
    assert!(!run.run.execution_contract_present);
    assert!(!run.run.dispatch_request_present);
    assert!(!run.run.dispatch_authority_released);
}

#[test]
fn same_key_divergence_and_different_key_identity_reuse_conflict_without_writes() {
    let (fixture, request) = prepared_fixture();
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("seed request");
    let before = source_snapshot(&fixture.connection());

    let mut divergent = request.clone();
    divergent.model = "gpt-4.1-mini-divergent".into();
    sqlite_group_agent_scheduled_node_provider_request_support::reencode_and_reidentify(
        &mut divergent,
    );
    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_scheduled_node_provider_request(&divergent),
        Err(HubStoreError::Conflict { .. })
    ));

    let mut another_key = request.clone();
    another_key.idempotency_key = "another-provider-key".into();
    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_scheduled_node_provider_request(&another_key),
        Err(HubStoreError::Conflict { .. })
    ));
    assert_eq!(fixture.row_count(TABLE), 1);
    assert_eq!(source_snapshot(&fixture.connection()), before);
}

#[test]
fn every_matching_identity_is_validated_before_a_different_key_conflict() {
    let (fixture, request) = prepared_fixture();
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("seed valid request");
    let connection = fixture.connection();
    connection
        .execute_batch("PRAGMA foreign_keys=OFF")
        .expect("disable fixture foreign keys");
    // The corrupt row matches the per-node slot: same Graph Run + node as
    // the later mixed request, so the slot conflict must surface the
    // corruption before any conflict error (v22 wave-parallel identity).
    let corrupt_sql = corrupt_row_sql(&request.node_id);
    connection
        .execute(corrupt_sql.as_str(), [&request.provider_request_id])
        .expect("insert independently matching corrupt row");
    drop(connection);
    let mut mixed = request;
    mixed.graph_run_id = "graph-run-corrupt".into();
    mixed.idempotency_key = "mixed-identity-key".into();

    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_scheduled_node_provider_request(&mixed),
        Err(HubStoreError::Corrupt { .. })
    ));
    mixed.idempotency_key = "scheduled-provider-key".into();
    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_scheduled_node_provider_request(&mixed),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_eq!(fixture.row_count(TABLE), 2);
}

#[test]
fn concurrent_exact_preparations_create_once_and_replay_once() {
    let (fixture, request) = prepared_fixture();
    let store_a = fixture.store.clone();
    let store_b = fixture.store.clone();
    let request_a = request.clone();
    let request_b = request;
    let gate = Arc::new(Barrier::new(3));
    let gate_a = Arc::clone(&gate);
    let gate_b = Arc::clone(&gate);
    let first = std::thread::spawn(move || {
        gate_a.wait();
        store_a.prepare_group_agent_scheduled_node_provider_request(&request_a)
    });
    let second = std::thread::spawn(move || {
        gate_b.wait();
        store_b.prepare_group_agent_scheduled_node_provider_request(&request_b)
    });
    gate.wait();
    let mut dispositions = [
        first
            .join()
            .expect("first thread")
            .expect("first result")
            .disposition,
        second
            .join()
            .expect("second thread")
            .expect("second result")
            .disposition,
    ];
    dispositions.sort_by_key(|value| match value {
        PrepareGroupAgentScheduledNodeProviderRequestDisposition::Created => 0,
        PrepareGroupAgentScheduledNodeProviderRequestDisposition::Replayed => 1,
    });
    assert_eq!(
        dispositions,
        [
            PrepareGroupAgentScheduledNodeProviderRequestDisposition::Created,
            PrepareGroupAgentScheduledNodeProviderRequestDisposition::Replayed,
        ]
    );
    assert_eq!(fixture.row_count(TABLE), 1);
}

#[test]
fn hot_wal_request_is_inspected_without_changing_main_or_wal_bytes() {
    let (fixture, request) = prepared_fixture();
    let writer = fixture.connection();
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA wal_autocheckpoint=0;")
        .expect("checkpoint sources before hot request");
    let created = fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("commit provider request to hot WAL")
        .inspection;
    let wal = fixture.database.with_extension("sqlite3-wal");
    let shm = fixture.database.with_extension("sqlite3-shm");
    assert!(wal.exists() && shm.exists(), "missing live WAL sidecars");
    let main_before = fs::read(&fixture.database).expect("read main before inspection");
    let wal_before = fs::read(&wal).expect("read WAL before inspection");

    let reader =
        forge_runtime_infrastructure::SqliteHubStore::open_existing_dispatch_inspection_read_only(
            &fixture.database,
        )
        .expect("open hot v15 request inspection");
    let inspected = reader
        .inspect_group_agent_scheduled_node_provider_request(&request.provider_request_id)
        .expect("inspect request through hot WAL");

    assert_eq!(inspected, created);
    assert_eq!(
        fs::read(&fixture.database).expect("read main after"),
        main_before
    );
    assert_eq!(fs::read(&wal).expect("read WAL after"), wal_before);
    drop((reader, writer));
}

#[test]
fn stored_body_corruption_wins_over_replay_conflict_and_breaks_deep_run_read() {
    let (fixture, request) = prepared_fixture();
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("seed request");
    fixture
        .connection()
        .execute_batch(&format!(
            "PRAGMA ignore_check_constraints=ON;
             UPDATE {TABLE}
             SET provider_request_blob=x'7b7d',provider_request_bytes=2
             WHERE id='{}';",
            request.provider_request_id
        ))
        .expect("corrupt exact body");

    let mut divergent_key = request.clone();
    divergent_key.idempotency_key = "different-key".into();
    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_scheduled_node_provider_request(&divergent_key),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        fixture
            .store
            .inspect_group_agent_scheduled_node_provider_request(&request.provider_request_id),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        fixture.store.inspect_group_agent_graph_run("graph-run-1"),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn drifted_run_is_aggregate_corruption_for_replay_and_inspection_without_new_writes() {
    let (fixture, request) = prepared_fixture();
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("seed request");
    fixture
        .connection()
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON;
             UPDATE group_agent_graph_runs SET last_event_seq=2
             WHERE id='graph-run-1';",
        )
        .expect("forge source cursor drift");
    let before = fixture.row_count(TABLE);

    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_scheduled_node_provider_request(&request),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        fixture
            .store
            .inspect_group_agent_scheduled_node_provider_request(&request.provider_request_id),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_eq!(fixture.row_count(TABLE), before);
}

#[test]
fn missing_candidate_parent_is_reported_as_corruption_by_graph_run_deep_read() {
    let (fixture, request) = prepared_fixture();
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("seed request");
    let connection = fixture.connection();
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DELETE FROM group_agent_graph_scheduled_node_contract_candidates;",
        )
        .expect("forge missing scheduled contract parent");
    drop(connection);

    assert!(matches!(
        fixture.store.inspect_group_agent_graph_run("graph-run-1"),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn invalid_list_bounds_and_missing_filters_are_structured_errors() {
    let (fixture, mut request) = prepared_fixture();
    assert!(matches!(
        fixture
            .store
            .list_group_agent_scheduled_node_provider_requests(None, 0),
        Err(HubStoreError::Conflict { .. })
    ));
    assert!(matches!(
        fixture
            .store
            .list_group_agent_scheduled_node_provider_requests(Some("missing-run"), 10),
        Err(HubStoreError::NotFound { .. })
    ));
    request.execution_ordinal = usize::MAX;
    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_scheduled_node_provider_request(&request),
        Err(HubStoreError::Conflict { .. })
    ));
    assert_eq!(fixture.row_count(TABLE), 0);
}

fn source_snapshot(connection: &rusqlite::Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT 'run|' || id || '|' || run_version || '|' || status || '|'
                    || execution_contract_present || '|' || dispatch_request_present || '|'
                    || dispatch_authority_released || '|' || last_event_seq || '|' || journal_bytes
             FROM group_agent_graph_runs
             UNION ALL
             SELECT 'event|' || graph_run_id || '|' || seq || '|' || hex(event_blob)
             FROM group_agent_graph_run_events
             UNION ALL
             SELECT 'schedule|' || id || '|' || hex(schedule_blob) || '|' || idempotency_key
             FROM group_agent_graph_execution_schedules
             UNION ALL
             SELECT 'candidate|' || id || '|' || hex(contract_blob) || '|' || idempotency_key
             FROM group_agent_graph_scheduled_node_contract_candidates
             ORDER BY 1",
        )
        .expect("prepare source snapshot");
    statement
        .query_map([], |row| row.get(0))
        .expect("query source snapshot")
        .collect::<Result<_, _>>()
        .expect("read source snapshot")
}

/// Builds and admits the ordinal-1 (backend) zero-receipt successor
/// candidate for the diamond fixture.
#[test]
fn two_nodes_in_one_run_prepare_provider_requests_through_v22() {
    use sqlite_group_agent_graph_run_support as run_support;
    use sqlite_group_agent_graph_execution_schedule_support as schedule_support;
    use sqlite_group_agent_scheduled_node_contract_support as contract_support;
    use sqlite_group_agent_scheduled_node_provider_request_support as request_support;
    use forge_runtime_domain::{
        GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore,
        GroupAgentScheduledNodeContractStore,
    };

    // Diamond: frontend + backend are same-wave siblings (zero predecessors);
    // both admit as successors and both get a provider request in one run.
    let fixture = run_support::Fixture::diamond();
    let _run = fixture
        .store
        .begin_group_agent_graph_run(&fixture.request("graph-run-1", "run-key", 30))
        .expect("seed diamond Graph Run");
    let schedule = schedule_support::request(&fixture, "schedule-key", 40);
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&schedule)
        .expect("admit schedule");
    let initial_admit = contract_support::admission(schedule, "scheduled-contract-key", 50);
    let initial = fixture
        .store
        .admit_group_agent_scheduled_node_contract(&initial_admit)
        .expect("admit initial contract")
        .inspection;
    let backend_inspection = request_support::admit_backend_successor(&fixture.store, &initial_admit);

    // Both provider requests land in the same run (v22 per-node slots; v18
    // per-run UNIQUE would deadlock the second).
    let initial_request = request_support::request(&initial, "scheduled-provider-key-initial", 60);
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&initial_request)
        .expect("initial-node provider request persists");
    let backend_request =
        request_support::request(&backend_inspection, "scheduled-provider-key-backend", 70);
    let stored = fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&backend_request)
        .expect("second-node provider request persists in the same run");
    assert_eq!(stored.inspection.record.execution_ordinal, 1);
    assert_eq!(stored.inspection.record.node_id, "backend");
}

///  inserts a provider-request row that matches the per-node
/// slot of the given node (v22 wave-parallel identity) but is corrupt.
fn corrupt_row_sql(node_id: &str) -> String {
    format!(
        "INSERT INTO group_agent_graph_scheduled_node_provider_requests
         SELECT 'corrupt-provider-request', 'graph-run-corrupt', 'schedule-corrupt',
           'contract-corrupt', provider_request_version, codec_protocol_version,
           execution_ordinal, '{node_id}', attempt, scheduled_contract_sha256,
           'logical-request-corrupt', logical_request_sha256, schedule_sha256,
           project_lane_sha256, provider_kind, endpoint, model, destination_sha256,
           pricing_snapshot_sha256, provider_request_blob, provider_request_bytes,
           provider_request_sha256, prepared_request_sha256, expected_last_event_seq,
           expected_last_event_sha256, provider_request_prepared, provider_request_sent,
           lifecycle_contract_admitted, execution_authority_released,
           dispatch_authority_released, project_lane_claimed, progress_observed,
           successor_advance_authorized, 'corrupt-provider-key', created_at_ms
         FROM group_agent_graph_scheduled_node_provider_requests WHERE id=?1"
    )
}

#[test]
fn adjudicate_update_columns_and_status_are_live_through_v23() {
    // Stage-03 Finding 1 regression: v23 restores 'adjudicated' and
    // adjudicated_at_ms; a 0-row UPDATE still validates the SQL against the
    // live table.
    let (fixture, _request) = sqlite_group_agent_scheduled_node_contract_support::prepared_fixture();
    let connection = fixture.connection();
    let updated = connection
        .execute(
            "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
             SET status='adjudicated', lane_active=0, adjudicated_at_ms=200
             WHERE provider_request_id='nonexistent-adjudicate-row'",
            [],
        )
        .expect("adjudicate UPDATE must be accepted by the v23 schema");
    assert_eq!(updated, 0, "no rows match the sentinel id");
    // The status/lane CHECK must accept an adjudicated row shape.
    let accepted = connection
        .query_row(
            "SELECT CASE WHEN ?1 IN ('claimed','terminalized','quarantined','adjudicated') THEN 1 ELSE 0 END",
            ["adjudicated"],
            |row| row.get::<_, i64>(0),
        )
        .expect("status domain");
    assert_eq!(accepted, 1, "adjudicated must be a legal lifecycle status");
}

/// `hex_bytes` decodes a 64-char hex digest into 32 raw bytes.
fn hex_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
        .collect()
}
