mod sqlite_group_agent_graph_run_support;

use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    BeginGroupAgentGraphRunDisposition, GroupAgentGraphRunStore, HubEntity, HubStoreError,
};
use rusqlite::params;

use sqlite_group_agent_graph_run_support::{Fixture, encode_graph_manifest, recanonicalize};

#[test]
fn begin_replays_semantics_and_reads_full_and_metadata_views() {
    let fixture = Fixture::new();
    let request = fixture.request("graph-run-1", "run-key", 30);
    let created = fixture
        .store
        .begin_group_agent_graph_run(&request)
        .expect("begin Graph Run");
    assert_eq!(
        created.disposition,
        BeginGroupAgentGraphRunDisposition::Created
    );
    assert_eq!(created.inspection.plan_json, request.plan_json);
    assert_eq!(
        created.inspection.event_jsons,
        vec![request.event_json.clone()]
    );

    let replay = fixture.request("ignored-candidate", "run-key", 999);
    let replayed = fixture
        .store
        .begin_group_agent_graph_run(&replay)
        .expect("replay Graph Run semantics");
    assert_eq!(
        replayed.disposition,
        BeginGroupAgentGraphRunDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_reads(&fixture, &created.inspection);
    assert_passive_row_shape(&fixture, &request);
}

#[test]
fn divergent_key_id_source_plan_and_event_inputs_leave_no_partial_rows() {
    let fixture = prepared_fixture();
    let request = fixture.request("graph-run-1", "run-key", 30);

    let mut divergent = fixture.request("other-run", "run-key", 31);
    divergent.graph_manifest_sha256 = "0".repeat(64);
    divergent.plan.graph_manifest_sha256 = divergent.graph_manifest_sha256.clone();
    recanonicalize(&mut divergent);
    assert_run_conflict(&fixture.store.begin_group_agent_graph_run(&divergent));

    let reused_id = fixture.request("graph-run-1", "other-key", 32);
    assert_run_conflict(&fixture.store.begin_group_agent_graph_run(&reused_id));

    let mut bad_source = fixture.request("bad-source", "bad-source-key", 33);
    bad_source.source_snapshot_sha256 = "0".repeat(64);
    recanonicalize(&mut bad_source);
    assert_run_conflict(&fixture.store.begin_group_agent_graph_run(&bad_source));

    let mut bad_plan = fixture.request("bad-plan", "bad-plan-key", 34);
    bad_plan.plan.authored_node_ids.reverse();
    recanonicalize(&mut bad_plan);
    assert_run_conflict(&fixture.store.begin_group_agent_graph_run(&bad_plan));

    let mut bad_event = fixture.request("bad-event", "bad-event-key", 35);
    bad_event.event.graph_run_id = "other".into();
    bad_event.event_json = bad_event
        .event
        .canonical_json()
        .expect("canonical bad event");
    assert_run_conflict(&fixture.store.begin_group_agent_graph_run(&bad_event));

    assert_eq!(fixture.row_count("group_agent_graph_runs"), 1);
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 1);
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_graph_run(&request.graph_run_id)
            .expect("original Graph Run remains")
            .plan,
        request.plan
    );
}

#[test]
fn concurrent_same_key_has_one_created_identity_and_only_replays() {
    const WORKERS: usize = 6;
    let fixture = Fixture::new();
    let barrier = Arc::new(Barrier::new(WORKERS));
    let workers = (0..WORKERS)
        .map(|index| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            let request = fixture.request(
                &format!("candidate-{index}"),
                "shared-key",
                40 + u64::try_from(index).expect("worker index fits"),
            );
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .begin_group_agent_graph_run(&request)
                    .expect("concurrent Graph Run begin")
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("Graph Run worker"))
        .collect::<Vec<_>>();

    assert_eq!(
        results
            .iter()
            .filter(|result| result.disposition == BeginGroupAgentGraphRunDisposition::Created)
            .count(),
        1
    );
    assert!(
        results
            .iter()
            .all(|result| result.inspection == results[0].inspection)
    );
    assert_eq!(fixture.row_count("group_agent_graph_runs"), 1);
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 1);
}

#[test]
fn concurrent_divergent_same_key_has_one_winner_and_one_conflict() {
    let fixture = Fixture::new();
    let barrier = Arc::new(Barrier::new(2));
    let valid = fixture.request("valid-run", "divergent-key", 50);
    let mut divergent = fixture.request("divergent-run", "divergent-key", 51);
    divergent.graph_manifest_sha256 = "0".repeat(64);
    divergent.plan.graph_manifest_sha256 = divergent.graph_manifest_sha256.clone();
    recanonicalize(&mut divergent);
    let workers = [valid, divergent]
        .into_iter()
        .map(|request| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.begin_group_agent_graph_run(&request)
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("divergent worker"))
        .collect::<Vec<_>>();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| is_run_conflict(result))
            .count(),
        1
    );
    assert_eq!(fixture.row_count("group_agent_graph_runs"), 1);
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 1);
}

