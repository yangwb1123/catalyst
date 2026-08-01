use std::collections::BTreeMap;

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_VERSION, GROUP_CONTEXT_VERSION,
    GROUP_RUN_VERSION, GroupAgentGraphEdge, GroupAgentGraphInspection, GroupAgentGraphManager,
    GroupAgentGraphManifest, GroupAgentGraphNode, GroupAgentGraphRecord, GroupAgentGraphSource,
    GroupAgentGraphStatus, MAX_GROUP_AGENT_GRAPH_AGENT_PROFILE_BYTES, MAX_GROUP_AGENT_GRAPH_EDGES,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES, MAX_GROUP_AGENT_GRAPH_MEMBER_ROLE_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES, MAX_GROUP_AGENT_GRAPH_NODES, PrepareGroupAgentGraph,
    compute_group_agent_graph_waves,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn frontend_backend_sso_graph_has_authored_order_waves() {
    let manifest = manifest();

    manifest.validate().expect("valid Group Agent Graph");
    assert_eq!(
        node_ids(&manifest),
        ["frontend", "backend", "sso", "integration"]
    );
    assert_eq!(
        manifest.waves,
        vec![
            vec!["sso".to_owned()],
            vec!["frontend".to_owned(), "backend".to_owned()],
            vec!["integration".to_owned()],
        ]
    );
    assert_eq!(
        compute_group_agent_graph_waves(&manifest.nodes, &manifest.edges).expect("waves"),
        manifest.waves
    );
}

#[test]
fn multiline_manager_task_and_acceptance_prose_is_valid() {
    let manifest = manifest();

    assert!(manifest.manager.instruction.contains('\n'));
    assert!(manifest.nodes[0].task.contains('\n'));
    assert!(manifest.nodes[0].acceptance.contains('\t'));
    manifest.validate().expect("multiline prose is allowed");
}

#[test]
fn distinct_nodes_may_target_the_same_frozen_project_member() {
    let manifest = manifest();
    let project_nodes = manifest
        .nodes
        .iter()
        .filter(|node| node.project_id == "project-front")
        .count();

    assert_eq!(project_nodes, 2);
    manifest
        .validate()
        .expect("a project member may own multiple graph nodes");
}

#[test]
fn duplicate_nodes_and_edges_are_rejected() {
    let mut duplicate_node = manifest();
    duplicate_node.nodes[1].node_id = duplicate_node.nodes[0].node_id.clone();
    assert!(duplicate_node.validate().is_err());

    let mut duplicate_edge = manifest();
    duplicate_edge.edges.push(duplicate_edge.edges[0].clone());
    duplicate_edge.edges.sort();
    assert!(duplicate_edge.validate().is_err());
    assert!(compute_group_agent_graph_waves(&duplicate_edge.nodes, &duplicate_edge.edges).is_err());
}

#[test]
fn self_unknown_and_cyclic_dependencies_are_rejected() {
    let nodes = manifest().nodes;
    let self_edge = vec![edge("sso", "sso")];
    assert!(compute_group_agent_graph_waves(&nodes, &self_edge).is_err());

    let unknown = vec![edge("unknown", "frontend")];
    assert!(compute_group_agent_graph_waves(&nodes, &unknown).is_err());

    let mut cycle = vec![
        edge("backend", "sso"),
        edge("frontend", "backend"),
        edge("sso", "frontend"),
    ];
    cycle.sort();
    assert!(compute_group_agent_graph_waves(&nodes, &cycle).is_err());
}

#[test]
fn manifest_requires_canonical_edge_order() {
    let mut candidate = manifest();
    candidate.edges.reverse();

    assert!(candidate.validate().is_err());
    assert!(
        compute_group_agent_graph_waves(&candidate.nodes, &candidate.edges).is_ok(),
        "edge input order is irrelevant to wave computation"
    );
}

#[test]
fn tampered_missing_and_duplicate_waves_are_rejected() {
    let valid = manifest();
    let mut reordered = valid.clone();
    reordered.waves[1].swap(0, 1);
    assert!(reordered.validate().is_err());

    let mut missing = valid.clone();
    missing.waves[1].pop();
    assert!(missing.validate().is_err());

    let mut duplicate = valid;
    duplicate.waves[1].push("frontend".into());
    assert!(duplicate.validate().is_err());
}

#[test]
fn source_and_record_bounds_fail_closed() {
    let mut bad_source = source();
    bad_source.snapshot_sha256 = "A".repeat(64);
    assert!(bad_source.validate().is_err());
    bad_source = source();
    bad_source.snapshot_bytes = 0;
    assert!(bad_source.validate().is_err());

    let mut bad_record = record(&manifest());
    bad_record.node_count = 0;
    assert!(bad_record.validate().is_err());
    bad_record = record(&manifest());
    bad_record.edge_count = MAX_GROUP_AGENT_GRAPH_EDGES + 1;
    assert!(bad_record.validate().is_err());
    bad_record = record(&manifest());
    bad_record.created_at_ms = u64::MAX;
    assert!(bad_record.validate().is_err());
}

