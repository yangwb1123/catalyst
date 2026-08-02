use std::sync::Arc;

use serde_json::Value;

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphControlSnapshot, GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind,
    GroupAgentGraphRunInspection, GroupAgentGraphRunRecord, GroupAgentGraphRunStatus,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractInspection,
    GroupAgentScheduledNodeContractRecord, GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentScheduledNodeProviderRequestRecord, Message,
    PrepareGroupAgentScheduledNodeProviderRequestDisposition,
    PrepareGroupAgentScheduledNodeProviderRequestResult,
};

use super::{
    GroupAgentScheduledNodeProviderRequestService,
    GroupAgentScheduledNodeProviderRequestServiceError,
    PrepareGroupAgentScheduledNodeProviderRequestInput,
    scheduled_provider_request_validation::{
        model_request, prepare_request, validate_list, validate_pristine_run, validate_result,
    },
};

#[path = "scheduled_provider_request_test_store.rs"]
mod store;
use store::{SpyCodec, SpyHub};

const BODY: &[u8] = br#"{"model":"fixture"}"#;

#[test]
fn preflight_rejects_invalid_values_without_a_service_or_store() {
    let valid = input();
    GroupAgentScheduledNodeProviderRequestService::preflight_prepare(&valid).expect("valid input");
    GroupAgentScheduledNodeProviderRequestService::preflight_inspect(
        "scheduled-node-provider-request-valid",
    )
    .expect("valid request ID");
    GroupAgentScheduledNodeProviderRequestService::preflight_list(Some("graph-run-valid"), 1)
        .expect("valid list");

    for invalid in [
        PrepareGroupAgentScheduledNodeProviderRequestInput {
            scheduled_contract_id: String::new(),
            ..valid.clone()
        },
        PrepareGroupAgentScheduledNodeProviderRequestInput {
            idempotency_key: String::new(),
            ..valid.clone()
        },
        PrepareGroupAgentScheduledNodeProviderRequestInput {
            prepared_at_ms: u64::MAX,
            ..valid
        },
    ] {
        assert!(matches!(
            GroupAgentScheduledNodeProviderRequestService::preflight_prepare(&invalid),
            Err(GroupAgentScheduledNodeProviderRequestServiceError::InvalidInput { .. })
        ));
    }
    assert!(GroupAgentScheduledNodeProviderRequestService::preflight_inspect("").is_err());
    assert!(GroupAgentScheduledNodeProviderRequestService::preflight_list(None, 0).is_err());
}

#[test]
fn service_uses_only_the_pure_codec_and_persists_its_exact_bytes() {
    let hub = Arc::new(SpyHub::new());
    let codec = Arc::new(SpyCodec::new(BODY.to_vec()));
    let service = GroupAgentScheduledNodeProviderRequestService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        codec.clone(),
    );

    let mut request = input();
    request.scheduled_contract_id = hub.contract_id();
    let result = service.prepare(&request).expect("prepare through service");

    assert_eq!(
        codec.calls(),
        ["encode", "validate_exact", "validate_exact"]
    );
    assert_eq!(hub.captured_body(), BODY);
    assert_eq!(result.inspection.provider_request_body, BODY);
}

#[test]
fn model_and_preparation_are_derived_only_from_the_validated_candidate() {
    let source = source();
    let model = model_request(&source.candidate);
    assert_eq!(model.system_prompt, source.candidate.request.system_prompt);
    assert_eq!(
        model.messages,
        vec![Message::User {
            text: source.candidate.request.user_prompt.clone()
        }]
    );
    assert!(model.tools.is_empty());
    assert_eq!(
        model.max_output_tokens,
        source.candidate.budgets.max_output_tokens
    );

    let request = prepare_request(&input(), &source, BODY.to_vec()).expect("prepare request");
    request.validate().expect("valid domain request");
    assert_eq!(request.provider_request_body, BODY);
    assert_eq!(request.graph_run_id, source.record.graph_run_id);
    assert_eq!(request.schedule_id, source.record.schedule_id);
    assert_eq!(request.scheduled_contract_id, source.record.contract_id);
    assert_eq!(request.logical_request_id, source.record.request_id);
    assert!(request.provider_request_prepared);
    assert!(!request.provider_request_sent);
    assert!(!request.lifecycle_contract_admitted);
    assert!(!request.execution_authority_released);
    assert!(!request.dispatch_authority_released);
    assert!(!request.project_lane_claimed);
    assert!(!request.progress_observed);
    assert!(!request.successor_advance_authorized);
}

