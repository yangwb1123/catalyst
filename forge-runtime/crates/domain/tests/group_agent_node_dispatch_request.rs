use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION, GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_NODE_DESTINATION_DIGEST_DOMAIN,
    GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION, GROUP_AGENT_NODE_DISPATCH_REQUEST_DIGEST_DOMAIN,
    GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION, GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
    GROUP_AGENT_NODE_PROVIDER_REQUEST_DIGEST_DOMAIN, GroupAgentGraphControlSnapshot,
    GroupAgentGraphCorePlan, GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind,
    GroupAgentGraphRunInspection, GroupAgentGraphRunRecord, GroupAgentGraphRunStatus,
    GroupAgentNodeDispatchRequestInspection, GroupAgentNodeDispatchRequestRecord,
    GroupAgentNodeExecutionContract, GroupAgentNodeExecutionContractInspection,
    GroupAgentNodeExecutionContractRecord, PrepareGroupAgentNodeDispatchRequest,
    group_agent_node_destination_sha256, group_agent_node_dispatch_request_id,
    group_agent_node_provider_request_sha256,
};
use serde::Deserialize;
use serde_json::{Value, json};
const BODY: &[u8] = br#"{"include":["reasoning.encrypted_content"],"input":[{"content":"frozen","role":"user","type":"message"}],"instructions":"system","max_output_tokens":4096,"model":"gpt-5.6-sol","store":false,"stream":true,"tools":[]}"#;
const BODY_SHA: &str = "c4a712933fe43d074709efc2bcbb6241c03cbccfb06446d4637524fb665ea91b";
const DESTINATION_SHA: &str = "11b9241e923e4ff5512595de764f6da0d350cbb319a51c7f20036ae8d6e18457";
const DISPATCH_SHA: &str = "80429643925301ec1a15a46be473619f029257e444fe1f734bccb3949b15bb9a";

#[derive(Deserialize)]
struct GoldenFixture {
    input: GoldenInput,
    expected: GoldenExpected,
}
#[derive(Deserialize)]
struct GoldenInput {
    canonical_control_snapshot_json: String,
}

#[derive(Deserialize)]
struct GoldenExpected {
    canonical_contract_json: String,
}

struct DispatchFixture {
    candidate: PrepareGroupAgentNodeDispatchRequest,
    inspection: GroupAgentNodeDispatchRequestInspection,
}

#[test]
fn exact_body_and_dispatch_identity_are_domain_separated_and_stable() {
    let fixture = dispatch_fixture();
    let request = &fixture.candidate;

    assert_eq!(
        GROUP_AGENT_NODE_PROVIDER_REQUEST_DIGEST_DOMAIN,
        b"forge.group-agent-node-provider-request.v1\0"
    );
    assert_eq!(
        GROUP_AGENT_NODE_DESTINATION_DIGEST_DOMAIN,
        b"forge.group-agent-node-destination.v1\0"
    );
    assert_eq!(
        GROUP_AGENT_NODE_DISPATCH_REQUEST_DIGEST_DOMAIN,
        b"forge.group-agent-node-dispatch-request.v1\0"
    );
    assert_eq!(BODY.len(), 215);
    assert_eq!(group_agent_node_provider_request_sha256(BODY), BODY_SHA);
    assert_eq!(request.provider_request_sha256, BODY_SHA);
    assert_eq!(request.destination_sha256, DESTINATION_SHA);
    assert_eq!(
        request.expected_sha256().expect("dispatch digest"),
        DISPATCH_SHA
    );
    assert_eq!(request.dispatch_request_sha256, DISPATCH_SHA);
    assert_eq!(
        request.dispatch_request_id,
        format!("node-dispatch-request-{DISPATCH_SHA}")
    );
}

