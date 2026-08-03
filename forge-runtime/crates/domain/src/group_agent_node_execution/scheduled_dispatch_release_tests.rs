use serde_json::Value;

use super::*;
use crate::{
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION, GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunStatus, group_agent_node_destination_sha256,
    group_agent_node_provider_request_sha256, group_agent_scheduled_node_provider_request_id,
};

#[path = "scheduled_dispatch_release_test_mutations.rs"]
mod mutation_support;
use mutation_support::{authorization_mutations, cross_source_mutations};
#[path = "scheduled_dispatch_release_test_authorization.rs"]
mod authorization_support;
use authorization_support::{authorization, resign_authorization};

const BODY: &str = "{}";

#[test]
fn exact_release_control_and_authorization_are_fully_bound() {
    let control = release_control();
    control.validate().expect("valid release control");
    assert_eq!(
        GroupAgentScheduledNodeDispatchReleaseControl::decode_exact(
            &control.canonical_json().unwrap()
        )
        .unwrap(),
        control
    );

    let authorization = authorization(&control);
    authorization.validate().expect("valid authorization");
    authorization
        .validate_against_release_control(&control)
        .expect("authorization bound to control");
    assert_eq!(
        GroupAgentScheduledNodeDispatchAuthorization::decode_exact(
            &authorization.canonical_json().unwrap()
        )
        .unwrap(),
        authorization
    );
}

#[test]
fn domains_bounds_identity_and_policy_are_frozen() {
    assert_eq!(
        GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN,
        b"forge.group-agent-scheduled-node-dispatch-release-control.v1\0"
    );
    assert_eq!(
        GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN,
        b"forge.group-agent-scheduled-node-dispatch-authorization.v1\0"
    );
    assert_eq!(
        MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_BYTES,
        64 * 1024 * 1024
    );
    assert_eq!(
        MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES,
        1024 * 1024
    );
    let authorization = authorization(&release_control());
    assert_eq!(
        authorization.authorization_id,
        group_agent_scheduled_node_dispatch_authorization_id(&authorization.authorization_sha256)
    );
    let payload = authorization.canonical_payload_json().unwrap();
    assert!(
        payload.contains(
            r#""atomic_transition":"exact_pristine_head_admission_release_and_lane_claim""#
        )
    );
    assert!(
        payload
            .contains(r#""successor":"verified_intermediate_terminal_receipt_before_successor""#)
    );
}

#[test]
fn stale_authorization_identity_rejects_every_binding_category() {
    let control = release_control();
    let original = authorization(&control);
    let digest = original.expected_sha256().unwrap();
    for (name, mutation) in authorization_mutations() {
        let mut changed = original.clone();
        mutation(&mut changed);
        assert_ne!(changed.expected_sha256().unwrap(), digest, "unbound {name}");
        assert!(
            changed.validate().is_err(),
            "stale identity accepted: {name}"
        );
    }
}

#[test]
fn self_consistent_cross_source_substitutions_are_rejected() {
    let control = release_control();
    for (name, mutation) in cross_source_mutations() {
        let mut changed = authorization(&control);
        mutation(&mut changed);
        resign_authorization(&mut changed);
        changed
            .validate()
            .expect("intrinsically valid substitution");
        assert!(
            changed.validate_against_release_control(&control).is_err(),
            "cross-source substitution accepted: {name}"
        );
    }
}

#[test]
fn release_control_rejects_body_record_and_stored_source_tampering() {
    let original = release_control();
    let mut body = original.clone();
    body.provider_request_json.push(' ');
    resign_control(&mut body);
    assert!(body.validate().is_err());

    let mut schedule_record = original.clone();
    schedule_record.schedule_record.schedule_bytes += 1;
    resign_control(&mut schedule_record);
    assert!(schedule_record.validate().is_err());

    let mut journal = original;
    journal.journal_events[0].graph_run_id.push('x');
    resign_control(&mut journal);
    assert!(journal.validate().is_err());
}

#[test]
fn decoders_reject_unknown_noncanonical_and_oversized_input() {
    let control_json = release_control().canonical_json().unwrap();
    let unknown = control_json.replacen("{\"v\":1", "{\"v\":1,\"unknown\":0", 1);
    assert!(GroupAgentScheduledNodeDispatchReleaseControl::decode_exact(&unknown).is_err());
    assert!(
        GroupAgentScheduledNodeDispatchReleaseControl::decode_exact(&(control_json + "\n"))
            .is_err()
    );
    let authorization_json = authorization(&release_control()).canonical_json().unwrap();
    let escaped =
        authorization_json.replacen("group-run-fixture-v1", "group-run-\\u0066ixture-v1", 1);
    assert!(GroupAgentScheduledNodeDispatchAuthorization::decode_exact(&escaped).is_err());
    assert!(
        GroupAgentScheduledNodeDispatchAuthorization::decode_exact(
            &" ".repeat(MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES + 1)
        )
        .is_err()
    );
}

fn release_control() -> GroupAgentScheduledNodeDispatchReleaseControl {
    let (mut snapshot, event) = graph_source();
    snapshot.last_event_sha256 = event.expected_sha256().unwrap();
    snapshot.snapshot_sha256 = snapshot.expected_sha256().unwrap();
    let mut schedule = schedule_fixture();
    schedule.control_snapshot_sha256 = snapshot.snapshot_sha256.clone();
    schedule.expected_last_event_sha256 = snapshot.last_event_sha256.clone();
    resign_schedule(&mut schedule);
    let mut contract = contract_fixture();
    bind_and_resign_contract(&mut contract, &snapshot, &schedule);
    let provider_request = provider_request(&contract);
    let graph_run = graph_run(&snapshot, &event);
    let schedule_record = schedule_record(&schedule);
    let scheduled_contract_record = contract_record(&contract);
    let mut control = GroupAgentScheduledNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run,
        journal_events: vec![event],
        control_snapshot: snapshot,
        schedule_record,
        schedule,
        scheduled_contract_record,
        scheduled_contract: contract,
        provider_request,
        provider_request_json: BODY.into(),
        snapshot_sha256: String::new(),
    };
    resign_control(&mut control);
    control
}