#[test]
fn result_validation_preserves_exact_bytes_source_and_created_time() {
    let source = source();
    let request = prepare_request(&input(), &source, BODY.to_vec()).unwrap();
    let valid = result(&request, source.clone());
    validate_result(&request, &source, valid).expect("valid created result");

    let mut body_drift = result(&request, source.clone());
    body_drift.inspection.provider_request_body.push(b'\n');
    assert!(validate_result(&request, &source, body_drift).is_err());

    let mut time_drift = result(&request, source.clone());
    time_drift.inspection.record.created_at_ms += 1;
    assert!(validate_result(&request, &source, time_drift).is_err());

    let mut source_drift = result(&request, source.clone());
    source_drift
        .inspection
        .scheduled_contract
        .record
        .created_at_ms += 1;
    assert!(validate_result(&request, &source, source_drift).is_err());
}

#[test]
fn list_validation_rejects_duplicates_and_unfiltered_rows() {
    let source = source();
    let request = prepare_request(&input(), &source, BODY.to_vec()).unwrap();
    let record = record(&request);
    validate_list(std::slice::from_ref(&record), Some(&record.graph_run_id), 1)
        .expect("one exact record");
    assert!(validate_list(&[record.clone(), record.clone()], None, 2).is_err());
    assert!(validate_list(&[record], Some("another-run"), 1).is_err());
}

#[test]
fn pristine_run_check_rejects_a_locally_valid_advanced_run() {
    let pristine = pristine_run();
    validate_pristine_run(&pristine.run.graph_run_id.clone(), pristine.clone())
        .expect("pristine Run");

    let mut advanced = pristine;
    advanced.run.status = GroupAgentGraphRunStatus::AwaitingCoreDispatch;
    assert!(
        validate_pristine_run(&advanced.run.graph_run_id.clone(), advanced).is_err(),
        "even projection drift must fail before encoding"
    );
}

fn input() -> PrepareGroupAgentScheduledNodeProviderRequestInput {
    PrepareGroupAgentScheduledNodeProviderRequestInput {
        scheduled_contract_id:
            "scheduled-node-contract-324169fceacde6aeb41f764332043bb236b80aee4fc57420c0122130679cc3a7"
                .into(),
        idempotency_key: "scheduled-provider-request-key".into(),
        prepared_at_ms: 90,
    }
}

fn source() -> GroupAgentScheduledNodeContractInspection {
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
        record: contract_record(&candidate, candidate_json.len()),
        candidate_json,
        candidate,
    }
}

fn contract_record(
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
        created_at_ms: 80,
    }
}

fn result(
    request: &crate::runtime_domain::PrepareGroupAgentScheduledNodeProviderRequest,
    source: GroupAgentScheduledNodeContractInspection,
) -> PrepareGroupAgentScheduledNodeProviderRequestResult {
    PrepareGroupAgentScheduledNodeProviderRequestResult {
        v: request.v,
        disposition: PrepareGroupAgentScheduledNodeProviderRequestDisposition::Created,
        inspection: GroupAgentScheduledNodeProviderRequestInspection {
            v: request.v,
            record: record(request),
            provider_request_body: request.provider_request_body.clone(),
            scheduled_contract: source,
        },
    }
}

