use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_DIGEST_DOMAIN,
    GROUP_AGENT_NODE_TERMINAL_CONTROL_DIGEST_DOMAIN,
    GROUP_AGENT_NODE_TERMINAL_RECEIPT_DIGEST_DOMAIN, GroupAgentNodeLifecycleValidationError,
    GroupAgentNodeTerminalArtifact, GroupAgentNodeTerminalControl, GroupAgentNodeTerminalReceipt,
};

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct ArtifactPayload<'a> {
    v: u16,
    terminal_artifact_protocol_version: u16,
    artifact_kind: super::GroupAgentNodeTerminalArtifactKind,
    graph_run_id: &'a str,
    node_id: &'a str,
    attempt: u16,
    dispatch_id: &'a str,
    claim_event_sha256: &'a str,
    authorization_sha256: &'a str,
    dispatch_request_sha256: &'a str,
    logical_request_sha256: &'a str,
    request_body_sha256: &'a str,
    pricing_snapshot_sha256: &'a str,
    lane_ownership_id: &'a str,
    project_lane_sha256: &'a str,
    provider_poll_started: bool,
    terminal_seen: bool,
    stream_eof_seen: bool,
    classification: super::GroupAgentNodeTerminalClassification,
    output_text: &'a str,
    output_bytes: usize,
    output_sha256: &'a str,
    usage_observed: bool,
    input_tokens: u64,
    output_tokens: u64,
    actual_cost_calculated: bool,
    actual_cost_usd_micros: u64,
    retry_authorized: bool,
    created_at_ms: u64,
}

#[derive(Serialize)]
struct ControlPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    terminal_control_protocol_version: u16,
    graph_run: &'a crate::GroupAgentGraphRunRecord,
    plan: &'a crate::GroupAgentGraphCorePlan,
    manifest: &'a crate::GroupAgentGraphManifest,
    journal_events: &'a [crate::GroupAgentGraphRunEvent],
    contract_record: &'a crate::GroupAgentNodeExecutionContractRecord,
    contract: &'a crate::GroupAgentNodeExecutionContract,
    dispatch_request: &'a crate::GroupAgentNodeDispatchRequestRecord,
    provider_request_json: &'a str,
    authorization: &'a crate::GroupAgentNodeDispatchAuthorization,
    pricing: &'a crate::GroupAgentNodePricingSnapshot,
    active_lane: &'a super::GroupAgentNodeActiveLane,
    claim: &'a super::GroupAgentNodeDispatchClaim,
    artifact: &'a GroupAgentNodeTerminalArtifact,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct ReceiptPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    terminal_receipt_protocol_version: u16,
    terminal_control_sha256: &'a str,
    expected_last_event_seq: u64,
    expected_last_event_sha256: &'a str,
    graph_run_id: &'a str,
    graph_id: &'a str,
    node_id: &'a str,
    attempt: u16,
    dispatch_id: &'a str,
    lane_ownership_id: &'a str,
    project_lane_sha256: &'a str,
    artifact_kind: super::GroupAgentNodeTerminalArtifactKind,
    artifact_id: &'a str,
    artifact_sha256: &'a str,
    node_outcome: super::GroupAgentNodeTerminalOutcome,
    wave_index: usize,
    wave_outcome: super::GroupAgentNodeTerminalOutcome,
    graph_status: crate::GroupAgentGraphRunStatus,
    retry_authorized: bool,
    lane_release_authorized: bool,
}

impl<'a> From<&'a GroupAgentNodeTerminalArtifact> for ArtifactPayload<'a> {
    fn from(value: &'a GroupAgentNodeTerminalArtifact) -> Self {
        Self {
            v: value.v,
            terminal_artifact_protocol_version: value.terminal_artifact_protocol_version,
            artifact_kind: value.artifact_kind,
            graph_run_id: &value.graph_run_id,
            node_id: &value.node_id,
            attempt: value.attempt,
            dispatch_id: &value.dispatch_id,
            claim_event_sha256: &value.claim_event_sha256,
            authorization_sha256: &value.authorization_sha256,
            dispatch_request_sha256: &value.dispatch_request_sha256,
            logical_request_sha256: &value.logical_request_sha256,
            request_body_sha256: &value.request_body_sha256,
            pricing_snapshot_sha256: &value.pricing_snapshot_sha256,
            lane_ownership_id: &value.lane_ownership_id,
            project_lane_sha256: &value.project_lane_sha256,
            provider_poll_started: value.provider_poll_started,
            terminal_seen: value.terminal_seen,
            stream_eof_seen: value.stream_eof_seen,
            classification: value.classification,
            output_text: &value.output_text,
            output_bytes: value.output_bytes,
            output_sha256: &value.output_sha256,
            usage_observed: value.usage_observed,
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            actual_cost_calculated: value.actual_cost_calculated,
            actual_cost_usd_micros: value.actual_cost_usd_micros,
            retry_authorized: value.retry_authorized,
            created_at_ms: value.created_at_ms,
        }
    }
}

