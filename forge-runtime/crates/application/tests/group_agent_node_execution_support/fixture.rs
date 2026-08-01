use std::collections::BTreeMap;

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphControlSnapshot, GroupAgentGraphInspection,
    GroupAgentGraphRecord, GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind,
    GroupAgentGraphRunInspection, GroupAgentGraphRunRecord, GroupAgentGraphRunStatus,
    GroupAgentGraphStatus, GroupAgentNodeExecutionContract,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct GoldenFixture {
    input: GoldenInput,
    expected: GoldenExpected,
}

#[derive(Deserialize)]
struct GoldenInput {
    canonical_control_snapshot_json: String,
}

#[derive(Deserialize)]
struct GoldenExpected {
    canonical_contract_json: String,
}

pub(crate) struct FixtureBundle {
    pub(crate) snapshot_json: String,
    pub(crate) contract_json: String,
    pub(crate) graph: GroupAgentGraphInspection,
    pub(crate) run: GroupAgentGraphRunInspection,
}

pub(crate) fn fixture() -> FixtureBundle {
    let fixture: GoldenFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("shared Go fixture");
    let mut snapshot: GroupAgentGraphControlSnapshot =
        serde_json::from_str(&fixture.input.canonical_control_snapshot_json).expect("snapshot");
    let mut contract: GroupAgentNodeExecutionContract =
        serde_json::from_str(&fixture.expected.canonical_contract_json).expect("contract");
    let event = prepared_event(&snapshot);
    snapshot.last_event_sha256 = event.expected_sha256().expect("prepared event digest");
    snapshot.snapshot_sha256 = snapshot.expected_sha256().expect("snapshot digest");
    contract
        .expected_last_event_sha256
        .clone_from(&snapshot.last_event_sha256);
    contract
        .control_snapshot_sha256
        .clone_from(&snapshot.snapshot_sha256);
    let contract_digest = contract.expected_sha256().expect("contract digest");
    contract.contract_id = format!("node-contract-{contract_digest}");
    contract.contract_sha256 = contract_digest;
    let event_json = event.canonical_json().expect("prepared event JSON");
    let plan_json = snapshot.plan.canonical_json().expect("Core Plan JSON");
    let manifest_json = sorted_json(&snapshot.manifest);
    let graph = graph(&snapshot, manifest_json);
    let run = run(&snapshot, plan_json, event, event_json);
    graph.validate().expect("valid fixture graph");
    run.validate().expect("valid fixture Graph Run");
    contract.validate().expect("valid fixture contract");
    FixtureBundle {
        snapshot_json: snapshot.canonical_json().expect("snapshot JSON"),
        contract_json: contract.canonical_json().expect("contract JSON"),
        graph,
        run,
    }
}

fn prepared_event(snapshot: &GroupAgentGraphControlSnapshot) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: snapshot.graph_run_id.clone(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: snapshot.graph_id.clone(),
            graph_manifest_sha256: snapshot.graph_manifest_sha256.clone(),
            plan_sha256: snapshot.core_plan_sha256.clone(),
            scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
            prepared_at_ms: 73,
        },
    }
}

fn graph(
    snapshot: &GroupAgentGraphControlSnapshot,
    manifest_json: String,
) -> GroupAgentGraphInspection {
    GroupAgentGraphInspection {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph: GroupAgentGraphRecord {
            v: GROUP_AGENT_GRAPH_VERSION,
            graph_id: snapshot.graph_id.clone(),
            group_run_id: snapshot.manifest.source.group_run_id.clone(),
            status: GroupAgentGraphStatus::Prepared,
            source_snapshot_sha256: snapshot.source_snapshot_sha256.clone(),
            manifest_sha256: snapshot.graph_manifest_sha256.clone(),
            manifest_bytes: manifest_json.len(),
            node_count: snapshot.manifest.nodes.len(),
            edge_count: snapshot.manifest.edges.len(),
            wave_count: snapshot.manifest.waves.len(),
            created_at_ms: 72,
        },
        manifest: snapshot.manifest.clone(),
        manifest_json,
    }
}

fn run(
    snapshot: &GroupAgentGraphControlSnapshot,
    plan_json: String,
    event: GroupAgentGraphRunEvent,
    event_json: String,
) -> GroupAgentGraphRunInspection {
    GroupAgentGraphRunInspection {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        run: GroupAgentGraphRunRecord {
            v: GROUP_AGENT_GRAPH_RUN_VERSION,
            graph_run_id: snapshot.graph_run_id.clone(),
            graph_id: snapshot.graph_id.clone(),
            status: GroupAgentGraphRunStatus::AwaitingExecutionContract,
            source_snapshot_sha256: snapshot.source_snapshot_sha256.clone(),
            graph_manifest_sha256: snapshot.graph_manifest_sha256.clone(),
            scheduler_protocol_version: snapshot.scheduler_protocol_version,
            plan_sha256: snapshot.core_plan_sha256.clone(),
            plan_bytes: plan_json.len(),
            node_count: snapshot.plan.authored_node_ids.len(),
            wave_count: snapshot.plan.waves.len(),
            execution_contract_present: false,
            dispatch_authority_released: false,
            last_event_seq: 1,
            journal_bytes: event_json.len(),
            created_at_ms: 73,
        },
        plan_json,
        plan: snapshot.plan.clone(),
        event_jsons: vec![event_json],
        events: vec![event],
    }
}

fn sorted_json(value: &impl serde::Serialize) -> String {
    let value = serde_json::to_value(value).expect("serialize fixture");
    serde_json::to_string(&sort(value)).expect("encode fixture")
}

fn sort(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort).collect()),
        Value::Object(items) => {
            let sorted = items
                .into_iter()
                .map(|(key, value)| (key, sort(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        other => other,
    }
}
