use crate::runtime_domain::{
    GroupAgentNodePricingSnapshot, GroupAgentScheduledNodeDispatchAuthorization,
    GroupAgentScheduledNodeDispatchReleaseControl,
    group_agent_scheduled_node_dispatch_authorization_id,
};

pub(super) fn authorization(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
    pricing: &GroupAgentNodePricingSnapshot,
) -> GroupAgentScheduledNodeDispatchAuthorization {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json"
    )))
    .expect("scheduled authorization fixture");
    let json = fixture["canonical_authorization_json"]
        .as_str()
        .expect("scheduled authorization fixture JSON");
    let mut value = GroupAgentScheduledNodeDispatchAuthorization::decode_exact(json)
        .expect("decode scheduled authorization fixture");
    bind_source(&mut value, control);
    bind_execution(&mut value, control, pricing);
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_scheduled_node_dispatch_authorization_id(&value.authorization_sha256);
    value
        .validate_against_release_control(control)
        .expect("authorization bound to release control");
    value
}

fn bind_source(
    value: &mut GroupAgentScheduledNodeDispatchAuthorization,
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) {
    let source = &control.control_snapshot;
    let request = &control.provider_request;
    value.graph_run_id.clone_from(&source.graph_run_id);
    value.graph_id.clone_from(&source.graph_id);
    value
        .group_run_id
        .clone_from(&source.manifest.source.group_run_id);
    value.group_id.clone_from(&source.manifest.source.group_id);
    value
        .source_snapshot_sha256
        .clone_from(&source.source_snapshot_sha256);
    value
        .graph_manifest_sha256
        .clone_from(&source.graph_manifest_sha256);
    value.core_plan_sha256.clone_from(&source.core_plan_sha256);
    value
        .control_snapshot_sha256
        .clone_from(&source.snapshot_sha256);
    value
        .release_control_snapshot_sha256
        .clone_from(&control.snapshot_sha256);
    value.schedule_id.clone_from(&control.schedule.schedule_id);
    value
        .schedule_sha256
        .clone_from(&control.schedule.schedule_sha256);
    bind_request_source(value, control, request);
}

fn bind_request_source(
    value: &mut GroupAgentScheduledNodeDispatchAuthorization,
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
    request: &crate::runtime_domain::GroupAgentScheduledNodeProviderRequestRecord,
) {
    value
        .scheduled_contract_id
        .clone_from(&control.scheduled_contract.contract_id);
    value
        .scheduled_contract_sha256
        .clone_from(&control.scheduled_contract.contract_sha256);
    value
        .scheduled_provider_request_id
        .clone_from(&request.provider_request_id);
    value
        .scheduled_provider_request_sha256
        .clone_from(&request.prepared_request_sha256);
    value
        .logical_request_id
        .clone_from(&request.logical_request_id);
    value
        .logical_request_sha256
        .clone_from(&request.logical_request_sha256);
    value
        .request_body_sha256
        .clone_from(&request.provider_request_sha256);
    value.request_body_bytes = request.provider_request_bytes;
    value.expected_last_event_seq = control.control_snapshot.last_event_seq;
    value
        .expected_last_event_sha256
        .clone_from(&control.control_snapshot.last_event_sha256);
}

fn bind_execution(
    value: &mut GroupAgentScheduledNodeDispatchAuthorization,
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
    pricing: &GroupAgentNodePricingSnapshot,
) {
    let contract = &control.scheduled_contract;
    let request = &control.provider_request;
    value.execution_ordinal = contract.node.execution_ordinal;
    value.node_id.clone_from(&contract.node.node_id);
    value.attempt = contract.node.attempt;
    value.project_id.clone_from(&contract.node.project_id);
    value
        .project_lane_sha256
        .clone_from(&contract.node.project_lane_sha256);
    value.same_project_policy = contract.node.same_project_policy;
    value.provider_kind = contract.provider.kind;
    value.endpoint.clone_from(&contract.provider.endpoint);
    value.model.clone_from(&contract.provider.model);
    value
        .destination_sha256
        .clone_from(&request.destination_sha256);
    value
        .pricing_snapshot_sha256
        .clone_from(&pricing.pricing_snapshot_sha256);
    value.budgets.clone_from(&contract.budgets);
    value.failure.clone_from(&contract.failure);
}
