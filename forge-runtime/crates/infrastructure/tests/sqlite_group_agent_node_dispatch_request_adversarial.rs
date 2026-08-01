#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
mod sqlite_group_agent_node_dispatch_request_support;
#[allow(dead_code)]
mod sqlite_group_agent_node_execution_contract_support;

use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    GroupAgentNodeDispatchRequestStore, HubEntity, HubStoreError,
    PrepareGroupAgentNodeDispatchRequestDisposition, PrepareGroupAgentNodeDispatchRequestResult,
    group_agent_node_destination_sha256, group_agent_node_provider_request_sha256,
};
use serde_json::{Value, json};

use sqlite_group_agent_node_dispatch_request_support::{admitted_fixture, recanonicalize, request};

type BodyMutation = fn(&mut Vec<u8>);

#[test]
fn exact_codec_rejects_the_complete_provider_body_mutation_matrix() {
    let cases: [(&str, BodyMutation); 8] = [
        ("unknown field", add_unknown_field),
        ("duplicate field", add_duplicate_field),
        ("noncanonical trailing LF", add_trailing_lf),
        ("include", change_include),
        ("tools", change_tools),
        ("instructions", change_instructions),
        ("token limit", change_token_limit),
        ("model", change_model),
    ];

    for (name, mutate) in cases {
        let (fixture, contract_id) = admitted_fixture();
        let mut candidate = request(&fixture, &contract_id, "dispatch-key", 50);
        mutate(&mut candidate.provider_request_body);
        candidate.provider_request_sha256 =
            group_agent_node_provider_request_sha256(&candidate.provider_request_body);
        recanonicalize(&mut candidate);

        assert_conflict(
            &fixture
                .store
                .prepare_group_agent_node_dispatch_request(&candidate),
            name,
        );
        assert_eq!(
            fixture.row_count("group_agent_graph_node_dispatch_requests"),
            0,
            "{name} left a request row"
        );
        assert_eq!(
            fixture.row_count("group_agent_graph_run_events"),
            2,
            "{name} left an event row"
        );
    }
}

#[test]
fn concurrent_different_keys_have_one_winner_and_all_other_calls_conflict() {
    const WORKERS: usize = 6;
    let (fixture, contract_id) = admitted_fixture();
    let barrier = Arc::new(Barrier::new(WORKERS));
    let workers = (0..WORKERS)
        .map(|index| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            let candidate = request(
                &fixture,
                &contract_id,
                &format!("different-key-{index}"),
                60 + u64::try_from(index).expect("worker index"),
            );
            std::thread::spawn(move || {
                barrier.wait();
                store.prepare_group_agent_node_dispatch_request(&candidate)
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("preparation worker"))
        .collect::<Vec<_>>();

    assert_eq!(created_count(&results), 1);
    assert_eq!(conflict_count(&results), WORKERS - 1);
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        1
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 3);
}

#[test]
fn one_request_identity_cannot_be_reused_for_different_bytes() {
    let (fixture, contract_id) = admitted_fixture();
    let original = request(&fixture, &contract_id, "dispatch-key", 50);
    let created = fixture
        .store
        .prepare_group_agent_node_dispatch_request(&original)
        .expect("seed exact request");
    let mut reused = original.clone();
    reused.provider_request_body.push(b'\n');

    assert_conflict(
        &fixture
            .store
            .prepare_group_agent_node_dispatch_request(&reused),
        "identity reuse",
    );
    let inspected = fixture
        .store
        .inspect_group_agent_node_dispatch_request(&original.dispatch_request_id)
        .expect("original remains readable");
    assert_eq!(inspected, created.inspection);
}

#[test]
fn self_consistent_destination_and_pricing_drift_conflict_without_writes() {
    let (destination_fixture, destination_contract) = admitted_fixture();
    let mut destination = request(
        &destination_fixture,
        &destination_contract,
        "destination-key",
        50,
    );
    destination.endpoint = "https://example.com/v1/responses".into();
    destination.destination_sha256 = group_agent_node_destination_sha256(
        destination.provider,
        &destination.endpoint,
        &destination.model,
    );
    recanonicalize(&mut destination);
    assert_conflict(
        &destination_fixture
            .store
            .prepare_group_agent_node_dispatch_request(&destination),
        "destination drift",
    );
    assert_no_preparation(&destination_fixture);

    let (pricing_fixture, pricing_contract) = admitted_fixture();
    let mut pricing = request(&pricing_fixture, &pricing_contract, "pricing-key", 50);
    pricing.pricing_snapshot_sha256 = "a".repeat(64);
    recanonicalize(&mut pricing);
    assert_conflict(
        &pricing_fixture
            .store
            .prepare_group_agent_node_dispatch_request(&pricing),
        "pricing drift",
    );
    assert_no_preparation(&pricing_fixture);
}

#[test]
fn missing_projected_contract_is_corrupt_before_any_preparation_write() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    fixture
        .connection()
        .execute(
            "DELETE FROM group_agent_graph_node_execution_contracts WHERE id=?1",
            [&contract_id],
        )
        .expect("delete projected contract");

    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_node_dispatch_request(&candidate),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_no_preparation(&fixture);
}

