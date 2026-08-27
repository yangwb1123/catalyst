use serde_json::{Map, Value, json};

use super::*;

pub(super) fn authorization(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> GroupAgentScheduledReadyNodeDispatchAuthorization {
    let mut fields = Map::new();
    merge(&mut fields, authorization_source(control));
    merge(&mut fields, authorization_execution(control));
    merge(&mut fields, authorization_state());
    fields.insert("authorization_id".into(), json!(""));
    fields.insert("authorization_sha256".into(), json!(""));
    serde_json::from_value::<GroupAgentScheduledReadyNodeDispatchAuthorization>(Value::Object(
        fields,
    ))
    .expect("ready authorization fields")
    .seal()
    .expect("seal ready authorization")
}

fn authorization_source(control: &GroupAgentScheduledReadyNodeDispatchReleaseControl) -> Value {
    let source = &control.control_snapshot;
    let contract = &control.scheduled_contract;
    let request = &control.provider_request;
    json!({
        "v": 2, "scheduler_protocol_version": 1, "dispatch_authorization_protocol_version": 2,
        "graph_run_id": source.graph_run_id, "graph_id": source.graph_id,
        "group_run_id": source.manifest.source.group_run_id, "group_id": source.manifest.source.group_id,
        "source_snapshot_sha256": source.source_snapshot_sha256,
        "graph_manifest_sha256": source.graph_manifest_sha256, "core_plan_sha256": source.core_plan_sha256,
        "control_snapshot_sha256": source.snapshot_sha256,
        "release_control_snapshot_sha256": control.snapshot_sha256,
        "progress_snapshot_sha256": control.progress_snapshot.snapshot_sha256,
        "reconcile_decision_sha256": control.reconcile_decision.decision_sha256,
        "schedule_id": contract.schedule_id, "schedule_sha256": contract.schedule_sha256,
        "scheduled_contract_id": contract.contract_id, "scheduled_contract_sha256": contract.contract_sha256,
        "scheduled_provider_request_id": request.provider_request_id,
        "scheduled_provider_request_sha256": request.prepared_request_sha256,
        "logical_request_id": request.logical_request_id, "logical_request_sha256": request.logical_request_sha256,
        "request_body_sha256": request.provider_request_sha256, "request_body_bytes": request.provider_request_bytes,
        "expected_last_event_seq": 1, "expected_last_event_sha256": source.last_event_sha256,
    })
}

fn authorization_execution(control: &GroupAgentScheduledReadyNodeDispatchReleaseControl) -> Value {
    let contract = &control.scheduled_contract;
    let request = &control.provider_request;
    json!({
        "execution_ordinal": contract.node.execution_ordinal, "node_id": contract.node.node_id,
        "attempt": 1, "project_id": contract.node.project_id,
        "project_lane_sha256": contract.node.project_lane_sha256,
        "same_project_policy": contract.node.same_project_policy,
        "provider_kind": contract.provider.kind, "endpoint": contract.provider.endpoint,
        "model": contract.provider.model, "destination_sha256": request.destination_sha256,
        "pricing_snapshot_sha256": request.pricing_snapshot_sha256, "budgets": contract.budgets,
        "release_requirements": requirements(), "maximum_future_node_releases": 1,
        "failure": contract.failure,
    })
}

fn authorization_state() -> Value {
    json!({
        "lifecycle_contract_admission_authorized": true,
        "execution_authority_release_authorized": true,
        "dispatch_authority_release_authorized": true,
        "scheduled_contract_candidate_present": true, "provider_request_prepared": true,
        "lifecycle_contract_admitted": false, "execution_authority_released": false,
        "dispatch_authority_released": false, "project_lane_claimed": false,
        "provider_request_sent": false, "progress_observed": false,
        "terminal_receipt_recorded": false, "successor_advance_authorized": false,
    })
}

fn requirements() -> GroupAgentScheduledReadyNodeDispatchReleaseRequirements {
    GroupAgentScheduledReadyNodeDispatchReleaseRequirements {
        consent: GroupAgentScheduledNodeDispatchConsentRequirement::FreshOffMachine,
        consent_contract_version: 1,
        credential_preflight:
            GroupAgentScheduledNodeDispatchCredentialPreflight::HeaderSafeEnvironment,
        destination_preflight:
            GroupAgentScheduledNodeDispatchDestinationPreflight::ExactRegisteredDestination,
        pricing_preflight:
            GroupAgentScheduledNodeDispatchPricingPreflight::ExactSnapshotWithinMaxCost,
        project_lane_claim:
            GroupAgentScheduledNodeDispatchProjectLaneClaim::GlobalExclusiveUntilTerminal,
        provider_health_check: GroupAgentScheduledNodeDispatchProviderHealthCheck::Forbidden,
        atomic_transition: GroupAgentScheduledReadyNodeDispatchAtomicTransitionRequirement::ExactProgressSnapshotSelectedNodeAdmissionReleaseAndLaneClaim,
        successor: GroupAgentScheduledReadyNodeSuccessorRequirement::ExactOrderedDirectPredecessorTerminalReceiptsBeforeSuccessor,
    }
}

fn merge(fields: &mut Map<String, Value>, value: Value) {
    let Value::Object(values) = value else {
        panic!("authorization fragment must be object");
    };
    fields.extend(values);
}