#[test]
fn record_and_seq3_decoders_reject_unknown_or_unsupported_fields() {
    let fixture = dispatch_fixture();
    let encoded = serde_json::to_string(&fixture.inspection.record).expect("record JSON");
    let unknown = encoded.replacen("\"v\":1", "\"v\":1,\"unknown\":true", 1);
    assert!(serde_json::from_str::<GroupAgentNodeDispatchRequestRecord>(&unknown).is_err());

    let encoded = fixture.inspection.preparation_event_json;
    let unknown = encoded.replacen("\"seq\":3", "\"seq\":3,\"unknown\":true", 1);
    assert!(serde_json::from_str::<GroupAgentGraphRunEvent>(&unknown).is_err());
    let mut unsupported = serde_json::from_str::<Value>(&encoded).expect("event value");
    unsupported["provider_kind"] = json!("unsupported_provider");
    assert!(serde_json::from_value::<GroupAgentGraphRunEvent>(unsupported).is_err());
}

#[test]
fn preparation_candidate_binds_exact_body_identity_and_seq3_event() {
    let candidate = dispatch_fixture().candidate;
    candidate.validate().expect("valid passive candidate");

    let mut body = candidate.clone();
    body.provider_request_body.push(b'\n');
    assert!(body.validate().is_err());

    let mut event = candidate.clone();
    let GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared { prepared_at_ms, .. } =
        &mut event.event.kind
    else {
        panic!("seq-3 event");
    };
    *prepared_at_ms += 1;
    event.event_json = event.event.canonical_json().expect("event JSON");
    assert!(event.validate().is_err());

    let mut bytes = candidate;
    bytes.event_json.push(' ');
    assert!(bytes.validate().is_err());
}

#[test]
fn inspection_binds_request_to_contract_admission_and_passive_v3_state() {
    let inspection = dispatch_fixture().inspection;
    inspection.validate().expect("valid durable inspection");
    let run = &inspection.contract.graph_run.run;

    assert_eq!(run.v, GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION);
    assert_eq!(
        run.status,
        GroupAgentGraphRunStatus::AwaitingDispatchAuthorization
    );
    assert!(run.execution_contract_present);
    assert!(run.dispatch_request_present);
    assert!(!run.dispatch_authority_released);
    assert_eq!(run.last_event_seq, 3);
    assert_eq!(
        inspection.record.expected_last_event_sha256,
        inspection
            .contract
            .admission_event
            .expected_sha256()
            .expect("admission digest")
    );
}

#[test]
fn body_codec_pricing_and_authority_tampering_fail_closed() {
    let fixture = dispatch_fixture();

    let mut body = fixture.inspection.clone();
    body.provider_request_body[0] ^= 1;
    assert!(body.validate().is_err());

    let mut codec = fixture.inspection.clone();
    codec.record.codec_protocol_version += 1;
    rebind_request_event(&mut codec);
    assert!(codec.validate().is_err());

    let mut pricing = fixture.inspection.clone();
    pricing.record.pricing_snapshot_sha256 = "3".repeat(64);
    rebind_request_event(&mut pricing);
    assert!(pricing.record.validate().is_ok());
    assert!(pricing.contract.graph_run.validate().is_ok());
    assert!(pricing.validate().is_err());

    let mut authority = fixture.inspection;
    authority.contract.graph_run.run.dispatch_authority_released = true;
    assert!(authority.validate().is_err());
}

#[test]
fn every_seq3_field_is_bound_to_the_durable_record() {
    let fixture = dispatch_fixture();
    let cases = [
        ("previous_event_sha256", json!("a".repeat(64))),
        (
            "contract_id",
            json!(format!("node-contract-{}", "a".repeat(64))),
        ),
        ("contract_sha256", json!("a".repeat(64))),
        (
            "dispatch_request_id",
            json!(format!("node-dispatch-request-{}", "a".repeat(64))),
        ),
        ("dispatch_request_sha256", json!("a".repeat(64))),
        ("request_body_sha256", json!("a".repeat(64))),
        ("request_body_bytes", json!(BODY.len() + 1)),
        ("logical_request_sha256", json!("a".repeat(64))),
        ("node_id", json!("backend")),
        ("attempt", json!(2)),
        ("project_lane_sha256", json!("a".repeat(64))),
        (
            "codec_protocol_version",
            json!(GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION + 1),
        ),
        ("destination_sha256", json!("a".repeat(64))),
        ("pricing_snapshot_sha256", json!("a".repeat(64))),
        ("prepared_at_ms", json!(91)),
    ];
    for (field, value) in cases {
        let tampered = replace_event_field(fixture.inspection.clone(), field, value);
        assert!(tampered.validate().is_err(), "unbound seq-3 field: {field}");
    }
}

