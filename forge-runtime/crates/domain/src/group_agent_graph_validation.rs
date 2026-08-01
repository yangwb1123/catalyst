use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphEdge,
    GroupAgentGraphInspection, GroupAgentGraphManifest, GroupAgentGraphNode, GroupAgentGraphRecord,
    GroupAgentGraphSource, GroupAgentGraphStatus, GroupAgentGraphValidationError,
    MAX_GROUP_AGENT_GRAPH_AGENT_PROFILE_BYTES, MAX_GROUP_AGENT_GRAPH_EDGES,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_MANAGER_INSTRUCTION_BYTES, MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES,
    MAX_GROUP_AGENT_GRAPH_MEMBER_ROLE_BYTES, MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES, MAX_GROUP_AGENT_GRAPH_NODES, PrepareGroupAgentGraph,
};
use crate::{GROUP_CONTEXT_VERSION, GROUP_RUN_VERSION, MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES};

pub(super) fn validate_source(
    source: &GroupAgentGraphSource,
) -> Result<(), GroupAgentGraphValidationError> {
    let valid = source.group_run_version == GROUP_RUN_VERSION
        && valid_identifier(&source.group_run_id)
        && valid_identifier(&source.group_id)
        && source.context_version == GROUP_CONTEXT_VERSION
        && is_lower_hex_digest(&source.context_slice_sha256)
        && is_lower_hex_digest(&source.snapshot_sha256)
        && (1..=MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES).contains(&source.snapshot_bytes);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Group Agent Graph source"))
}

pub(super) fn validate_manifest(
    manifest: &GroupAgentGraphManifest,
) -> Result<(), GroupAgentGraphValidationError> {
    if manifest.v != GROUP_AGENT_GRAPH_VERSION {
        return Err(invalid("unsupported Group Agent Graph manifest version"));
    }
    manifest.source.validate()?;
    validate_manager(&manifest.manager)?;
    validate_nodes(&manifest.nodes)?;
    validate_edges(&manifest.nodes, &manifest.edges)?;
    validate_canonical_edge_order(&manifest.edges)?;
    let expected = compute_waves(&manifest.nodes, &manifest.edges)?;
    if manifest.waves != expected {
        return Err(invalid(
            "Group Agent Graph waves are not the deterministic DAG waves",
        ));
    }
    validate_manifest_size(manifest)
}

pub(super) fn validate_record(
    record: &GroupAgentGraphRecord,
) -> Result<(), GroupAgentGraphValidationError> {
    let valid = record.v == GROUP_AGENT_GRAPH_VERSION
        && valid_identifier(&record.graph_id)
        && valid_identifier(&record.group_run_id)
        && record.status == GroupAgentGraphStatus::Prepared
        && is_lower_hex_digest(&record.source_snapshot_sha256)
        && is_lower_hex_digest(&record.manifest_sha256)
        && (1..=MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES).contains(&record.manifest_bytes)
        && (1..=MAX_GROUP_AGENT_GRAPH_NODES).contains(&record.node_count)
        && record.edge_count <= MAX_GROUP_AGENT_GRAPH_EDGES
        && (1..=record.node_count).contains(&record.wave_count)
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Group Agent Graph record"))
}

pub(super) fn validate_prepare(
    request: &PrepareGroupAgentGraph,
) -> Result<(), GroupAgentGraphValidationError> {
    request.manifest.validate()?;
    validate_exact_manifest(
        &request.manifest,
        &request.manifest_json,
        &request.manifest_sha256,
    )?;
    let valid = request.v == GROUP_AGENT_GRAPH_VERSION
        && valid_identifier(&request.graph_id)
        && !request.manifest_json.is_empty()
        && request.manifest_json.len() <= MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES
        && is_lower_hex_digest(&request.manifest_sha256)
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Group Agent Graph preparation"))
}

pub(super) fn validate_inspection(
    inspection: &GroupAgentGraphInspection,
) -> Result<(), GroupAgentGraphValidationError> {
    inspection.graph.validate()?;
    inspection.manifest.validate()?;
    validate_exact_manifest(
        &inspection.manifest,
        &inspection.manifest_json,
        &inspection.graph.manifest_sha256,
    )?;
    let graph = &inspection.graph;
    let manifest = &inspection.manifest;
    let valid = inspection.v == GROUP_AGENT_GRAPH_VERSION
        && graph.group_run_id == manifest.source.group_run_id
        && graph.source_snapshot_sha256 == manifest.source.snapshot_sha256
        && graph.manifest_bytes == inspection.manifest_json.len()
        && graph.node_count == manifest.nodes.len()
        && graph.edge_count == manifest.edges.len()
        && graph.wave_count == manifest.waves.len();
    valid
        .then_some(())
        .ok_or_else(|| invalid("Group Agent Graph inspection disagrees with its manifest"))
}

