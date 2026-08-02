use serde_json::Value;

use super::*;
use crate::{
    GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractRecord, group_agent_node_destination_sha256,
    group_agent_node_provider_request_sha256,
};

const BODY: &[u8] = b"{}";
const BODY_SHA256: &str = "8075abac4fd1ae16f654882594d9583a28c769020a4ed6a5251fe9b6f570b460";
const DESTINATION_SHA256: &str = "11b9241e923e4ff5512595de764f6da0d350cbb319a51c7f20036ae8d6e18457";
const PREPARED_SHA256: &str = "8970529aa055c2e8960d66f7cb8a0a7867469cc682bdab15f0ace752e4a836e6";
const PREPARED_ID: &str = "scheduled-node-provider-request-8970529aa055c2e8960d66f7cb8a0a7867469cc682bdab15f0ace752e4a836e6";
const PAYLOAD_JSON: &str = r#"{"v":1,"codec_protocol_version":1,"graph_run_id":"graph-run-fixture-v1","schedule_id":"graph-execution-schedule-809d5235e4298ea8a66cb0654b0e662b94a8568e4c184cf1a927bda1c46e8148","schedule_sha256":"809d5235e4298ea8a66cb0654b0e662b94a8568e4c184cf1a927bda1c46e8148","scheduled_contract_id":"scheduled-node-contract-324169fceacde6aeb41f764332043bb236b80aee4fc57420c0122130679cc3a7","scheduled_contract_sha256":"324169fceacde6aeb41f764332043bb236b80aee4fc57420c0122130679cc3a7","expected_last_event_seq":1,"expected_last_event_sha256":"f764b91303620ce38d186efade0418809176805b1c1774754c606197f6468e62","execution_ordinal":0,"node_id":"frontend","attempt":1,"project_lane_sha256":"a4ca84117c90526cc46c0e58273f3d607257c0902011d4fd5f980aadf9dff30a","provider_kind":"openai_responses","endpoint":"https://api.openai.com/v1/responses","model":"gpt-5.6-sol","destination_sha256":"11b9241e923e4ff5512595de764f6da0d350cbb319a51c7f20036ae8d6e18457","logical_request_id":"scheduled-node-request-b265e82314ec207140d2779c5080afe481f55db0a8339930859f1cda0dc653af","logical_request_sha256":"b265e82314ec207140d2779c5080afe481f55db0a8339930859f1cda0dc653af","pricing_snapshot_sha256":"4444444444444444444444444444444444444444444444444444444444444444","request_body_bytes":2,"request_body_sha256":"8075abac4fd1ae16f654882594d9583a28c769020a4ed6a5251fe9b6f570b460","provider_request_prepared":true,"provider_request_sent":false,"lifecycle_contract_admitted":false,"execution_authority_released":false,"dispatch_authority_released":false,"project_lane_claimed":false,"progress_observed":false,"successor_advance_authorized":false}"#;

#[test]
fn identity_locks_exact_payload_domain_digest_and_direct_id() {
    let record = record();

    assert_eq!(record.canonical_payload_json().unwrap(), PAYLOAD_JSON);
    assert!(!PAYLOAD_JSON.ends_with('\n'));
    assert_eq!(record.expected_sha256().unwrap(), PREPARED_SHA256);
    assert_eq!(record.prepared_request_sha256, PREPARED_SHA256);
    assert_eq!(record.provider_request_id, PREPARED_ID);
    assert_eq!(
        group_agent_scheduled_node_provider_request_id(PREPARED_SHA256),
        PREPARED_ID
    );
    assert_eq!(group_agent_node_provider_request_sha256(BODY), BODY_SHA256);
    assert_eq!(
        group_agent_node_destination_sha256(record.provider, &record.endpoint, &record.model),
        DESTINATION_SHA256
    );
    record.validate().expect("golden scheduled request record");
}

#[test]
fn every_semantic_field_is_bound_but_creation_metadata_is_not() {
    let original = record();
    let digest = original.expected_sha256().unwrap();
    for (name, mutation) in record_mutations() {
        let mut changed = original.clone();
        mutation(&mut changed);
        assert_ne!(changed.expected_sha256().unwrap(), digest, "{name}");
        assert!(changed.validate().is_err(), "stale {name} must fail");
    }

    let mut later = original;
    later.created_at_ms += 1;
    assert_eq!(later.expected_sha256().unwrap(), digest);
    later.validate().expect("creation time is non-semantic");
}

type RecordMutation = (
    &'static str,
    fn(&mut GroupAgentScheduledNodeProviderRequestRecord),
);

fn record_mutations() -> Vec<RecordMutation> {
    let mut mutations = source_mutations();
    mutations.extend(destination_mutations());
    mutations.extend(effect_mutations());
    mutations
}