#[test]
fn record_expected_head_must_be_the_exact_admission_digest() {
    let mut inspection = dispatch_fixture().inspection;
    inspection.record.expected_last_event_sha256 = "a".repeat(64);
    rebind_request_event(&mut inspection);

    assert!(inspection.record.validate().is_ok());
    assert!(inspection.preparation_event.validate().is_ok());
    assert!(inspection.validate().is_err());
}

fn dispatch_fixture() -> DispatchFixture {
    let (plan, mut contract) = source_contract();
    let prepared = prepared_event(&plan, &contract);
    contract.expected_last_event_sha256 = prepared.expected_sha256().expect("prepared digest");
    resign_contract(&mut contract);
    let admission = admission_event(&contract);
    let candidate = prepare_candidate(&contract, &admission);
    let graph_run = graph_run(&plan, &contract, [&prepared, &admission, &candidate.event]);
    let contract_inspection = contract_inspection(contract, admission, graph_run);
    let record = record_from_candidate(&candidate);
    let inspection = GroupAgentNodeDispatchRequestInspection {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        record,
        provider_request_body: candidate.provider_request_body.clone(),
        preparation_event_json: candidate.event_json.clone(),
        preparation_event: candidate.event.clone(),
        contract: contract_inspection,
    };
    DispatchFixture {
        candidate,
        inspection,
    }
}

fn source_contract() -> (GroupAgentGraphCorePlan, GroupAgentNodeExecutionContract) {
    let fixture: GoldenFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("shared contract fixture");
    let snapshot: GroupAgentGraphControlSnapshot =
        serde_json::from_str(&fixture.input.canonical_control_snapshot_json).expect("snapshot");
    let contract = serde_json::from_str(&fixture.expected.canonical_contract_json)
        .expect("execution contract");
    (snapshot.plan, contract)
}

fn prepared_event(
    plan: &GroupAgentGraphCorePlan,
    contract: &GroupAgentNodeExecutionContract,
) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: contract.graph_run_id.clone(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: plan.graph_id.clone(),
            graph_manifest_sha256: plan.graph_manifest_sha256.clone(),
            plan_sha256: plan.plan_sha256.clone(),
            scheduler_protocol_version: plan.scheduler_protocol_version,
            prepared_at_ms: 73,
        },
    }
}

fn admission_event(contract: &GroupAgentNodeExecutionContract) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
        graph_run_id: contract.graph_run_id.clone(),
        seq: 2,
        kind: GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
            previous_event_sha256: contract.expected_last_event_sha256.clone(),
            control_snapshot_sha256: contract.control_snapshot_sha256.clone(),
            contract_id: contract.contract_id.clone(),
            contract_sha256: contract.contract_sha256.clone(),
            contract_bytes: contract.canonical_json().expect("contract JSON").len(),
            node_id: contract.node.node_id.clone(),
            attempt: contract.node.attempt,
            request_sha256: contract.request.request_sha256.clone(),
            project_lane_sha256: contract.node.project_lane_sha256.clone(),
            admitted_at_ms: 80,
        },
    }
}