#[test]
fn deep_stored_corruption_is_corrupt_but_metadata_list_stays_content_free() {
    for sql in [
        "UPDATE group_agent_graph_runs SET plan_blob=zeroblob(length(plan_blob))",
        "UPDATE group_agent_graph_runs SET plan_sha256=zeroblob(32)",
        "UPDATE group_agent_graph_runs SET source_snapshot_sha256=zeroblob(32)",
        "UPDATE group_agent_graph_runs SET graph_manifest_sha256=zeroblob(32)",
        "UPDATE group_agent_graph_run_events SET event_blob=zeroblob(length(event_blob))",
        "UPDATE group_agent_graph_run_events SET event_sha256=zeroblob(32)",
        "DELETE FROM group_agent_graph_run_events",
        "UPDATE group_agent_graphs SET manifest_blob=zeroblob(length(manifest_blob))",
        "UPDATE group_runs SET snapshot_sha256=zeroblob(32) WHERE id='group-run-1'",
    ] {
        let fixture = prepared_fixture();
        fixture
            .connection()
            .execute_batch(sql)
            .expect("inject deep Graph Run corruption");
        assert_corrupt(&fixture.store.inspect_group_agent_graph_run("graph-run-1"));
        assert_corrupt(
            &fixture
                .store
                .begin_group_agent_graph_run(&fixture.request("other", "run-key", 99)),
        );
        let listed = fixture
            .store
            .list_group_agent_graph_runs(Some("graph-1"), 10)
            .expect("metadata list skips plan, event, and graph contents");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].graph_run_id, "graph-run-1");
    }
}

#[test]
fn frozen_member_corruption_is_detected_before_replay_conflict() {
    let fixture = prepared_fixture();
    let mut manifest = fixture.graph.manifest.clone();
    manifest.nodes[0].member_role = "sso".into();
    let (bytes, digest) = encode_graph_manifest(&manifest);
    fixture
        .connection()
        .execute(
            "UPDATE group_agent_graphs
             SET manifest_blob=?1,manifest_bytes=?2,manifest_sha256=?3
             WHERE id='graph-1'",
            params![
                bytes,
                i64::try_from(bytes.len()).expect("length fits"),
                digest
            ],
        )
        .expect("inject frozen member corruption");
    let mut divergent = fixture.request("other", "run-key", 80);
    divergent.graph_manifest_sha256 = "0".repeat(64);
    divergent.plan.graph_manifest_sha256 = divergent.graph_manifest_sha256.clone();
    recanonicalize(&mut divergent);
    assert_corrupt(&fixture.store.begin_group_agent_graph_run(&divergent));
}

#[test]
fn invalid_metadata_fails_list_while_missing_filter_and_limits_fail_closed() {
    let fixture = prepared_fixture();
    fixture
        .connection()
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON;
             UPDATE group_agent_graph_runs SET dispatch_authority_released=1;",
        )
        .expect("inject released authority");
    assert_corrupt(&fixture.store.list_group_agent_graph_runs(None, 10));

    let empty = Fixture::new();
    assert!(matches!(
        empty.store.list_group_agent_graph_runs(Some("missing"), 10),
        Err(HubStoreError::NotFound {
            entity: HubEntity::GroupAgentGraph,
            ..
        })
    ));
    for limit in [0, 101] {
        assert_run_conflict(&empty.store.list_group_agent_graph_runs(None, limit));
    }
    assert_run_conflict(
        &empty
            .store
            .list_group_agent_graph_runs(Some(&"x".repeat(129)), 10),
    );
}

#[test]
fn post_insert_reread_failure_rolls_back_run_and_sole_event() {
    let fixture = Fixture::new();
    fixture
        .connection()
        .execute_batch(
            "CREATE TRIGGER mutate_graph_run_after_event
             AFTER INSERT ON group_agent_graph_run_events
             BEGIN
               UPDATE group_agent_graph_runs SET node_count=3 WHERE id=NEW.graph_run_id;
             END;",
        )
        .expect("install post-insert mutation");
    assert_corrupt(&fixture.store.begin_group_agent_graph_run(&fixture.request(
        "graph-run-1",
        "run-key",
        90,
    )));
    assert_eq!(fixture.row_count("group_agent_graph_runs"), 0);
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 0);
}

fn prepared_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_group_agent_graph_run(&fixture.request("graph-run-1", "run-key", 30))
        .expect("seed Graph Run");
    fixture
}

fn assert_reads(fixture: &Fixture, expected: &forge_runtime_domain::GroupAgentGraphRunInspection) {
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_graph_run(&expected.run.graph_run_id)
            .expect("inspect Graph Run"),
        *expected
    );
    let listed = fixture
        .store
        .list_group_agent_graph_runs(Some("graph-1"), 10)
        .expect("list Graph Runs");
    assert_eq!(listed.as_slice(), std::slice::from_ref(&expected.run));
}

fn assert_passive_row_shape(
    fixture: &Fixture,
    request: &forge_runtime_domain::BeginGroupAgentGraphRun,
) {
    let connection = fixture.connection();
    let row: (String, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT status,execution_contract_present,dispatch_authority_released,
                    last_event_seq,journal_bytes
             FROM group_agent_graph_runs WHERE id='graph-run-1'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("Graph Run row");
    assert_eq!(row.0, "awaiting_execution_contract");
    assert_eq!((row.1, row.2, row.3), (0, 0, 1));
    assert_eq!(
        row.4,
        i64::try_from(request.event_json.len()).expect("length fits")
    );
    let digest: String = connection
        .query_row(
            "SELECT lower(hex(event_sha256)) FROM group_agent_graph_run_events",
            [],
            |row| row.get(0),
        )
        .expect("event digest");
    assert_eq!(digest, request.event.expected_sha256().expect("event SHA"));
}

fn is_run_conflict<T>(result: &Result<T, HubStoreError>) -> bool {
    matches!(
        result,
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAgentGraphRun,
            ..
        })
    )
}

fn assert_run_conflict<T>(result: &Result<T, HubStoreError>) {
    assert!(is_run_conflict(result), "expected Graph Run conflict");
}

fn assert_corrupt<T>(result: &Result<T, HubStoreError>) {
    assert!(
        matches!(result, Err(HubStoreError::Corrupt { .. })),
        "expected corruption"
    );
}