fn source_mutations() -> Vec<RecordMutation> {
    vec![
        ("version", |value| value.v += 1),
        ("codec", |value| value.codec_protocol_version += 1),
        ("run", |value| value.graph_run_id.push('x')),
        ("schedule", |value| value.schedule_id.push('x')),
        ("schedule digest", |value| {
            value.schedule_sha256 = "0".repeat(64);
        }),
        ("contract", |value| value.scheduled_contract_id.push('x')),
        ("contract digest", |value| {
            value.scheduled_contract_sha256 = "0".repeat(64);
        }),
        ("head sequence", |value| value.expected_last_event_seq += 1),
        ("head digest", |value| {
            value.expected_last_event_sha256 = "0".repeat(64);
        }),
        ("ordinal", |value| value.execution_ordinal += 1),
        ("node", |value| value.node_id.push('x')),
        ("attempt", |value| value.attempt += 1),
    ]
}

fn destination_mutations() -> Vec<RecordMutation> {
    vec![
        ("lane", |value| {
            value.project_lane_sha256 = "0".repeat(64);
        }),
        ("endpoint", |value| value.endpoint.push('/')),
        ("model", |value| value.model.push('x')),
        ("destination", |value| {
            value.destination_sha256 = "0".repeat(64);
        }),
        ("logical ID", |value| value.logical_request_id.push('x')),
        ("logical digest", |value| {
            value.logical_request_sha256 = "0".repeat(64);
        }),
        ("pricing", |value| {
            value.pricing_snapshot_sha256 = "0".repeat(64);
        }),
        ("body bytes", |value| value.provider_request_bytes += 1),
        ("body digest", |value| {
            value.provider_request_sha256 = "0".repeat(64);
        }),
    ]
}

fn effect_mutations() -> Vec<RecordMutation> {
    vec![
        ("prepared flag", |value| {
            value.provider_request_prepared = false;
        }),
        ("sent flag", |value| value.provider_request_sent = true),
        ("lifecycle flag", |value| {
            value.lifecycle_contract_admitted = true;
        }),
        ("execution flag", |value| {
            value.execution_authority_released = true;
        }),
        ("dispatch flag", |value| {
            value.dispatch_authority_released = true;
        }),
        ("lane claim", |value| value.project_lane_claimed = true),
        ("progress flag", |value| value.progress_observed = true),
        ("successor flag", |value| {
            value.successor_advance_authorized = true;
        }),
    ]
}

#[test]
fn prepare_binds_exact_body_and_excludes_key_and_local_time() {
    let mut request = prepare();
    request.validate().expect("valid preparation");
    let digest = request.expected_sha256().unwrap();

    request.idempotency_key = "another-valid-key".into();
    request.prepared_at_ms += 1;
    assert_eq!(request.expected_sha256().unwrap(), digest);
    request.validate().expect("non-semantic metadata");

    request.provider_request_body.push(b'\n');
    assert!(request.validate().is_err());
}

#[test]
fn inspection_binds_body_and_every_source_identity() {
    let inspection = inspection();
    inspection.validate().expect("valid inspection");

    let mut body_drift = inspection.clone();
    body_drift.provider_request_body.push(b'\n');
    assert!(body_drift.validate().is_err());

    let mut source_drift = inspection;
    source_drift.scheduled_contract.record.request_sha256 = "0".repeat(64);
    assert!(source_drift.validate().is_err());
}