fn prepare_candidate(
    contract: &GroupAgentNodeExecutionContract,
    admission: &GroupAgentGraphRunEvent,
) -> PrepareGroupAgentNodeDispatchRequest {
    let mut request = PrepareGroupAgentNodeDispatchRequest {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        dispatch_request_id: String::new(),
        graph_run_id: contract.graph_run_id.clone(),
        contract_id: contract.contract_id.clone(),
        contract_sha256: contract.contract_sha256.clone(),
        node_id: contract.node.node_id.clone(),
        attempt: contract.node.attempt,
        request_sha256: contract.request.request_sha256.clone(),
        project_lane_sha256: contract.node.project_lane_sha256.clone(),
        provider: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        pricing_snapshot_sha256: contract.budgets.pricing_snapshot_sha256.clone(),
        provider_request_body: BODY.to_vec(),
        provider_request_sha256: group_agent_node_provider_request_sha256(BODY),
        destination_sha256: group_agent_node_destination_sha256(
            contract.provider.kind,
            &contract.provider.endpoint,
            &contract.provider.model,
        ),
        dispatch_request_sha256: String::new(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: 2,
        expected_last_event_sha256: admission.expected_sha256().expect("admission digest"),
        event: admission.clone(),
        event_json: String::new(),
        idempotency_key: "dispatch-key".into(),
        prepared_at_ms: 90,
    };
    request.dispatch_request_sha256 = request.expected_sha256().expect("dispatch digest");
    request.dispatch_request_id =
        group_agent_node_dispatch_request_id(&request.dispatch_request_sha256);
    request.event = event_from_candidate(&request);
    request.event_json = request.event.canonical_json().expect("dispatch event JSON");
    request
}

fn event_from_candidate(request: &PrepareGroupAgentNodeDispatchRequest) -> GroupAgentGraphRunEvent {
    event_from_fields(&record_from_candidate(request))
}

fn event_from_fields(record: &GroupAgentNodeDispatchRequestRecord) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
        graph_run_id: record.graph_run_id.clone(),
        seq: 3,
        kind: GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
            previous_event_sha256: record.expected_last_event_sha256.clone(),
            contract_id: record.contract_id.clone(),
            contract_sha256: record.contract_sha256.clone(),
            dispatch_request_id: record.dispatch_request_id.clone(),
            dispatch_request_sha256: record.dispatch_request_sha256.clone(),
            request_body_sha256: record.provider_request_sha256.clone(),
            request_body_bytes: record.provider_request_bytes,
            logical_request_sha256: record.request_sha256.clone(),
            node_id: record.node_id.clone(),
            attempt: record.attempt,
            project_lane_sha256: record.project_lane_sha256.clone(),
            codec_protocol_version: record.codec_protocol_version,
            provider_kind: record.provider,
            destination_sha256: record.destination_sha256.clone(),
            pricing_snapshot_sha256: record.pricing_snapshot_sha256.clone(),
            prepared_at_ms: record.created_at_ms,
        },
    }
}