pub(super) fn compute_waves(
    nodes: &[GroupAgentGraphNode],
    edges: &[GroupAgentGraphEdge],
) -> Result<Vec<Vec<String>>, GroupAgentGraphValidationError> {
    validate_nodes(nodes)?;
    validate_edges(nodes, edges)?;
    let positions = node_positions(nodes)?;
    let (mut indegrees, outgoing) = graph_tables(nodes, edges, &positions)?;
    let mut emitted = vec![false; nodes.len()];
    let mut waves = Vec::new();
    while let Some(ready) = next_wave(&indegrees, &emitted) {
        emit_wave(&ready, &mut indegrees, &outgoing, &mut emitted);
        waves.push(
            ready
                .into_iter()
                .map(|position| nodes[position].node_id.clone())
                .collect(),
        );
    }
    if emitted.iter().all(|value| *value) {
        Ok(waves)
    } else {
        Err(invalid("Group Agent Graph contains a dependency cycle"))
    }
}

pub(super) fn validate_manager(
    manager: &super::GroupAgentGraphManager,
) -> Result<(), GroupAgentGraphValidationError> {
    let valid = valid_text(
        &manager.agent_profile,
        MAX_GROUP_AGENT_GRAPH_AGENT_PROFILE_BYTES,
    ) && valid_prose(
        &manager.instruction,
        MAX_GROUP_AGENT_GRAPH_MANAGER_INSTRUCTION_BYTES,
    );
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Group Agent Graph manager"))
}

fn validate_nodes(nodes: &[GroupAgentGraphNode]) -> Result<(), GroupAgentGraphValidationError> {
    if !(1..=MAX_GROUP_AGENT_GRAPH_NODES).contains(&nodes.len()) {
        return Err(invalid("Group Agent Graph node count is out of range"));
    }
    for node in nodes {
        validate_node(node)?;
    }
    node_positions(nodes).map(|_| ())
}

pub(super) fn validate_node(
    node: &GroupAgentGraphNode,
) -> Result<(), GroupAgentGraphValidationError> {
    let valid = valid_identifier(&node.node_id)
        && valid_identifier(&node.project_id)
        && valid_text(&node.member_role, MAX_GROUP_AGENT_GRAPH_MEMBER_ROLE_BYTES)
        && valid_text(
            &node.agent_profile,
            MAX_GROUP_AGENT_GRAPH_AGENT_PROFILE_BYTES,
        )
        && valid_prose(&node.task, MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES)
        && valid_prose(
            &node.acceptance,
            MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES,
        );
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Group Agent Graph node"))
}

fn validate_edges(
    nodes: &[GroupAgentGraphNode],
    edges: &[GroupAgentGraphEdge],
) -> Result<(), GroupAgentGraphValidationError> {
    if edges.len() > MAX_GROUP_AGENT_GRAPH_EDGES {
        return Err(invalid("Group Agent Graph edge count is out of range"));
    }
    let positions = node_positions(nodes)?;
    graph_tables(nodes, edges, &positions).map(|_| ())
}

fn validate_canonical_edge_order(
    edges: &[GroupAgentGraphEdge],
) -> Result<(), GroupAgentGraphValidationError> {
    let canonical = edges.windows(2).all(|pair| pair[0] < pair[1]);
    canonical
        .then_some(())
        .ok_or_else(|| invalid("Group Agent Graph edges are not in canonical order"))
}

fn node_positions(
    nodes: &[GroupAgentGraphNode],
) -> Result<BTreeMap<&str, usize>, GroupAgentGraphValidationError> {
    let mut positions = BTreeMap::new();
    for (position, node) in nodes.iter().enumerate() {
        if positions.insert(node.node_id.as_str(), position).is_some() {
            return Err(invalid("Group Agent Graph node identifiers must be unique"));
        }
    }
    Ok(positions)
}

