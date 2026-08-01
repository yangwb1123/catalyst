use serde::{Deserialize, Deserializer};

use super::{GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedWire {
    v: u16,
    graph_run_id: String,
    seq: u64,
    #[serde(rename = "type")]
    kind: PreparedType,
    graph_id: String,
    graph_manifest_sha256: String,
    plan_sha256: String,
    scheduler_protocol_version: u16,
    prepared_at_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum PreparedType {
    GraphRunPrepared,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractWire {
    v: u16,
    graph_run_id: String,
    seq: u64,
    #[serde(rename = "type")]
    kind: ContractType,
    previous_event_sha256: String,
    control_snapshot_sha256: String,
    contract_id: String,
    contract_sha256: String,
    contract_bytes: usize,
    node_id: String,
    attempt: u16,
    request_sha256: String,
    project_lane_sha256: String,
    admitted_at_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContractType {
    NodeExecutionContractAdmitted,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchRequestWire {
    v: u16,
    graph_run_id: String,
    seq: u64,
    #[serde(rename = "type")]
    kind: DispatchRequestType,
    previous_event_sha256: String,
    contract_id: String,
    contract_sha256: String,
    node_id: String,
    attempt: u16,
    request_sha256: String,
    project_lane_sha256: String,
    provider_request_sha256: String,
    provider_request_bytes: usize,
    codec_version: u16,
    pricing_snapshot_sha256: String,
    prepared_at_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum DispatchRequestType {
    NodeDispatchRequestPrepared,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EventWire {
    Prepared(PreparedWire),
    Contract(ContractWire),
    DispatchRequest(DispatchRequestWire),
}

impl<'de> Deserialize<'de> for GroupAgentGraphRunEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match EventWire::deserialize(deserializer)? {
            EventWire::Prepared(wire) => prepared(wire),
            EventWire::Contract(wire) => contract(wire),
            EventWire::DispatchRequest(wire) => dispatch_request(wire),
        })
    }
}

fn prepared(wire: PreparedWire) -> GroupAgentGraphRunEvent {
    let PreparedType::GraphRunPrepared = wire.kind;
    GroupAgentGraphRunEvent {
        v: wire.v,
        graph_run_id: wire.graph_run_id,
        seq: wire.seq,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: wire.graph_id,
            graph_manifest_sha256: wire.graph_manifest_sha256,
            plan_sha256: wire.plan_sha256,
            scheduler_protocol_version: wire.scheduler_protocol_version,
            prepared_at_ms: wire.prepared_at_ms,
        },
    }
}

fn contract(wire: ContractWire) -> GroupAgentGraphRunEvent {
    let ContractType::NodeExecutionContractAdmitted = wire.kind;
    GroupAgentGraphRunEvent {
        v: wire.v,
        graph_run_id: wire.graph_run_id,
        seq: wire.seq,
        kind: GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
            previous_event_sha256: wire.previous_event_sha256,
            control_snapshot_sha256: wire.control_snapshot_sha256,
            contract_id: wire.contract_id,
            contract_sha256: wire.contract_sha256,
            contract_bytes: wire.contract_bytes,
            node_id: wire.node_id,
            attempt: wire.attempt,
            request_sha256: wire.request_sha256,
            project_lane_sha256: wire.project_lane_sha256,
            admitted_at_ms: wire.admitted_at_ms,
        },
    }
}

fn dispatch_request(wire: DispatchRequestWire) -> GroupAgentGraphRunEvent {
    let DispatchRequestType::NodeDispatchRequestPrepared = wire.kind;
    GroupAgentGraphRunEvent {
        v: wire.v,
        graph_run_id: wire.graph_run_id,
        seq: wire.seq,
        kind: GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
            previous_event_sha256: wire.previous_event_sha256,
            contract_id: wire.contract_id,
            contract_sha256: wire.contract_sha256,
            node_id: wire.node_id,
            attempt: wire.attempt,
            request_sha256: wire.request_sha256,
            project_lane_sha256: wire.project_lane_sha256,
            provider_request_sha256: wire.provider_request_sha256,
            provider_request_bytes: wire.provider_request_bytes,
            codec_version: wire.codec_version,
            pricing_snapshot_sha256: wire.pricing_snapshot_sha256,
            prepared_at_ms: wire.prepared_at_ms,
        },
    }
}
