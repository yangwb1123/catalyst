use std::collections::BTreeSet;

use crate::group_agent_graph_run_service::{
    BeginGroupAgentGraphRun, BeginGroupAgentGraphRunDisposition, BeginGroupAgentGraphRunResult,
    GROUP_AGENT_GRAPH_RUN_VERSION, GroupAgentGraphCorePlan, GroupAgentGraphInspection,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection,
    GroupAgentGraphRunRecord, MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_RUN_LIST_LIMIT,
};

use crate::{
    GroupAgentGraphRunServiceError, PrepareGroupAgentGraphRunInput,
    group_agent_graph_validation::checked_inspection,
};

pub(crate) fn validate_prepare_input(
    input: &PrepareGroupAgentGraphRunInput,
) -> Result<GroupAgentGraphCorePlan, GroupAgentGraphRunServiceError> {
    validate_identifier(&input.graph_run_id)?;
    validate_identifier(&input.graph_id)?;
    if !valid_text(
        &input.idempotency_key,
        MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    ) || i64::try_from(input.created_at_ms).is_err()
        || input.plan_json.is_empty()
        || input.plan_json.len() > MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES
    {
        return Err(GroupAgentGraphRunServiceError::InvalidInput);
    }
    let plan: GroupAgentGraphCorePlan = serde_json::from_str(&input.plan_json)
        .map_err(|_| GroupAgentGraphRunServiceError::InvalidPlan)?;
    plan.validate()
        .map_err(|_| GroupAgentGraphRunServiceError::InvalidPlan)?;
    if plan.graph_id != input.graph_id
        || plan.canonical_json().as_deref() != Ok(input.plan_json.as_str())
    {
        return Err(GroupAgentGraphRunServiceError::InvalidPlan);
    }
    Ok(plan)
}

pub(crate) fn checked_graph(
    inspection: GroupAgentGraphInspection,
) -> Result<GroupAgentGraphInspection, GroupAgentGraphRunServiceError> {
    checked_inspection(inspection).map_err(|_| GroupAgentGraphRunServiceError::InvalidGraph)
}

pub(crate) fn checked_run(
    inspection: GroupAgentGraphRunInspection,
) -> Result<GroupAgentGraphRunInspection, GroupAgentGraphRunServiceError> {
    inspection
        .validate()
        .map_err(|_| GroupAgentGraphRunServiceError::InconsistentStoreResult)?;
    let plan: GroupAgentGraphCorePlan = serde_json::from_str(&inspection.plan_json)
        .map_err(|_| GroupAgentGraphRunServiceError::InconsistentStoreResult)?;
    let events = inspection
        .event_jsons
        .iter()
        .map(|json| serde_json::from_str::<GroupAgentGraphRunEvent>(json))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GroupAgentGraphRunServiceError::InconsistentStoreResult)?;
    if plan != inspection.plan || events != inspection.events {
        return Err(GroupAgentGraphRunServiceError::InconsistentStoreResult);
    }
    Ok(inspection)
}

pub(crate) fn validate_run_graph(
    plan: &GroupAgentGraphCorePlan,
    graph: &GroupAgentGraphInspection,
) -> Result<(), GroupAgentGraphRunServiceError> {
    let authored_node_ids = graph
        .manifest
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let valid = plan.graph_id == graph.graph.graph_id
        && plan.graph_version == graph.graph.v
        && plan.graph_manifest_sha256 == graph.graph.manifest_sha256
        && plan.authored_node_ids == authored_node_ids
        && plan.edges == graph.manifest.edges
        && plan.waves == graph.manifest.waves
        && !plan.execution_contract_present
        && !plan.dispatch_authority_released;
    valid
        .then_some(())
        .ok_or(GroupAgentGraphRunServiceError::InvalidGraph)
}