fn graph_tables(
    nodes: &[GroupAgentGraphNode],
    edges: &[GroupAgentGraphEdge],
    positions: &BTreeMap<&str, usize>,
) -> Result<(Vec<usize>, Vec<Vec<usize>>), GroupAgentGraphValidationError> {
    let mut seen = BTreeSet::new();
    let mut indegrees = vec![0_usize; nodes.len()];
    let mut outgoing = vec![Vec::new(); nodes.len()];
    for edge in edges {
        let pair = (edge.from_node_id.as_str(), edge.to_node_id.as_str());
        if !seen.insert(pair) {
            return Err(invalid("Group Agent Graph edges must be unique"));
        }
        let (from, to) = edge_positions(edge, positions)?;
        outgoing[from].push(to);
        indegrees[to] = indegrees[to]
            .checked_add(1)
            .ok_or_else(|| invalid("Group Agent Graph indegree overflowed"))?;
    }
    Ok((indegrees, outgoing))
}

fn edge_positions(
    edge: &GroupAgentGraphEdge,
    positions: &BTreeMap<&str, usize>,
) -> Result<(usize, usize), GroupAgentGraphValidationError> {
    if edge.from_node_id == edge.to_node_id {
        return Err(invalid("Group Agent Graph self-dependencies are forbidden"));
    }
    let from = positions
        .get(edge.from_node_id.as_str())
        .copied()
        .ok_or_else(|| invalid("Group Agent Graph edge has an unknown source node"))?;
    let to = positions
        .get(edge.to_node_id.as_str())
        .copied()
        .ok_or_else(|| invalid("Group Agent Graph edge has an unknown destination node"))?;
    Ok((from, to))
}

fn next_wave(indegrees: &[usize], emitted: &[bool]) -> Option<Vec<usize>> {
    let ready = indegrees
        .iter()
        .enumerate()
        .filter_map(|(position, degree)| (!emitted[position] && *degree == 0).then_some(position))
        .collect::<Vec<_>>();
    (!ready.is_empty()).then_some(ready)
}

fn emit_wave(
    ready: &[usize],
    indegrees: &mut [usize],
    outgoing: &[Vec<usize>],
    emitted: &mut [bool],
) {
    for position in ready {
        emitted[*position] = true;
        for successor in &outgoing[*position] {
            indegrees[*successor] -= 1;
        }
    }
}

fn validate_manifest_size(
    manifest: &GroupAgentGraphManifest,
) -> Result<(), GroupAgentGraphValidationError> {
    let size = serde_json::to_vec(manifest)
        .map_err(|_| invalid("Group Agent Graph manifest cannot be serialized"))?
        .len();
    if (1..=MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES).contains(&size) {
        Ok(())
    } else {
        Err(invalid("Group Agent Graph manifest exceeds its byte bound"))
    }
}

fn validate_exact_manifest(
    manifest: &GroupAgentGraphManifest,
    actual_json: &str,
    expected_digest: &str,
) -> Result<(), GroupAgentGraphValidationError> {
    let canonical = canonical_json(manifest)?;
    let valid =
        actual_json.as_bytes() == canonical && manifest_digest(manifest)? == expected_digest;
    valid
        .then_some(())
        .ok_or_else(|| invalid("Group Agent Graph manifest bytes or digest are not canonical"))
}

pub(super) fn manifest_digest(
    manifest: &GroupAgentGraphManifest,
) -> Result<String, GroupAgentGraphValidationError> {
    let canonical = canonical_json(manifest)?;
    Ok(digest_hex(
        GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN,
        &canonical,
    ))
}

fn canonical_json(
    manifest: &GroupAgentGraphManifest,
) -> Result<Vec<u8>, GroupAgentGraphValidationError> {
    let value = serde_json::to_value(manifest)
        .map_err(|_| invalid("Group Agent Graph manifest cannot be serialized"))?;
    serde_json::to_vec(&sort_json(value))
        .map_err(|_| invalid("Group Agent Graph manifest cannot be encoded"))
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => {
            let sorted = items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        other => other,
    }
}

fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(unsupported_character)
}

fn valid_prose(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(|character| {
            (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
                || is_bidi_control(character)
        })
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unsupported_character(value: char) -> bool {
    value.is_control() || is_bidi_control(value)
}

fn is_bidi_control(value: char) -> bool {
    matches!(
        value,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn invalid(message: &str) -> GroupAgentGraphValidationError {
    GroupAgentGraphValidationError {
        message: message.into(),
    }
}