fn graph_source() -> (GroupAgentGraphControlSnapshot, GroupAgentGraphRunEvent) {
    let fixture = scheduled_fixture();
    let json = fixture["input"]["canonical_control_snapshot_json"]
        .as_str()
        .unwrap();
    let snapshot: GroupAgentGraphControlSnapshot = serde_json::from_str(json).unwrap();
    let event = GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: snapshot.graph_run_id.clone(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: snapshot.graph_id.clone(),
            graph_manifest_sha256: snapshot.graph_manifest_sha256.clone(),
            plan_sha256: snapshot.core_plan_sha256.clone(),
            scheduler_protocol_version: snapshot.scheduler_protocol_version,
            prepared_at_ms: 73,
        },
    };
    (snapshot, event)
}

fn graph_run(
    snapshot: &GroupAgentGraphControlSnapshot,
    event: &GroupAgentGraphRunEvent,
) -> GroupAgentGraphRunRecord {
    GroupAgentGraphRunRecord {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: snapshot.graph_run_id.clone(),
        graph_id: snapshot.graph_id.clone(),
        status: GroupAgentGraphRunStatus::AwaitingExecutionContract,
        source_snapshot_sha256: snapshot.source_snapshot_sha256.clone(),
        graph_manifest_sha256: snapshot.graph_manifest_sha256.clone(),
        scheduler_protocol_version: snapshot.scheduler_protocol_version,
        plan_sha256: snapshot.core_plan_sha256.clone(),
        plan_bytes: snapshot.plan.canonical_json().unwrap().len(),
        node_count: snapshot.plan.authored_node_ids.len(),
        wave_count: snapshot.plan.waves.len(),
        execution_contract_present: false,
        dispatch_request_present: false,
        dispatch_authority_released: false,
        last_event_seq: 1,
        journal_bytes: event.canonical_json().unwrap().len(),
        created_at_ms: 73,
    }
}