pub(crate) fn begin_request(
    input: &PrepareGroupAgentGraphRunInput,
    plan: GroupAgentGraphCorePlan,
    graph: &GroupAgentGraphInspection,
) -> Result<BeginGroupAgentGraphRun, GroupAgentGraphRunServiceError> {
    let event = prepared_event(input, &plan);
    let event_json = event
        .canonical_json()
        .map_err(|_| GroupAgentGraphRunServiceError::InvalidInput)?;
    let request = BeginGroupAgentGraphRun {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: input.graph_run_id.clone(),
        graph_id: input.graph_id.clone(),
        source_snapshot_sha256: graph.graph.source_snapshot_sha256.clone(),
        graph_manifest_sha256: graph.graph.manifest_sha256.clone(),
        plan,
        plan_json: input.plan_json.clone(),
        event,
        event_json,
        idempotency_key: input.idempotency_key.clone(),
        created_at_ms: input.created_at_ms,
    };
    request
        .validate()
        .map_err(|_| GroupAgentGraphRunServiceError::InvalidInput)?;
    Ok(request)
}

fn prepared_event(
    input: &PrepareGroupAgentGraphRunInput,
    plan: &GroupAgentGraphCorePlan,
) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: input.graph_run_id.clone(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: input.graph_id.clone(),
            graph_manifest_sha256: plan.graph_manifest_sha256.clone(),
            plan_sha256: plan.plan_sha256.clone(),
            scheduler_protocol_version: plan.scheduler_protocol_version,
            prepared_at_ms: input.created_at_ms,
        },
    }
}

pub(crate) fn validate_prepare_result(
    request: &BeginGroupAgentGraphRun,
    result: BeginGroupAgentGraphRunResult,
) -> Result<BeginGroupAgentGraphRunResult, GroupAgentGraphRunServiceError> {
    if result.v != GROUP_AGENT_GRAPH_RUN_VERSION {
        return Err(GroupAgentGraphRunServiceError::InconsistentStoreResult);
    }
    let disposition = result.disposition;
    let inspection = checked_run(result.inspection)?;
    if !same_semantics(request, &inspection) || !created_matches(request, disposition, &inspection)
    {
        return Err(GroupAgentGraphRunServiceError::InconsistentStoreResult);
    }
    Ok(BeginGroupAgentGraphRunResult {
        v: result.v,
        disposition,
        inspection,
    })
}

fn same_semantics(
    request: &BeginGroupAgentGraphRun,
    inspection: &GroupAgentGraphRunInspection,
) -> bool {
    inspection.run.graph_id == request.graph_id
        && inspection.run.source_snapshot_sha256 == request.source_snapshot_sha256
        && inspection.run.graph_manifest_sha256 == request.graph_manifest_sha256
        && inspection.plan == request.plan
        && inspection.plan_json == request.plan_json
}

fn created_matches(
    request: &BeginGroupAgentGraphRun,
    disposition: BeginGroupAgentGraphRunDisposition,
    inspection: &GroupAgentGraphRunInspection,
) -> bool {
    disposition != BeginGroupAgentGraphRunDisposition::Created
        || (inspection.run.graph_run_id == request.graph_run_id
            && inspection.run.created_at_ms == request.created_at_ms
            && inspection.events.as_slice() == [request.event.clone()]
            && inspection.event_jsons.as_slice() == [request.event_json.clone()])
}

pub(crate) fn validate_list_input(
    graph_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentGraphRunServiceError> {
    if !(1..=MAX_GROUP_AGENT_GRAPH_RUN_LIST_LIMIT).contains(&limit) {
        return Err(GroupAgentGraphRunServiceError::InvalidInput);
    }
    if let Some(id) = graph_id {
        validate_identifier(id)?;
    }
    Ok(())
}

pub(crate) fn validate_list(
    records: &[GroupAgentGraphRunRecord],
    graph_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentGraphRunServiceError> {
    if records.len() > limit {
        return Err(GroupAgentGraphRunServiceError::InconsistentStoreResult);
    }
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|_| GroupAgentGraphRunServiceError::InconsistentStoreResult)?;
        if graph_id.is_some_and(|id| id != record.graph_id)
            || !ids.insert(record.graph_run_id.as_str())
        {
            return Err(GroupAgentGraphRunServiceError::InconsistentStoreResult);
        }
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), GroupAgentGraphRunServiceError> {
    crate::group_agent_graph_validation::validate_identifier(value)
        .map_err(|_| GroupAgentGraphRunServiceError::InvalidInput)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum
        && !value.chars().any(|value| {
            value.is_control()
                || matches!(
                    value,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
}