impl<'a> From<&'a GroupAgentNodeTerminalControl> for ControlPayload<'a> {
    fn from(value: &'a GroupAgentNodeTerminalControl) -> Self {
        Self {
            v: value.v,
            scheduler_protocol_version: value.scheduler_protocol_version,
            terminal_control_protocol_version: value.terminal_control_protocol_version,
            graph_run: &value.graph_run,
            plan: &value.plan,
            manifest: &value.manifest,
            journal_events: &value.journal_events,
            contract_record: &value.contract_record,
            contract: &value.contract,
            dispatch_request: &value.dispatch_request,
            provider_request_json: &value.provider_request_json,
            authorization: &value.authorization,
            pricing: &value.pricing,
            active_lane: &value.active_lane,
            claim: &value.claim,
            artifact: &value.artifact,
        }
    }
}

impl<'a> From<&'a GroupAgentNodeTerminalReceipt> for ReceiptPayload<'a> {
    fn from(value: &'a GroupAgentNodeTerminalReceipt) -> Self {
        Self {
            v: value.v,
            scheduler_protocol_version: value.scheduler_protocol_version,
            terminal_receipt_protocol_version: value.terminal_receipt_protocol_version,
            terminal_control_sha256: &value.terminal_control_sha256,
            expected_last_event_seq: value.expected_last_event_seq,
            expected_last_event_sha256: &value.expected_last_event_sha256,
            graph_run_id: &value.graph_run_id,
            graph_id: &value.graph_id,
            node_id: &value.node_id,
            attempt: value.attempt,
            dispatch_id: &value.dispatch_id,
            lane_ownership_id: &value.lane_ownership_id,
            project_lane_sha256: &value.project_lane_sha256,
            artifact_kind: value.artifact_kind,
            artifact_id: &value.artifact_id,
            artifact_sha256: &value.artifact_sha256,
            node_outcome: value.node_outcome,
            wave_index: value.wave_index,
            wave_outcome: value.wave_outcome,
            graph_status: value.graph_status,
            retry_authorized: value.retry_authorized,
            lane_release_authorized: value.lane_release_authorized,
        }
    }
}

pub(super) fn canonical_json(
    value: &impl Serialize,
) -> Result<String, GroupAgentNodeLifecycleValidationError> {
    let encoded =
        serde_json::to_string(value).map_err(|_| invalid("value cannot be canonically encoded"))?;
    Ok(encoded
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029"))
}

pub(super) fn artifact_payload_json(
    value: &GroupAgentNodeTerminalArtifact,
) -> Result<String, GroupAgentNodeLifecycleValidationError> {
    canonical_json(&ArtifactPayload::from(value))
}

pub(super) fn artifact_digest(
    value: &GroupAgentNodeTerminalArtifact,
) -> Result<String, GroupAgentNodeLifecycleValidationError> {
    digest_payload(
        GROUP_AGENT_NODE_TERMINAL_ARTIFACT_DIGEST_DOMAIN,
        &ArtifactPayload::from(value),
    )
}

pub(super) fn control_payload_json(
    value: &GroupAgentNodeTerminalControl,
) -> Result<String, GroupAgentNodeLifecycleValidationError> {
    canonical_json(&ControlPayload::from(value))
}

pub(super) fn control_digest(
    value: &GroupAgentNodeTerminalControl,
) -> Result<String, GroupAgentNodeLifecycleValidationError> {
    digest_payload(
        GROUP_AGENT_NODE_TERMINAL_CONTROL_DIGEST_DOMAIN,
        &ControlPayload::from(value),
    )
}

pub(super) fn receipt_payload_json(
    value: &GroupAgentNodeTerminalReceipt,
) -> Result<String, GroupAgentNodeLifecycleValidationError> {
    canonical_json(&ReceiptPayload::from(value))
}

pub(super) fn receipt_digest(
    value: &GroupAgentNodeTerminalReceipt,
) -> Result<String, GroupAgentNodeLifecycleValidationError> {
    digest_payload(
        GROUP_AGENT_NODE_TERMINAL_RECEIPT_DIGEST_DOMAIN,
        &ReceiptPayload::from(value),
    )
}

pub(super) fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn digest_payload(
    domain: &[u8],
    value: &impl Serialize,
) -> Result<String, GroupAgentNodeLifecycleValidationError> {
    let canonical = canonical_json(value)
        .map_err(|_| invalid("digest payload cannot be canonically encoded"))?;
    Ok(digest_hex(domain, canonical.as_bytes()))
}

fn invalid(message: &str) -> GroupAgentNodeLifecycleValidationError {
    GroupAgentNodeLifecycleValidationError {
        message: message.into(),
    }
}
