use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION, GROUP_AGENT_GRAPH_RUN_VERSION,
    GroupAgentGraphControlSnapshot, GroupAgentGraphInspection, GroupAgentGraphRunInspection,
    HubEntity, HubStoreError,
};

pub(super) fn reconstruct(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<(GroupAgentGraphControlSnapshot, String), HubStoreError> {
    validate_binding(run, graph)?;
    let first_event = run
        .events
        .first()
        .ok_or_else(|| corrupt("stored Graph Run has no control-snapshot head"))?;
    let last_event_sha256 = first_event
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
    let json = snapshot
        .canonical_json()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok((snapshot, json))
}

pub(super) fn verify_candidate(
    expected: &(GroupAgentGraphControlSnapshot, String),
    snapshot: &GroupAgentGraphControlSnapshot,
    json: &str,
) -> Result<(), HubStoreError> {
    if expected.0 == *snapshot && expected.1.as_bytes() == json.as_bytes() {
        Ok(())
    } else {
        Err(conflict(
            "control snapshot does not exactly match the stored Graph Run",
        ))
    }
}

fn validate_binding(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<(), HubStoreError> {
    let exact = run.run.graph_id == graph.graph.graph_id
        && run.run.source_snapshot_sha256 == graph.graph.source_snapshot_sha256
        && run.run.graph_manifest_sha256 == graph.graph.manifest_sha256
        && run.plan.graph_id == graph.graph.graph_id
        && run.plan.graph_manifest_sha256 == graph.graph.manifest_sha256;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("stored Graph Run and Graph snapshot binding disagrees"))
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentNodeExecutionContract,
        message: message.into(),
    }
}