#[test]
fn node_count_identifier_and_field_bounds_fail_closed() {
    let mut empty = manifest();
    empty.nodes.clear();
    empty.edges.clear();
    empty.waves.clear();
    assert!(empty.validate().is_err());

    let mut too_many = manifest();
    too_many.nodes = (0..=MAX_GROUP_AGENT_GRAPH_NODES)
        .map(|index| node(&format!("node-{index}"), "project-front", "frontend"))
        .collect();
    too_many.edges.clear();
    too_many.waves = vec![node_ids_owned(&too_many)];
    assert!(too_many.validate().is_err());

    let mut long = manifest();
    long.nodes[0].node_id = "n".repeat(MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES + 1);
    assert!(long.validate().is_err());
    long = manifest();
    long.nodes[0].agent_profile = "a".repeat(MAX_GROUP_AGENT_GRAPH_AGENT_PROFILE_BYTES + 1);
    assert!(long.validate().is_err());

    let mut maximum_role = manifest();
    maximum_role.nodes[0].member_role = "r".repeat(MAX_GROUP_AGENT_GRAPH_MEMBER_ROLE_BYTES);
    maximum_role.validate().expect("64-byte role is valid");
    maximum_role.nodes[0].member_role.push('r');
    assert!(maximum_role.validate().is_err());
}

#[test]
fn individually_bounded_fields_cannot_exceed_manifest_limit() {
    let mut oversized = manifest();
    oversized.nodes = (0..MAX_GROUP_AGENT_GRAPH_NODES)
        .map(|index| {
            let mut value = node(&format!("node-{index}"), "project-front", "frontend");
            value.task = "t".repeat(MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES);
            value.acceptance = "a".repeat(MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES);
            value
        })
        .collect();
    oversized.edges.clear();
    oversized.waves = vec![node_ids_owned(&oversized)];

    assert!(
        serde_json::to_vec(&oversized).expect("JSON").len() > MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES
    );
    assert!(oversized.validate().is_err());
}

#[test]
fn prepare_binds_canonical_manifest_bytes_and_digest() {
    let valid = prepare();
    valid.validate().expect("valid preparation");

    let mut tampered_bytes = prepare();
    tampered_bytes.manifest_json = "x".repeat(tampered_bytes.manifest_json.len());
    assert!(tampered_bytes.validate().is_err());

    let mut tampered_digest = prepare();
    tampered_digest.manifest_sha256 = digest('9');
    assert!(tampered_digest.validate().is_err());

    let mut long_key = prepare();
    long_key.idempotency_key = "k".repeat(MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES + 1);
    assert!(long_key.validate().is_err());
}

#[test]
fn canonical_manifest_has_a_fixed_v1_byte_golden() {
    let bytes = canonical_bytes(&manifest());

    assert_eq!(bytes.len(), 1_485);
    assert_eq!(
        manifest_digest(&bytes),
        "997d77ffe7e17d02aec30b732738c9f18adbe7121cb121834dce95ef5bc62a44"
    );
}

#[test]
fn inspection_rejects_metadata_and_byte_envelope_divergence() {
    let valid = inspection();
    valid.validate().expect("valid inspection");

    assert_inspection_rejected(&valid, |value| {
        value.graph.group_run_id = "other-run".into();
    });
    assert_inspection_rejected(&valid, |value| {
        value.graph.source_snapshot_sha256 = digest('9');
    });
    assert_inspection_rejected(&valid, |value| value.graph.wave_count += 1);
    assert_inspection_rejected(&valid, |value| {
        value.manifest_json = "x".repeat(value.manifest_json.len());
    });
    assert_inspection_rejected(&valid, |value| value.graph.manifest_sha256 = digest('9'));
}

#[test]
fn v1_json_rejects_unknown_fields() {
    let mut value = serde_json::to_value(manifest()).expect("manifest value");
    value
        .as_object_mut()
        .expect("manifest object")
        .insert("future_field".into(), Value::Bool(true));
    assert!(serde_json::from_value::<GroupAgentGraphManifest>(value).is_err());

    let mut manager = serde_json::to_value(manifest().manager).expect("manager value");
    manager
        .as_object_mut()
        .expect("manager object")
        .insert("capabilities".into(), Value::Array(Vec::new()));
    assert!(serde_json::from_value::<GroupAgentGraphManager>(manager).is_err());

    let mut edge = serde_json::to_value(edge("sso", "frontend")).expect("edge value");
    edge.as_object_mut()
        .expect("edge object")
        .insert("result_mapping".into(), Value::String("forbidden".into()));
    assert!(serde_json::from_value::<GroupAgentGraphEdge>(edge).is_err());
}