fn record(
    request: &crate::runtime_domain::PrepareGroupAgentScheduledNodeProviderRequest,
) -> GroupAgentScheduledNodeProviderRequestRecord {
    GroupAgentScheduledNodeProviderRequestRecord {
        v: request.v,
        provider_request_id: request.provider_request_id.clone(),
        graph_run_id: request.graph_run_id.clone(),
        schedule_id: request.schedule_id.clone(),
        scheduled_contract_id: request.scheduled_contract_id.clone(),
        execution_ordinal: request.execution_ordinal,
        node_id: request.node_id.clone(),
        attempt: request.attempt,
        scheduled_contract_sha256: request.scheduled_contract_sha256.clone(),
        logical_request_id: request.logical_request_id.clone(),
        logical_request_sha256: request.logical_request_sha256.clone(),
        schedule_sha256: request.schedule_sha256.clone(),
        project_lane_sha256: request.project_lane_sha256.clone(),
        provider: request.provider,
        endpoint: request.endpoint.clone(),
        model: request.model.clone(),
        destination_sha256: request.destination_sha256.clone(),
        pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
        provider_request_sha256: request.provider_request_sha256.clone(),
        provider_request_bytes: request.provider_request_body.len(),
        prepared_request_sha256: request.prepared_request_sha256.clone(),
        codec_protocol_version: request.codec_protocol_version,
        expected_last_event_seq: request.expected_last_event_seq,
        expected_last_event_sha256: request.expected_last_event_sha256.clone(),
        provider_request_prepared: request.provider_request_prepared,
        provider_request_sent: request.provider_request_sent,
        lifecycle_contract_admitted: request.lifecycle_contract_admitted,
        execution_authority_released: request.execution_authority_released,
        dispatch_authority_released: request.dispatch_authority_released,
        project_lane_claimed: request.project_lane_claimed,
        progress_observed: request.progress_observed,
        successor_advance_authorized: request.successor_advance_authorized,
        created_at_ms: request.prepared_at_ms,
    }
}

fn pristine_run() -> GroupAgentGraphRunInspection {
    let mut control = control_fixture();
    let event = prepared_event(&control);
    control.last_event_sha256 = event.expected_sha256().unwrap();
    control.snapshot_sha256 = control.expected_sha256().unwrap();
    let event_json = event.canonical_json().unwrap();
    let plan_json = control.plan.canonical_json().unwrap();
    GroupAgentGraphRunInspection {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        run: pristine_run_record(&control, plan_json.len(), event_json.len()),
        plan_json,
        plan: control.plan,
        event_jsons: vec![event_json],
        events: vec![event],
    }
}

fn control_fixture() -> GroupAgentGraphControlSnapshot {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("control fixture");
    let control_json = fixture["input"]["canonical_control_snapshot_json"]
        .as_str()
        .expect("control JSON");
    serde_json::from_str(control_json).expect("control")
}

fn prepared_event(control: &GroupAgentGraphControlSnapshot) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: control.graph_run_id.clone(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: control.graph_id.clone(),
            graph_manifest_sha256: control.graph_manifest_sha256.clone(),
            plan_sha256: control.core_plan_sha256.clone(),
            scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
            prepared_at_ms: 90,
        },
    }
}

fn pristine_run_record(
    control: &GroupAgentGraphControlSnapshot,
    plan_bytes: usize,
    journal_bytes: usize,
) -> GroupAgentGraphRunRecord {
    GroupAgentGraphRunRecord {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: control.graph_run_id.clone(),
        graph_id: control.graph_id.clone(),
        status: GroupAgentGraphRunStatus::AwaitingExecutionContract,
        source_snapshot_sha256: control.source_snapshot_sha256.clone(),
        graph_manifest_sha256: control.graph_manifest_sha256.clone(),
        scheduler_protocol_version: control.scheduler_protocol_version,
        plan_sha256: control.core_plan_sha256.clone(),
        plan_bytes,
        node_count: control.plan.authored_node_ids.len(),
        wave_count: control.plan.waves.len(),
        execution_contract_present: false,
        dispatch_request_present: false,
        dispatch_authority_released: false,
        last_event_seq: 1,
        journal_bytes,
        created_at_ms: 90,
    }
}
