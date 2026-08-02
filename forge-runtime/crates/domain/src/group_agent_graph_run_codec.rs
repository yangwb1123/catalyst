use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    GROUP_AGENT_GRAPH_CORE_PLAN_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_RUN_CONTROL_EVENT_DIGEST_DOMAIN,
    GROUP_AGENT_GRAPH_RUN_EVENT_DIGEST_DOMAIN, GroupAgentGraphCorePlan, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunValidationError,
};

#[derive(Serialize)]
struct PlanDigestPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    graph_version: u16,
    graph_id: &'a str,
    graph_manifest_sha256: &'a str,
    authored_node_ids: &'a [String],
    edges: &'a [crate::GroupAgentGraphEdge],
    waves: &'a [Vec<String>],
    execution_contract_present: bool,
    dispatch_authority_released: bool,
}

pub(super) fn canonical_json(
    value: &impl Serialize,
) -> Result<String, GroupAgentGraphRunValidationError> {
    let bytes = canonical_bytes(value)?;
    String::from_utf8(bytes).map_err(|_| invalid("canonical JSON was not UTF-8"))
}

pub(super) fn plan_digest(
    plan: &GroupAgentGraphCorePlan,
) -> Result<String, GroupAgentGraphRunValidationError> {
    let payload = PlanDigestPayload {
        v: plan.v,
        scheduler_protocol_version: plan.scheduler_protocol_version,
        graph_version: plan.graph_version,
        graph_id: &plan.graph_id,
        graph_manifest_sha256: &plan.graph_manifest_sha256,
        authored_node_ids: &plan.authored_node_ids,
        edges: &plan.edges,
        waves: &plan.waves,
        execution_contract_present: plan.execution_contract_present,
        dispatch_authority_released: plan.dispatch_authority_released,
    };
    let bytes = canonical_bytes(&payload)?;
    Ok(digest_hex(
        GROUP_AGENT_GRAPH_CORE_PLAN_DIGEST_DOMAIN,
        &bytes,
    ))
}

pub(super) fn event_digest(
    event: &GroupAgentGraphRunEvent,
) -> Result<String, GroupAgentGraphRunValidationError> {
    let bytes = canonical_bytes(event)?;
    let domain = match &event.kind {
        GroupAgentGraphRunEventKind::GraphRunPrepared { .. } => {
            GROUP_AGENT_GRAPH_RUN_EVENT_DIGEST_DOMAIN
        }
        GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted { .. }
        | GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared { .. }
        | GroupAgentGraphRunEventKind::NodeDispatchReleased { .. }
        | GroupAgentGraphRunEventKind::NodeLifecycleTerminalized { .. } => {
            GROUP_AGENT_GRAPH_RUN_CONTROL_EVENT_DIGEST_DOMAIN
        }
    };
    Ok(digest_hex(domain, &bytes))
}

fn canonical_bytes(value: &impl Serialize) -> Result<Vec<u8>, GroupAgentGraphRunValidationError> {
    serde_json::to_vec(value).map_err(|_| invalid("value cannot be encoded as JSON"))
}

fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn invalid(message: &str) -> GroupAgentGraphRunValidationError {
    GroupAgentGraphRunValidationError {
        message: message.into(),
    }
}
