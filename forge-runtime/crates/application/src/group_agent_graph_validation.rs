use std::collections::BTreeSet;

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_VERSION,
    GROUP_CONTEXT_DIGEST_DOMAIN, GROUP_CONTEXT_VERSION, GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN,
    GROUP_RUN_VERSION, GroupAgentGraphInspection, GroupAgentGraphManifest, GroupAgentGraphNode,
    GroupAgentGraphRecord, GroupAgentGraphSource, GroupAgentGraphStore, GroupRunSnapshot,
    GroupRunStatus, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_LIST_LIMIT,
    MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES, PrepareGroupAgentGraph, PrepareGroupAgentGraphDisposition,
    PrepareGroupAgentGraphResult, compute_group_agent_graph_waves,
};

use crate::{
    GroupAgentGraphServiceError, PrepareGroupAgentGraphInput,
    group_model_analysis_codec::{canonical_json_bytes, digest_hex},
};

pub(crate) fn validate_input(
    input: &PrepareGroupAgentGraphInput,
) -> Result<(), GroupAgentGraphServiceError> {
    validate_identifier(&input.graph_id)?;
    validate_identifier(&input.group_run_id)?;
    if !valid_text(
        &input.idempotency_key,
        MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    ) || i64::try_from(input.created_at_ms).is_err()
    {
        return Err(GroupAgentGraphServiceError::InvalidInput);
    }
    input
        .manager
        .validate()
        .map_err(|_| GroupAgentGraphServiceError::InvalidInput)?;
    compute_group_agent_graph_waves(&input.nodes, &input.edges)
        .map_err(|_| GroupAgentGraphServiceError::InvalidInput)?;
    Ok(())
}

pub(crate) fn build_manifest(
    input: &PrepareGroupAgentGraphInput,
    source: GroupAgentGraphSource,
) -> Result<(GroupAgentGraphManifest, String, String), GroupAgentGraphServiceError> {
    let mut edges = input.edges.clone();
    edges.sort();
    let waves = compute_group_agent_graph_waves(&input.nodes, &edges)
        .map_err(|_| GroupAgentGraphServiceError::InvalidInput)?;
    let manifest = GroupAgentGraphManifest {
        v: GROUP_AGENT_GRAPH_VERSION,
        source,
        manager: input.manager.clone(),
        nodes: input.nodes.clone(),
        edges,
        waves,
    };
    manifest
        .validate()
        .map_err(|_| GroupAgentGraphServiceError::InvalidInput)?;
    let bytes =
        canonical_json_bytes(&manifest).map_err(|_| GroupAgentGraphServiceError::InvalidInput)?;
    let digest = digest_hex(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes);
    let json = String::from_utf8(bytes).map_err(|_| GroupAgentGraphServiceError::InvalidInput)?;
    Ok((manifest, json, digest))
}

pub(crate) fn checked_source(
    snapshot: &GroupRunSnapshot,
    requested_id: &str,
) -> Result<GroupAgentGraphSource, GroupAgentGraphServiceError> {
    let context_bytes = canonical_json_bytes(&snapshot.context)
        .map_err(|_| GroupAgentGraphServiceError::InvalidSource)?;
    let payload_bytes = canonical_json_bytes(&snapshot.context.payload)
        .map_err(|_| GroupAgentGraphServiceError::InvalidSource)?;
    let run = &snapshot.run;
    let valid = snapshot.v == GROUP_RUN_VERSION
        && run.v == GROUP_RUN_VERSION
        && run.status == GroupRunStatus::Prepared
        && run.run_id == requested_id
        && run.context_version == GROUP_CONTEXT_VERSION
        && snapshot.context.v == GROUP_CONTEXT_VERSION
        && snapshot.context.payload.group.id == run.group_id
        && snapshot.context.slice_sha256 == run.context_slice_sha256
        && digest_hex(GROUP_CONTEXT_DIGEST_DOMAIN, &payload_bytes) == run.context_slice_sha256
        && snapshot.context_json.as_bytes() == context_bytes
        && run.snapshot_bytes == context_bytes.len()
        && (1..=MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES).contains(&run.snapshot_bytes)
        && digest_hex(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, &context_bytes) == run.snapshot_sha256;
    if !valid {
        return Err(GroupAgentGraphServiceError::InvalidSource);
    }
    Ok(GroupAgentGraphSource {
        group_run_version: run.v,
        group_run_id: run.run_id.clone(),
        group_id: run.group_id.clone(),
        context_version: run.context_version,
        context_slice_sha256: run.context_slice_sha256.clone(),
        snapshot_sha256: run.snapshot_sha256.clone(),
        snapshot_bytes: run.snapshot_bytes,
    })
}