fn record() -> GroupAgentScheduledNodeProviderRequestRecord {
    GroupAgentScheduledNodeProviderRequestRecord {
        v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        provider_request_id: PREPARED_ID.into(),
        graph_run_id: "graph-run-fixture-v1".into(),
        schedule_id: "graph-execution-schedule-809d5235e4298ea8a66cb0654b0e662b94a8568e4c184cf1a927bda1c46e8148".into(),
        scheduled_contract_id: "scheduled-node-contract-324169fceacde6aeb41f764332043bb236b80aee4fc57420c0122130679cc3a7".into(),
        execution_ordinal: 0,
        node_id: "frontend".into(),
        attempt: 1,
        scheduled_contract_sha256: "324169fceacde6aeb41f764332043bb236b80aee4fc57420c0122130679cc3a7".into(),
        logical_request_id: "scheduled-node-request-b265e82314ec207140d2779c5080afe481f55db0a8339930859f1cda0dc653af".into(),
        logical_request_sha256: "b265e82314ec207140d2779c5080afe481f55db0a8339930859f1cda0dc653af".into(),
        schedule_sha256: "809d5235e4298ea8a66cb0654b0e662b94a8568e4c184cf1a927bda1c46e8148".into(),
        project_lane_sha256: "a4ca84117c90526cc46c0e58273f3d607257c0902011d4fd5f980aadf9dff30a".into(),
        provider: GroupAgentNodeProviderKind::OpenAiResponses,
        endpoint: "https://api.openai.com/v1/responses".into(),
        model: "gpt-5.6-sol".into(),
        destination_sha256: DESTINATION_SHA256.into(),
        pricing_snapshot_sha256: "4".repeat(64),
        provider_request_sha256: BODY_SHA256.into(),
        provider_request_bytes: BODY.len(),
        prepared_request_sha256: PREPARED_SHA256.into(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: 1,
        expected_last_event_sha256: "f764b91303620ce38d186efade0418809176805b1c1774754c606197f6468e62".into(),
        provider_request_prepared: true,
        provider_request_sent: false,
        lifecycle_contract_admitted: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        project_lane_claimed: false,
        progress_observed: false,
        successor_advance_authorized: false,
        created_at_ms: 7,
    }
}

fn prepare() -> PrepareGroupAgentScheduledNodeProviderRequest {
    let record = record();
    PrepareGroupAgentScheduledNodeProviderRequest {
        v: record.v,
        provider_request_id: record.provider_request_id,
        graph_run_id: record.graph_run_id,
        schedule_id: record.schedule_id,
        scheduled_contract_id: record.scheduled_contract_id,
        execution_ordinal: record.execution_ordinal,
        node_id: record.node_id,
        attempt: record.attempt,
        scheduled_contract_sha256: record.scheduled_contract_sha256,
        logical_request_id: record.logical_request_id,
        logical_request_sha256: record.logical_request_sha256,
        schedule_sha256: record.schedule_sha256,
        project_lane_sha256: record.project_lane_sha256,
        provider: record.provider,
        endpoint: record.endpoint,
        model: record.model,
        destination_sha256: record.destination_sha256,
        pricing_snapshot_sha256: record.pricing_snapshot_sha256,
        provider_request_body: BODY.to_vec(),
        provider_request_sha256: record.provider_request_sha256,
        prepared_request_sha256: record.prepared_request_sha256,
        codec_protocol_version: record.codec_protocol_version,
        expected_last_event_seq: record.expected_last_event_seq,
        expected_last_event_sha256: record.expected_last_event_sha256,
        provider_request_prepared: true,
        provider_request_sent: false,
        lifecycle_contract_admitted: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        project_lane_claimed: false,
        progress_observed: false,
        successor_advance_authorized: false,
        idempotency_key: "scheduled-provider-request-key".into(),
        prepared_at_ms: 7,
    }
}

fn inspection() -> GroupAgentScheduledNodeProviderRequestInspection {
    GroupAgentScheduledNodeProviderRequestInspection {
        v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        record: record(),
        provider_request_body: BODY.to_vec(),
        scheduled_contract: scheduled_contract_inspection(),
    }
}

fn scheduled_contract_inspection() -> GroupAgentScheduledNodeContractInspection {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    )))
    .expect("candidate fixture");
    let candidate_json = fixture["expected"]["canonical_contract_json"]
        .as_str()
        .expect("candidate JSON")
        .to_owned();
    let candidate = GroupAgentScheduledNodeContractCandidate::decode_exact(&candidate_json)
        .expect("scheduled candidate");
    GroupAgentScheduledNodeContractInspection {
        v: candidate.v,
        record: scheduled_contract_record(&candidate, candidate_json.len()),
        candidate_json,
        candidate,
    }
}

fn scheduled_contract_record(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    bytes: usize,
) -> GroupAgentScheduledNodeContractRecord {
    GroupAgentScheduledNodeContractRecord {
        v: candidate.v,
        contract_id: candidate.contract_id.clone(),
        graph_run_id: candidate.graph_run_id.clone(),
        schedule_id: candidate.schedule_id.clone(),
        node_id: candidate.node.node_id.clone(),
        execution_ordinal: candidate.node.execution_ordinal,
        attempt: candidate.node.attempt,
        control_snapshot_sha256: candidate.control_snapshot_sha256.clone(),
        schedule_sha256: candidate.schedule_sha256.clone(),
        contract_sha256: candidate.contract_sha256.clone(),
        contract_bytes: bytes,
        request_id: candidate.request.request_id.clone(),
        request_sha256: candidate.request.request_sha256.clone(),
        project_lane_sha256: candidate.node.project_lane_sha256.clone(),
        expected_last_event_seq: candidate.expected_last_event_seq,
        expected_last_event_sha256: candidate.expected_last_event_sha256.clone(),
        predecessor_receipt_count: 0,
        lifecycle_contract_admitted: false,
        provider_request_present: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        progress_observed: false,
        successor_advance_authorized: false,
        created_at_ms: 6,
    }
}