fn manifest() -> GroupAgentGraphManifest {
    let nodes = vec![
        multiline_node("frontend", "project-front", "frontend"),
        node("backend", "project-api", "backend"),
        node("sso", "project-sso", "identity"),
        node("integration", "project-front", "frontend"),
    ];
    let mut edges = vec![
        edge("sso", "frontend"),
        edge("sso", "backend"),
        edge("frontend", "integration"),
        edge("backend", "integration"),
    ];
    edges.sort();
    let waves = compute_group_agent_graph_waves(&nodes, &edges).expect("fixture waves");
    GroupAgentGraphManifest {
        v: GROUP_AGENT_GRAPH_VERSION,
        source: source(),
        manager: GroupAgentGraphManager {
            agent_profile: "integration-manager".into(),
            instruction: "Coordinate frontend, backend, and SSO.\nPreserve attribution.".into(),
        },
        nodes,
        edges,
        waves,
    }
}

fn node(node_id: &str, project_id: &str, member_role: &str) -> GroupAgentGraphNode {
    GroupAgentGraphNode {
        node_id: node_id.into(),
        project_id: project_id.into(),
        member_role: member_role.into(),
        agent_profile: "implementer".into(),
        task: format!("Complete {node_id}."),
        acceptance: format!("{node_id} is explicit and testable."),
    }
}

fn multiline_node(node_id: &str, project_id: &str, role: &str) -> GroupAgentGraphNode {
    let mut value = node(node_id, project_id, role);
    value.task = "Implement the browser flow.\nIntegrate the issuer contract.".into();
    value.acceptance = "Tests pass.\tThe contract is explicit.".into();
    value
}

fn edge(from: &str, to: &str) -> GroupAgentGraphEdge {
    GroupAgentGraphEdge {
        from_node_id: from.into(),
        to_node_id: to.into(),
    }
}

fn source() -> GroupAgentGraphSource {
    GroupAgentGraphSource {
        group_run_version: GROUP_RUN_VERSION,
        group_run_id: "group-run-1".into(),
        group_id: "group-1".into(),
        context_version: GROUP_CONTEXT_VERSION,
        context_slice_sha256: digest('2'),
        snapshot_sha256: digest('3'),
        snapshot_bytes: 1_024,
    }
}

fn record(manifest: &GroupAgentGraphManifest) -> GroupAgentGraphRecord {
    let bytes = canonical_bytes(manifest);
    GroupAgentGraphRecord {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: "graph-1".into(),
        group_run_id: manifest.source.group_run_id.clone(),
        status: GroupAgentGraphStatus::Prepared,
        source_snapshot_sha256: manifest.source.snapshot_sha256.clone(),
        manifest_sha256: manifest_digest(&bytes),
        manifest_bytes: bytes.len(),
        node_count: manifest.nodes.len(),
        edge_count: manifest.edges.len(),
        wave_count: manifest.waves.len(),
        created_at_ms: 50,
    }
}

fn prepare() -> PrepareGroupAgentGraph {
    let manifest = manifest();
    let bytes = canonical_bytes(&manifest);
    PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: "graph-1".into(),
        manifest,
        manifest_json: String::from_utf8(bytes.clone()).expect("UTF-8"),
        manifest_sha256: manifest_digest(&bytes),
        idempotency_key: "graph-key".into(),
        created_at_ms: 50,
    }
}

fn inspection() -> GroupAgentGraphInspection {
    let manifest = manifest();
    let manifest_json = String::from_utf8(canonical_bytes(&manifest)).expect("UTF-8");
    GroupAgentGraphInspection {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph: record(&manifest),
        manifest,
        manifest_json,
    }
}

fn assert_inspection_rejected(
    valid: &GroupAgentGraphInspection,
    mutate: impl FnOnce(&mut GroupAgentGraphInspection),
) {
    let mut candidate = valid.clone();
    mutate(&mut candidate);
    assert!(candidate.validate().is_err());
}

fn node_ids(manifest: &GroupAgentGraphManifest) -> Vec<&str> {
    manifest
        .nodes
        .iter()
        .map(|value| value.node_id.as_str())
        .collect()
}

fn node_ids_owned(manifest: &GroupAgentGraphManifest) -> Vec<String> {
    manifest
        .nodes
        .iter()
        .map(|value| value.node_id.clone())
        .collect()
}

fn canonical_bytes(value: &impl Serialize) -> Vec<u8> {
    let value = serde_json::to_value(value).expect("canonical value");
    serde_json::to_vec(&sort_json(value)).expect("canonical bytes")
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

fn manifest_digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn digest(character: char) -> String {
    character.to_string().repeat(64)
}
