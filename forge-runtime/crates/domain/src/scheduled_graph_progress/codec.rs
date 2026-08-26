use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES, MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES,
    SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_DIGEST_DOMAIN,
    SCHEDULED_GRAPH_RECONCILE_DECISION_DIGEST_DOMAIN, ScheduledGraphProgressSnapshot,
    ScheduledGraphProgressValidationError, ScheduledGraphReconcileDecision,
};

#[derive(Serialize)]
struct SnapshotPayload<'a> {
    v: u16,
    progress_protocol_version: u16,
    graph_run_id: &'a str,
    graph_id: &'a str,
    schedule_id: &'a str,
    schedule_sha256: &'a str,
    node_count: usize,
    execution_mode: crate::GroupAgentGraphExecutionMode,
    max_in_flight_nodes: usize,
    progression_policy: crate::GroupAgentGraphExecutionProgressionPolicy,
    attempt_policy: crate::GroupAgentGraphExecutionAttemptPolicy,
    failure_policy: crate::GroupAgentGraphExecutionFailurePolicy,
    nodes: &'a [super::ScheduledGraphProgressNode],
}

#[derive(Serialize)]
struct DecisionPayload<'a> {
    v: u16,
    progress_protocol_version: u16,
    graph_run_id: &'a str,
    schedule_id: &'a str,
    schedule_sha256: &'a str,
    snapshot_sha256: &'a str,
    disposition: super::ScheduledGraphReconcileDisposition,
    next_execution_ordinal: Option<usize>,
    next_node_id: Option<&'a str>,
}

pub(super) fn decode_snapshot_exact(
    bytes: &[u8],
) -> Result<ScheduledGraphProgressSnapshot, ScheduledGraphProgressValidationError> {
    ensure_bound(
        bytes,
        MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES,
        "progress snapshot",
    )?;
    let value: ScheduledGraphProgressSnapshot = serde_json::from_slice(bytes)
        .map_err(|_| invalid("progress snapshot input is invalid JSON"))?;
    value.validate()?;
    ensure_canonical(bytes, &value, "progress snapshot")?;
    Ok(value)
}

pub(super) fn decode_decision_exact(
    bytes: &[u8],
) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphProgressValidationError> {
    ensure_bound(
        bytes,
        MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES,
        "reconcile decision",
    )?;
    let value: ScheduledGraphReconcileDecision = serde_json::from_slice(bytes)
        .map_err(|_| invalid("reconcile decision input is invalid JSON"))?;
    value.validate()?;
    ensure_canonical(bytes, &value, "reconcile decision")?;
    Ok(value)
}

pub(super) fn canonical_json(
    value: &impl Serialize,
) -> Result<String, ScheduledGraphProgressValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("progress value cannot be encoded"))
}

pub(super) fn snapshot_digest(
    snapshot: &ScheduledGraphProgressSnapshot,
) -> Result<String, ScheduledGraphProgressValidationError> {
    digest_json(
        SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_DIGEST_DOMAIN,
        &snapshot_payload(snapshot),
    )
}

pub(super) fn decision_digest(
    decision: &ScheduledGraphReconcileDecision,
) -> Result<String, ScheduledGraphProgressValidationError> {
    digest_json(
        SCHEDULED_GRAPH_RECONCILE_DECISION_DIGEST_DOMAIN,
        &decision_payload(decision),
    )
}

fn snapshot_payload(snapshot: &ScheduledGraphProgressSnapshot) -> SnapshotPayload<'_> {
    SnapshotPayload {
        v: snapshot.v,
        progress_protocol_version: snapshot.progress_protocol_version,
        graph_run_id: &snapshot.graph_run_id,
        graph_id: &snapshot.graph_id,
        schedule_id: &snapshot.schedule_id,
        schedule_sha256: &snapshot.schedule_sha256,
        node_count: snapshot.node_count,
        execution_mode: snapshot.execution_mode,
        max_in_flight_nodes: snapshot.max_in_flight_nodes,
        progression_policy: snapshot.progression_policy,
        attempt_policy: snapshot.attempt_policy,
        failure_policy: snapshot.failure_policy,
        nodes: &snapshot.nodes,
    }
}

fn decision_payload(decision: &ScheduledGraphReconcileDecision) -> DecisionPayload<'_> {
    DecisionPayload {
        v: decision.v,
        progress_protocol_version: decision.progress_protocol_version,
        graph_run_id: &decision.graph_run_id,
        schedule_id: &decision.schedule_id,
        schedule_sha256: &decision.schedule_sha256,
        snapshot_sha256: &decision.snapshot_sha256,
        disposition: decision.disposition,
        next_execution_ordinal: decision.next_execution_ordinal,
        next_node_id: decision.next_node_id.as_deref(),
    }
}

fn digest_json(
    domain: &[u8],
    value: &impl Serialize,
) -> Result<String, ScheduledGraphProgressValidationError> {
    let json = canonical_json(value)?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(json.as_bytes());
    Ok(format!("{:x}", digest.finalize()))
}

fn ensure_bound(
    bytes: &[u8],
    maximum: usize,
    subject: &str,
) -> Result<(), ScheduledGraphProgressValidationError> {
    (!(bytes.is_empty()) && bytes.len() <= maximum)
        .then_some(())
        .ok_or_else(|| invalid(&format!("{subject} input is outside its byte bound")))
}

fn ensure_canonical(
    bytes: &[u8],
    value: &impl Serialize,
    subject: &str,
) -> Result<(), ScheduledGraphProgressValidationError> {
    (canonical_json(value)?.as_bytes() == bytes)
        .then_some(())
        .ok_or_else(|| {
            invalid(&format!(
                "{subject} input is not exact compact canonical JSON"
            ))
        })
}

fn invalid(message: &str) -> ScheduledGraphProgressValidationError {
    super::validation::invalid(message)
}