#[test]
fn orphaned_fresh_contract_is_corrupt_before_any_preparation_write() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    fixture
        .connection()
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DELETE FROM group_agent_graph_runs WHERE id='graph-run-1';",
        )
        .expect("orphan admitted contract fixture");

    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_node_dispatch_request(&candidate),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_no_preparation(&fixture);
}

#[test]
fn missing_run_is_not_corrupt_because_a_different_run_has_a_contract() {
    let (fixture, contract_id) = admitted_fixture();
    let mut candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    candidate.graph_run_id = "missing-other-run".into();
    recanonicalize(&mut candidate);

    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_node_dispatch_request(&candidate),
        Err(HubStoreError::NotFound {
            entity: HubEntity::GroupAgentGraphRun,
            ..
        })
    ));
    assert_no_preparation(&fixture);
}

fn assert_no_preparation(fixture: &sqlite_group_agent_graph_run_support::Fixture) {
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        0
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 2);
}

fn created_count(
    results: &[Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError>],
) -> usize {
    results
        .iter()
        .filter(|result| {
            result.as_ref().is_ok_and(|value| {
                value.disposition == PrepareGroupAgentNodeDispatchRequestDisposition::Created
            })
        })
        .count()
}

fn conflict_count<T>(results: &[Result<T, HubStoreError>]) -> usize {
    results
        .iter()
        .filter(|result| {
            matches!(
                result,
                Err(HubStoreError::Conflict {
                    entity: HubEntity::GroupAgentNodeDispatchRequest,
                    ..
                })
            )
        })
        .count()
}

fn assert_conflict<T>(result: &Result<T, HubStoreError>, case: &str) {
    assert!(
        matches!(
            result,
            Err(HubStoreError::Conflict {
                entity: HubEntity::GroupAgentNodeDispatchRequest,
                ..
            })
        ),
        "{case} did not conflict"
    );
}

fn add_unknown_field(body: &mut Vec<u8>) {
    edit_json(body, |value| {
        value["unknown"] = Value::Bool(true);
    });
}

fn add_duplicate_field(body: &mut Vec<u8>) {
    replace_bytes(body, b"\"store\":false", b"\"store\":false,\"store\":false");
}

fn add_trailing_lf(body: &mut Vec<u8>) {
    body.push(b'\n');
}

fn change_include(body: &mut Vec<u8>) {
    edit_json(body, |value| {
        value["include"] = json!(["reasoning.summary"]);
    });
}

fn change_tools(body: &mut Vec<u8>) {
    edit_json(body, |value| {
        value["tools"] = json!([{"type": "web_search_preview"}]);
    });
}

fn change_instructions(body: &mut Vec<u8>) {
    edit_json(body, |value| {
        value["instructions"] = json!("changed frozen instruction");
    });
}

fn change_token_limit(body: &mut Vec<u8>) {
    edit_json(body, |value| {
        value["max_output_tokens"] = json!(1);
    });
}

fn change_model(body: &mut Vec<u8>) {
    edit_json(body, |value| {
        value["model"] = json!("changed-model");
    });
}

fn edit_json(body: &mut Vec<u8>, edit: impl FnOnce(&mut Value)) {
    let mut value: Value = serde_json::from_slice(body).expect("provider body JSON");
    edit(&mut value);
    *body = serde_json::to_vec(&value).expect("canonical JSON mutation");
}

fn replace_bytes(body: &mut Vec<u8>, old: &[u8], new: &[u8]) {
    let offset = body
        .windows(old.len())
        .position(|window| window == old)
        .expect("mutation target");
    body.splice(offset..offset + old.len(), new.iter().copied());
}
