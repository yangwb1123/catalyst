use std::sync::Arc;

use forge_runtime_application::{GroupAgentGraphService, PrepareGroupAgentGraphInput};
use forge_runtime_domain::{
    GROUP_CONTEXT_DIGEST_DOMAIN, GROUP_CONTEXT_VERSION, GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN,
    GROUP_RUN_VERSION, GroupAgentGraphEdge, GroupAgentGraphManager, GroupAgentGraphNode,
    GroupContextMember, GroupContextPayload, GroupContextPolicy, GroupContextSlice,
    GroupContextStats, GroupRunRecord, GroupRunSnapshot, GroupRunStatus, SessionGroup,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::stores::{MemoryGraphStore, MemoryRunStore};

pub(crate) const GROUP_RUN_ID: &str = "group-run-1";

pub(crate) struct Harness {
    pub(crate) service: GroupAgentGraphService,
    pub(crate) runs: Arc<MemoryRunStore>,
    pub(crate) graphs: Arc<MemoryGraphStore>,
}

pub(crate) fn harness() -> Harness {
    let runs = Arc::new(MemoryRunStore::new(snapshot()));
    let graphs = Arc::new(MemoryGraphStore::default());
    let service = GroupAgentGraphService::new(runs.clone(), graphs.clone());
    Harness {
        service,
        runs,
        graphs,
    }
}

pub(crate) fn prepare_input() -> PrepareGroupAgentGraphInput {
    PrepareGroupAgentGraphInput {
        graph_id: "graph-1".into(),
        group_run_id: GROUP_RUN_ID.into(),
        manager: GroupAgentGraphManager {
            agent_profile: "integration-manager".into(),
            instruction: "Coordinate contracts.\n\tPreserve project ownership.".into(),
        },
        nodes: nodes(),
        edges: vec![
            edge("sso-contract", "backend-integration"),
            edge("frontend-integration", "release-check"),
            edge("sso-contract", "frontend-integration"),
            edge("backend-integration", "release-check"),
        ],
        idempotency_key: "graph-key".into(),
        created_at_ms: 50,
    }
}

pub(crate) fn nodes() -> Vec<GroupAgentGraphNode> {
    vec![
        node("frontend-integration", "project-frontend", "frontend"),
        node("backend-integration", "project-backend", "backend"),
        node("sso-contract", "project-sso", "identity"),
        node("release-check", "project-frontend", "frontend"),
    ]
}

fn node(id: &str, project: &str, role: &str) -> GroupAgentGraphNode {
    GroupAgentGraphNode {
        node_id: id.into(),
        project_id: project.into(),
        member_role: role.into(),
        agent_profile: "implementer".into(),
        task: format!("Implement {id}.\nRun focused tests."),
        acceptance: "Contract passes.\n\tEvidence is recorded.".into(),
    }
}

fn edge(from: &str, to: &str) -> GroupAgentGraphEdge {
    GroupAgentGraphEdge {
        from_node_id: from.into(),
        to_node_id: to.into(),
    }
}

pub(crate) fn corrupt_snapshot(mut snapshot: GroupRunSnapshot) -> GroupRunSnapshot {
    snapshot.context.payload.members[0].role = "changed-without-rehash".into();
    snapshot
}

pub(crate) fn rebind_snapshot(mut snapshot: GroupRunSnapshot) -> GroupRunSnapshot {
    snapshot.context.payload.members[0].role = "changed-and-rehashed".into();
    bind_snapshot(&mut snapshot);
    snapshot
}

fn snapshot() -> GroupRunSnapshot {
    let payload = GroupContextPayload {
        policy: GroupContextPolicy::default(),
        group: SessionGroup {
            id: "group-1".into(),
            name: "Integration Group".into(),
            created_at_ms: 1,
        },
        members: vec![
            member("project-frontend", "frontend"),
            member("project-backend", "backend"),
            member("project-sso", "identity"),
        ],
        conversations: Vec::new(),
        stats: GroupContextStats {
            member_count: 3,
            ..GroupContextStats::default()
        },
    };
    let mut snapshot = GroupRunSnapshot {
        v: GROUP_RUN_VERSION,
        run: GroupRunRecord {
            v: GROUP_RUN_VERSION,
            run_id: GROUP_RUN_ID.into(),
            group_id: "group-1".into(),
            status: GroupRunStatus::Prepared,
            context_version: GROUP_CONTEXT_VERSION,
            context_slice_sha256: String::new(),
            snapshot_sha256: String::new(),
            snapshot_bytes: 0,
            created_at_ms: 5,
        },
        context: GroupContextSlice {
            v: GROUP_CONTEXT_VERSION,
            payload,
            slice_sha256: String::new(),
        },
        context_json: String::new(),
    };
    bind_snapshot(&mut snapshot);
    snapshot
}

fn member(project_id: &str, role: &str) -> GroupContextMember {
    GroupContextMember {
        project_id: project_id.into(),
        project_name: project_id.into(),
        role: role.into(),
    }
}

fn bind_snapshot(snapshot: &mut GroupRunSnapshot) {
    let payload = canonical(&snapshot.context.payload);
    let slice = digest(GROUP_CONTEXT_DIGEST_DOMAIN, &payload);
    snapshot.context.slice_sha256.clone_from(&slice);
    snapshot.run.context_slice_sha256 = slice;
    let context = canonical(&snapshot.context);
    snapshot.context_json = String::from_utf8(context.clone()).expect("context UTF-8");
    snapshot.run.snapshot_bytes = context.len();
    snapshot.run.snapshot_sha256 = digest(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, &context);
}

pub(crate) fn canonical(value: &impl Serialize) -> Vec<u8> {
    let value = serde_json::to_value(value).expect("serialize fixture");
    serde_json::to_vec(&sort_json(value)).expect("encode fixture")
}

pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => {
            let sorted = items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<std::collections::BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        other => other,
    }
}