pub(crate) fn checked_inspection(
    inspection: GroupAgentGraphInspection,
) -> Result<GroupAgentGraphInspection, GroupAgentGraphServiceError> {
    inspection
        .validate()
        .map_err(|_| GroupAgentGraphServiceError::InconsistentStoreResult)?;
    let bytes = canonical_json_bytes(&inspection.manifest)
        .map_err(|_| GroupAgentGraphServiceError::InconsistentStoreResult)?;
    let graph = &inspection.graph;
    let valid = inspection.manifest_json.as_bytes() == bytes
        && graph.manifest_bytes == bytes.len()
        && graph.manifest_sha256 == digest_hex(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes);
    valid
        .then_some(inspection)
        .ok_or(GroupAgentGraphServiceError::InconsistentStoreResult)
}

pub(crate) fn validate_prepare_result(
    input: &PrepareGroupAgentGraphInput,
    request: &PrepareGroupAgentGraph,
    result: PrepareGroupAgentGraphResult,
) -> Result<PrepareGroupAgentGraphResult, GroupAgentGraphServiceError> {
    if result.v != GROUP_AGENT_GRAPH_VERSION {
        return Err(GroupAgentGraphServiceError::InconsistentStoreResult);
    }
    let disposition = result.disposition;
    let inspection = checked_inspection(result.inspection)?;
    let created_matches = disposition != PrepareGroupAgentGraphDisposition::Created
        || (inspection.graph.graph_id == input.graph_id
            && inspection.graph.created_at_ms == input.created_at_ms);
    let exact_candidate = inspection.manifest == request.manifest
        && inspection.manifest_json == request.manifest_json
        && inspection.graph.manifest_sha256 == request.manifest_sha256;
    if !created_matches || !exact_candidate {
        return Err(GroupAgentGraphServiceError::InconsistentStoreResult);
    }
    Ok(PrepareGroupAgentGraphResult {
        v: result.v,
        disposition,
        inspection,
    })
}

pub(crate) fn validate_members(nodes: &[GroupAgentGraphNode], snapshot: &GroupRunSnapshot) -> bool {
    nodes.iter().all(|node| {
        snapshot
            .context
            .payload
            .members
            .iter()
            .any(|member| member.project_id == node.project_id && member.role == node.member_role)
    })
}

pub(crate) fn validate_list(
    records: &[GroupAgentGraphRecord],
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentGraphServiceError> {
    if records.len() > limit {
        return Err(GroupAgentGraphServiceError::InconsistentStoreResult);
    }
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|_| GroupAgentGraphServiceError::InconsistentStoreResult)?;
        let valid = group_run_id.is_none_or(|id| id == record.group_run_id)
            && ids.insert(record.graph_id.as_str());
        if !valid {
            return Err(GroupAgentGraphServiceError::InconsistentStoreResult);
        }
    }
    Ok(())
}

pub(crate) fn validate_list_input(
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentGraphServiceError> {
    if !(1..=MAX_GROUP_AGENT_GRAPH_LIST_LIMIT).contains(&limit) {
        return Err(GroupAgentGraphServiceError::InvalidInput);
    }
    if let Some(id) = group_run_id {
        validate_identifier(id)?;
    }
    Ok(())
}

pub(crate) fn canonical_request(
    input: &PrepareGroupAgentGraphInput,
    source: GroupAgentGraphSource,
) -> Result<PrepareGroupAgentGraph, GroupAgentGraphServiceError> {
    let (manifest, manifest_json, manifest_sha256) = build_manifest(input, source)?;
    let request = PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: input.graph_id.clone(),
        manifest,
        manifest_json,
        manifest_sha256,
        idempotency_key: input.idempotency_key.clone(),
        created_at_ms: input.created_at_ms,
    };
    request
        .validate()
        .map_err(|_| GroupAgentGraphServiceError::InvalidInput)?;
    Ok(request)
}

pub(crate) fn validate_identifier(value: &str) -> Result<(), GroupAgentGraphServiceError> {
    if valid_text(
        value,
        forge_runtime_domain::MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    ) {
        Ok(())
    } else {
        Err(GroupAgentGraphServiceError::InvalidInput)
    }
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(unsupported_character)
}

fn unsupported_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

pub(crate) fn prepare_with_store(
    store: &dyn GroupAgentGraphStore,
    input: &PrepareGroupAgentGraphInput,
    request: &PrepareGroupAgentGraph,
) -> Result<PrepareGroupAgentGraphResult, GroupAgentGraphServiceError> {
    let result = store.prepare_group_agent_graph(request)?;
    validate_prepare_result(input, request, result)
}
