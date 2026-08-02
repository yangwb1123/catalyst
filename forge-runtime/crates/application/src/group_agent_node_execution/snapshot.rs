use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION, GROUP_AGENT_GRAPH_RUN_VERSION,
    GroupAgentGraphControlSnapshot, GroupAgentGraphInspection, GroupAgentGraphRunInspection,
    GroupAgentGraphRunStatus,
};

use super::{
    ExportGroupAgentGraphControl, GroupAgentNodeExecutionContractServiceError,
    error::{conflict, corrupt},
};

pub(super) fn export(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<ExportGroupAgentGraphControl, GroupAgentNodeExecutionContractServiceError> {
    require_base_run(run)?;
    reconstruct_base(run, graph)
}

pub(super) fn for_admission(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<ExportGroupAgentGraphControl, GroupAgentNodeExecutionContractServiceError> {
    require_admission_state(run)?;
    reconstruct_base(run, graph)
}

pub(super) fn historical_base(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<ExportGroupAgentGraphControl, GroupAgentNodeExecutionContractServiceError> {
    reconstruct_base(run, graph)
}

fn reconstruct_base(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<ExportGroupAgentGraphControl, GroupAgentNodeExecutionContractServiceError> {
    validate_source_binding(run, graph)?;
    let last_event_sha256 = run.events[0]
        .expected_sha256()
        .map_err(|error| corrupt(&error.to_string()))?;
    let mut snapshot = GroupAgentGraphControlSnapshot {
        v: GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION,
        scheduler_protocol_version: run.run.scheduler_protocol_version,
        graph_run_version: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: run.run.graph_run_id.clone(),
        graph_id: run.run.graph_id.clone(),
        source_snapshot_sha256: run.run.source_snapshot_sha256.clone(),
        graph_manifest_sha256: run.run.graph_manifest_sha256.clone(),
        core_plan_sha256: run.run.plan_sha256.clone(),
        last_event_seq: 1,
        last_event_sha256,
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan: run.plan.clone(),
        manifest: graph.manifest.clone(),
        snapshot_sha256: String::new(),
    };
    snapshot.snapshot_sha256 = snapshot
        .expected_sha256()
        .map_err(|error| corrupt(&error.to_string()))?;
    snapshot
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    let snapshot_json = snapshot
        .canonical_json()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(ExportGroupAgentGraphControl {
        snapshot,
        snapshot_json,
    })
}

pub(super) fn require_base_run(
    inspection: &GroupAgentGraphRunInspection,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
    is_base_run(inspection).then_some(()).ok_or_else(|| {
        conflict("control export requires the exact v1 awaiting-execution-contract state")
    })
}

fn is_base_run(inspection: &GroupAgentGraphRunInspection) -> bool {
    let run = &inspection.run;
    run.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && inspection.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingExecutionContract
        && !run.execution_contract_present
        && !run.dispatch_request_present
        && !run.dispatch_authority_released
        && run.last_event_seq == 1
        && inspection.events.len() == 1
        && inspection.event_jsons.len() == 1
}

fn require_admission_state(
    inspection: &GroupAgentGraphRunInspection,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
    let run = &inspection.run;
    let admitted = run.v == crate::runtime_domain::GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION
        && inspection.v == crate::runtime_domain::GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingCoreDispatch
        && run.execution_contract_present
        && !run.dispatch_request_present
        && !run.dispatch_authority_released
        && run.last_event_seq == 2
        && inspection.events.len() == 2
        && inspection.event_jsons.len() == 2;
    let prepared = run.v == crate::runtime_domain::GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION
        && inspection.v == crate::runtime_domain::GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingDispatchAuthorization
        && run.execution_contract_present
        && run.dispatch_request_present
        && !run.dispatch_authority_released
        && run.last_event_seq == 3
        && inspection.events.len() == 3
        && inspection.event_jsons.len() == 3;
    (is_base_run(inspection) || admitted || prepared)
        .then_some(())
        .ok_or_else(|| conflict("admission requires an exact v1, v2, or v3 passive state"))
}

pub(super) fn validate_source_binding(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
    let authored = graph
        .manifest
        .nodes
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<Vec<_>>();
    let planned = run
        .plan
        .authored_node_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let valid = graph.graph.graph_id == run.run.graph_id
        && graph.graph.source_snapshot_sha256 == run.run.source_snapshot_sha256
        && graph.graph.manifest_sha256 == run.run.graph_manifest_sha256
        && run.plan.graph_id == run.run.graph_id
        && run.plan.graph_manifest_sha256 == graph.graph.manifest_sha256
        && run.plan.plan_sha256 == run.run.plan_sha256
        && authored == planned
        && graph.manifest.edges == run.plan.edges
        && graph.manifest.waves == run.plan.waves;
    valid
        .then_some(())
        .ok_or_else(|| corrupt("Graph Run, Core Plan, and source Graph bindings disagree"))
}