fn schedule_fixture() -> GroupAgentGraphExecutionSchedule {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-graph-execution-schedule-v1.json"
    )))
    .unwrap();
    GroupAgentGraphExecutionSchedule::decode_exact(
        fixture["canonical_execution_schedule_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

fn contract_fixture() -> GroupAgentScheduledNodeContractCandidate {
    let fixture = scheduled_fixture();
    GroupAgentScheduledNodeContractCandidate::decode_exact(
        fixture["expected"]["canonical_contract_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

fn scheduled_fixture() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    )))
    .unwrap()
}

fn resign_schedule(schedule: &mut GroupAgentGraphExecutionSchedule) {
    let sha256 = schedule.expected_sha256().unwrap();
    schedule.schedule_id = format!("graph-execution-schedule-{sha256}");
    schedule.schedule_sha256 = sha256;
}

fn bind_and_resign_contract(
    contract: &mut GroupAgentScheduledNodeContractCandidate,
    snapshot: &GroupAgentGraphControlSnapshot,
    schedule: &GroupAgentGraphExecutionSchedule,
) {
    contract.control_snapshot_sha256 = snapshot.snapshot_sha256.clone();
    contract.schedule_id = schedule.schedule_id.clone();
    contract.schedule_sha256 = schedule.schedule_sha256.clone();
    contract.expected_last_event_sha256 = snapshot.last_event_sha256.clone();
    contract.request.schedule_id = schedule.schedule_id.clone();
    contract.request.schedule_sha256 = schedule.schedule_sha256.clone();
    let request_sha256 = contract.request.expected_sha256().unwrap();
    contract.request.request_id = format!("scheduled-node-request-{request_sha256}");
    contract.request.request_sha256 = request_sha256;
    let contract_sha256 = contract.expected_sha256().unwrap();
    contract.contract_id = format!("scheduled-node-contract-{contract_sha256}");
    contract.contract_sha256 = contract_sha256;
}

fn schedule_record(
    schedule: &GroupAgentGraphExecutionSchedule,
) -> GroupAgentGraphExecutionScheduleRecord {
    GroupAgentGraphExecutionScheduleRecord {
        v: schedule.v,
        schedule_id: schedule.schedule_id.clone(),
        graph_run_id: schedule.graph_run_id.clone(),
        graph_id: schedule.graph_id.clone(),
        control_snapshot_sha256: schedule.control_snapshot_sha256.clone(),
        schedule_sha256: schedule.schedule_sha256.clone(),
        schedule_bytes: schedule.canonical_json().unwrap().len(),
        node_count: schedule.node_count,
        wave_count: schedule.wave_count,
        expected_last_event_seq: schedule.expected_last_event_seq,
        expected_last_event_sha256: schedule.expected_last_event_sha256.clone(),
        execution_contract_present: false,
        dispatch_authority_released: false,
        created_at_ms: 74,
    }
}

fn contract_record(
    contract: &GroupAgentScheduledNodeContractCandidate,
) -> GroupAgentScheduledNodeContractRecord {
    GroupAgentScheduledNodeContractRecord {
        v: contract.v,
        contract_id: contract.contract_id.clone(),
        graph_run_id: contract.graph_run_id.clone(),
        schedule_id: contract.schedule_id.clone(),
        node_id: contract.node.node_id.clone(),
        execution_ordinal: 0,
        attempt: 1,
        control_snapshot_sha256: contract.control_snapshot_sha256.clone(),
        schedule_sha256: contract.schedule_sha256.clone(),
        contract_sha256: contract.contract_sha256.clone(),
        contract_bytes: contract.canonical_json().unwrap().len(),
        request_id: contract.request.request_id.clone(),
        request_sha256: contract.request.request_sha256.clone(),
        project_lane_sha256: contract.node.project_lane_sha256.clone(),
        expected_last_event_seq: 1,
        expected_last_event_sha256: contract.expected_last_event_sha256.clone(),
        predecessor_receipt_count: 0,
        lifecycle_contract_admitted: false,
        provider_request_present: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        progress_observed: false,
        successor_advance_authorized: false,
        created_at_ms: 75,
    }
}

fn provider_request(
    contract: &GroupAgentScheduledNodeContractCandidate,
) -> GroupAgentScheduledNodeProviderRequestRecord {
    let mut record = GroupAgentScheduledNodeProviderRequestRecord {
        v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        provider_request_id: String::new(),
        graph_run_id: contract.graph_run_id.clone(),
        schedule_id: contract.schedule_id.clone(),
        scheduled_contract_id: contract.contract_id.clone(),
        execution_ordinal: 0,
        node_id: contract.node.node_id.clone(),
        attempt: 1,
        scheduled_contract_sha256: contract.contract_sha256.clone(),
        logical_request_id: contract.request.request_id.clone(),
        logical_request_sha256: contract.request.request_sha256.clone(),
        schedule_sha256: contract.schedule_sha256.clone(),
        project_lane_sha256: contract.node.project_lane_sha256.clone(),
        provider: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        destination_sha256: group_agent_node_destination_sha256(
            contract.provider.kind,
            &contract.provider.endpoint,
            &contract.provider.model,
        ),
        pricing_snapshot_sha256: contract.budgets.pricing_snapshot_sha256.clone(),
        provider_request_sha256: group_agent_node_provider_request_sha256(BODY.as_bytes()),
        provider_request_bytes: BODY.len(),
        prepared_request_sha256: String::new(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: 1,
        expected_last_event_sha256: contract.expected_last_event_sha256.clone(),
        provider_request_prepared: true,
        provider_request_sent: false,
        lifecycle_contract_admitted: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        project_lane_claimed: false,
        progress_observed: false,
        successor_advance_authorized: false,
        created_at_ms: 76,
    };
    record.prepared_request_sha256 = record.expected_sha256().unwrap();
    record.provider_request_id =
        group_agent_scheduled_node_provider_request_id(&record.prepared_request_sha256);
    record
}

fn resign_control(value: &mut GroupAgentScheduledNodeDispatchReleaseControl) {
    value.snapshot_sha256 = value.expected_sha256().unwrap();
}