fn record_from_candidate(
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> GroupAgentNodeDispatchRequestRecord {
    GroupAgentNodeDispatchRequestRecord {
        v: request.v,
        dispatch_request_id: request.dispatch_request_id.clone(),
        graph_run_id: request.graph_run_id.clone(),
        contract_id: request.contract_id.clone(),
        node_id: request.node_id.clone(),
        attempt: request.attempt,
        contract_sha256: request.contract_sha256.clone(),
        request_sha256: request.request_sha256.clone(),
        project_lane_sha256: request.project_lane_sha256.clone(),
        provider: request.provider,
        endpoint: request.endpoint.clone(),
        model: request.model.clone(),
        pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
        provider_request_sha256: request.provider_request_sha256.clone(),
        provider_request_bytes: request.provider_request_body.len(),
        destination_sha256: request.destination_sha256.clone(),
        dispatch_request_sha256: request.dispatch_request_sha256.clone(),
        codec_protocol_version: request.codec_protocol_version,
        expected_last_event_seq: request.expected_last_event_seq,
        expected_last_event_sha256: request.expected_last_event_sha256.clone(),
        created_at_ms: request.prepared_at_ms,
    }
}

fn graph_run(
    plan: &GroupAgentGraphCorePlan,
    contract: &GroupAgentNodeExecutionContract,
    events: [&GroupAgentGraphRunEvent; 3],
) -> GroupAgentGraphRunInspection {
    let plan_json = plan.canonical_json().expect("plan JSON");
    let events: Vec<_> = events.into_iter().cloned().collect();
    let event_jsons: Vec<_> = events
        .iter()
        .map(|event| event.canonical_json().expect("event JSON"))
        .collect();
    GroupAgentGraphRunInspection {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
        run: GroupAgentGraphRunRecord {
            v: GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
            graph_run_id: contract.graph_run_id.clone(),
            graph_id: contract.graph_id.clone(),
            status: GroupAgentGraphRunStatus::AwaitingDispatchAuthorization,
            source_snapshot_sha256: contract.source_snapshot_sha256.clone(),
            graph_manifest_sha256: contract.graph_manifest_sha256.clone(),
            scheduler_protocol_version: plan.scheduler_protocol_version,
            plan_sha256: plan.plan_sha256.clone(),
            plan_bytes: plan_json.len(),
            node_count: plan.authored_node_ids.len(),
            wave_count: plan.waves.len(),
            execution_contract_present: true,
            dispatch_request_present: true,
            dispatch_authority_released: false,
            last_event_seq: 3,
            journal_bytes: event_jsons.iter().map(String::len).sum(),
            created_at_ms: 73,
        },
        plan_json,
        plan: plan.clone(),
        event_jsons,
        events,
    }
}

fn contract_inspection(
    contract: GroupAgentNodeExecutionContract,
    admission_event: GroupAgentGraphRunEvent,
    graph_run: GroupAgentGraphRunInspection,
) -> GroupAgentNodeExecutionContractInspection {
    let contract_json = contract.canonical_json().expect("contract JSON");
    let admission_event_json = admission_event.canonical_json().expect("admission JSON");
    GroupAgentNodeExecutionContractInspection {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        record: GroupAgentNodeExecutionContractRecord {
            v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
            contract_id: contract.contract_id.clone(),
            graph_run_id: contract.graph_run_id.clone(),
            node_id: contract.node.node_id.clone(),
            attempt: contract.node.attempt,
            control_snapshot_sha256: contract.control_snapshot_sha256.clone(),
            contract_sha256: contract.contract_sha256.clone(),
            contract_bytes: contract_json.len(),
            request_sha256: contract.request.request_sha256.clone(),
            project_lane_sha256: contract.node.project_lane_sha256.clone(),
            expected_last_event_seq: contract.expected_last_event_seq,
            expected_last_event_sha256: contract.expected_last_event_sha256.clone(),
            created_at_ms: 80,
        },
        contract_json,
        contract,
        admission_event_json,
        admission_event,
        graph_run,
    }
}

fn resign_contract(contract: &mut GroupAgentNodeExecutionContract) {
    let digest = contract.expected_sha256().expect("contract digest");
    contract.contract_id = format!("node-contract-{digest}");
    contract.contract_sha256 = digest;
}

fn rebind_request_event(inspection: &mut GroupAgentNodeDispatchRequestInspection) {
    inspection.record.dispatch_request_sha256 = inspection
        .record
        .expected_sha256()
        .expect("dispatch digest");
    inspection.record.dispatch_request_id =
        group_agent_node_dispatch_request_id(&inspection.record.dispatch_request_sha256);
    replace_event(inspection, event_from_fields(&inspection.record));
}

fn replace_event_field(
    mut inspection: GroupAgentNodeDispatchRequestInspection,
    field: &str,
    value: Value,
) -> GroupAgentNodeDispatchRequestInspection {
    let mut wire = serde_json::to_value(&inspection.preparation_event).expect("event value");
    wire[field] = value;
    let event = serde_json::from_value(wire).expect("tampered event");
    replace_event(&mut inspection, event);
    inspection
}

fn replace_event(
    inspection: &mut GroupAgentNodeDispatchRequestInspection,
    event: GroupAgentGraphRunEvent,
) {
    let json = event.canonical_json().expect("event JSON");
    inspection.preparation_event = event.clone();
    inspection.preparation_event_json.clone_from(&json);
    inspection.contract.graph_run.events[2] = event;
    inspection.contract.graph_run.event_jsons[2] = json;
    inspection.contract.graph_run.run.journal_bytes = inspection
        .contract
        .graph_run
        .event_jsons
        .iter()
        .map(String::len)
        .sum();
}
