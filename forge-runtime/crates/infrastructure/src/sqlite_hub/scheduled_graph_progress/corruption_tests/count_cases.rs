use super::{
    fixture::{apply_corruption, assert_snapshot_corrupt, provider_fixture},
    lifecycle_fixture::claimed_fixture,
};

#[test]
fn orphan_provider_without_candidate_fails_closed() {
    let fixture = provider_fixture();
    apply_corruption(
        &fixture,
        "DELETE FROM group_agent_graph_scheduled_node_contract_candidates",
    );
    assert_snapshot_corrupt(&fixture, "provider without candidate");
}

#[test]
fn orphan_lifecycle_without_provider_fails_closed() {
    let fixture = claimed_fixture();
    apply_corruption(
        &fixture,
        "DELETE FROM group_agent_graph_scheduled_node_provider_requests",
    );
    assert_snapshot_corrupt(&fixture, "lifecycle without provider");
}

#[test]
fn extra_nonprojectable_candidate_fails_closed() {
    let fixture = provider_fixture();
    apply_corruption(
        &fixture,
        "INSERT INTO group_agent_graph_scheduled_node_successor_candidates
         SELECT * FROM group_agent_graph_scheduled_node_contract_candidates",
    );
    assert_snapshot_corrupt(&fixture, "extra candidate row count");
}

#[test]
fn extra_nonprojectable_provider_fails_closed() {
    let fixture = provider_fixture();
    apply_corruption(&fixture, EXTRA_PROVIDER);
    assert_snapshot_corrupt(&fixture, "extra provider row count");
}

#[test]
fn extra_nonprojectable_lifecycle_fails_closed() {
    let fixture = claimed_fixture();
    apply_corruption(&fixture, EXTRA_LIFECYCLE);
    assert_snapshot_corrupt(&fixture, "extra lifecycle row count");
}

const EXTRA_PROVIDER: &str = "INSERT INTO group_agent_graph_scheduled_node_provider_requests
     SELECT id||'-extra',graph_run_id,schedule_id,scheduled_contract_id||'-extra',
       provider_request_version,codec_protocol_version,31,node_id||'-extra',attempt,
       scheduled_contract_sha256,logical_request_id||'-extra',logical_request_sha256,
       schedule_sha256,project_lane_sha256,provider_kind,endpoint,model,destination_sha256,
       pricing_snapshot_sha256,provider_request_blob,provider_request_bytes,
       provider_request_sha256,prepared_request_sha256,expected_last_event_seq,
       expected_last_event_sha256,provider_request_prepared,provider_request_sent,
       lifecycle_contract_admitted,execution_authority_released,dispatch_authority_released,
       project_lane_claimed,progress_observed,successor_advance_authorized,
       idempotency_key||'-extra',created_at_ms
     FROM group_agent_graph_scheduled_node_provider_requests";

const EXTRA_LIFECYCLE: &str = "INSERT INTO group_agent_graph_scheduled_node_dispatch_lifecycles
     SELECT id||'-extra',graph_run_id,provider_request_id||'-extra',authorization_id||'-extra',
       authorization_sha256,provider_request_sha256,request_body_blob,request_body_bytes,
       zeroblob(32),node_id||'-extra',attempt,claim_json,claim_json_bytes,
       active_lane_json,active_lane_json_bytes,release_control_json,release_control_json_bytes,
       authorization_json,authorization_json_bytes,pricing_json,pricing_json_bytes,
       claim_event_json,claim_event_json_bytes,status,lane_active,artifact_json,
       artifact_json_bytes,terminal_control_json,terminal_control_json_bytes,
       terminal_receipt_json,terminal_receipt_json_bytes,created_at_ms,terminalized_at_ms,
       adjudicated_at_ms
     FROM group_agent_graph_scheduled_node_dispatch_lifecycles";
