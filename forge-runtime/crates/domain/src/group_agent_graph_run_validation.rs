use std::collections::{BTreeMap, BTreeSet};

use super::{
    GROUP_AGENT_GRAPH_CORE_PLAN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphCorePlan, GroupAgentGraphRunValidationError,
    MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES,
};
use crate::{
    GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphEdge, MAX_GROUP_AGENT_GRAPH_EDGES,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, MAX_GROUP_AGENT_GRAPH_NODES,
};

pub(super) fn validate_plan(
    plan: &GroupAgentGraphCorePlan,
) -> Result<(), GroupAgentGraphRunValidationError> {
    validate_plan_header(plan)?;
    let positions = validate_node_ids(&plan.authored_node_ids)?;
    validate_edges(&plan.edges, &positions)?;
    let expected = compute_waves(&plan.authored_node_ids, &plan.edges, &positions)?;
    if plan.waves != expected {
        return Err(invalid(
            "Core Plan waves are not deterministic authored-order waves",
        ));
    }
    if plan.expected_sha256()? != plan.plan_sha256 {
        return Err(invalid(
            "Core Plan digest does not match its canonical payload",
        ));
    }
    let bytes = plan.canonical_json()?.len();
    if !(1..=MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES).contains(&bytes) {
        return Err(invalid("Core Plan exceeds its byte bound"));
    }
    Ok(())
}

fn validate_plan_header(
    plan: &GroupAgentGraphCorePlan,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let valid = plan.v == GROUP_AGENT_GRAPH_CORE_PLAN_VERSION
        && plan.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && plan.graph_version == GROUP_AGENT_GRAPH_VERSION
        && valid_identifier(&plan.graph_id)
        && is_lower_hex_digest(&plan.graph_manifest_sha256)
        && !plan.execution_contract_present
        && !plan.dispatch_authority_released
        && is_lower_hex_digest(&plan.plan_sha256);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid passive Group Agent Graph Core Plan header"))
}

fn validate_node_ids(
    node_ids: &[String],
) -> Result<BTreeMap<&str, usize>, GroupAgentGraphRunValidationError> {
    if !(1..=MAX_GROUP_AGENT_GRAPH_NODES).contains(&node_ids.len()) {
        return Err(invalid("Core Plan node count is outside its bounds"));
    }
    let mut positions = BTreeMap::new();
    for (position, node_id) in node_ids.iter().enumerate() {
        if !valid_identifier(node_id) || positions.insert(node_id.as_str(), position).is_some() {
            return Err(invalid(
                "Core Plan node identifiers are invalid or duplicated",
            ));
        }
    }
    Ok(positions)
}

fn validate_edges(
    edges: &[GroupAgentGraphEdge],
    positions: &BTreeMap<&str, usize>,
) -> Result<(), GroupAgentGraphRunValidationError> {
    if edges.len() > MAX_GROUP_AGENT_GRAPH_EDGES {
        return Err(invalid("Core Plan edge count is outside its bounds"));
    }
    if !edges.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(invalid("Core Plan edges are not in canonical order"));
    }
    let mut seen = BTreeSet::new();
    for edge in edges {
        let pair = (edge.from_node_id.as_str(), edge.to_node_id.as_str());
        if !seen.insert(pair) || edge.from_node_id == edge.to_node_id {
            return Err(invalid(
                "Core Plan edges are duplicated or self-referential",
            ));
        }
        if !positions.contains_key(pair.0) || !positions.contains_key(pair.1) {
            return Err(invalid("Core Plan edge has an unknown endpoint"));
        }
    }
    Ok(())
}

fn compute_waves(
    node_ids: &[String],
    edges: &[GroupAgentGraphEdge],
    positions: &BTreeMap<&str, usize>,
) -> Result<Vec<Vec<String>>, GroupAgentGraphRunValidationError> {
    let (mut indegrees, outgoing) = graph_tables(node_ids.len(), edges, positions)?;
    let mut emitted = vec![false; node_ids.len()];
    let mut waves = Vec::new();
    while let Some(ready) = next_wave(&indegrees, &emitted) {
        for position in &ready {
            emitted[*position] = true;
            for successor in &outgoing[*position] {
                indegrees[*successor] -= 1;
            }
        }
        waves.push(
            ready
                .into_iter()
                .map(|position| node_ids[position].clone())
                .collect(),
        );
    }
    emitted
        .iter()
        .all(|value| *value)
        .then_some(waves)
        .ok_or_else(|| invalid("Core Plan contains a dependency cycle"))
}

fn graph_tables(
    count: usize,
    edges: &[GroupAgentGraphEdge],
    positions: &BTreeMap<&str, usize>,
) -> Result<(Vec<usize>, Vec<Vec<usize>>), GroupAgentGraphRunValidationError> {
    let mut indegrees = vec![0_usize; count];
    let mut outgoing = vec![Vec::new(); count];
    for edge in edges {
        let from = positions[edge.from_node_id.as_str()];
        let to = positions[edge.to_node_id.as_str()];
        indegrees[to] = indegrees[to]
            .checked_add(1)
            .ok_or_else(|| invalid("Core Plan indegree overflowed"))?;
        outgoing[from].push(to);
    }
    Ok((indegrees, outgoing))
}

fn next_wave(indegrees: &[usize], emitted: &[bool]) -> Option<Vec<usize>> {
    let ready = indegrees
        .iter()
        .enumerate()
        .filter_map(|(position, degree)| (!emitted[position] && *degree == 0).then_some(position))
        .collect::<Vec<_>>();
    (!ready.is_empty()).then_some(ready)
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
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

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: &str) -> GroupAgentGraphRunValidationError {
    GroupAgentGraphRunValidationError {
        message: message.into(),
    }
}
