use std::collections::BTreeSet;

use crate::runtime_domain::{
    AdmitGroupAgentScheduledNodeContractCandidate, AdmitGroupAgentScheduledNodeContractDisposition,
    AdmitGroupAgentScheduledNodeContractResult, GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphInspection,
    GroupAgentGraphRunInspection, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractInspection,
    GroupAgentScheduledNodeContractRecord, GroupAgentScheduledNodeContractScope,
    GroupAgentScheduledNodeLifecycleInspection, GroupAgentScheduledNodeLifecycleStatus,
    GroupAgentScheduledNodePredecessorOutcome, GroupAgentScheduledNodePredecessorReceipt,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalArtifactKind,
    GroupAgentScheduledNodeTerminalReceipt, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_LIST_LIMIT,
};

use super::{
    AdmitGroupAgentScheduledNodeContractInput, ExportGroupAgentGraphControl,
    GroupAgentScheduledNodeContractServiceError,
    scheduled_contract_error::{corrupt, invalid},
};

pub(super) fn parse_admit_input(
    input: &AdmitGroupAgentScheduledNodeContractInput,
) -> Result<GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractServiceError> {
    validate_identifier(&input.graph_run_id, "Graph Run ID")?;
    validate_text(
        &input.idempotency_key,
        MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        "idempotency key",
    )?;
    if i64::try_from(input.admitted_at_ms).is_err()
        || !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES).contains(&input.contract_json.len())
    {
        return Err(invalid("contract byte bound or admission time is invalid"));
    }
    let candidate = GroupAgentScheduledNodeContractCandidate::decode_exact(&input.contract_json)
        .map_err(|error| invalid(&error.to_string()))?;
    if candidate.graph_run_id != input.graph_run_id {
        return Err(invalid("contract Graph Run binding disagrees"));
    }
    Ok(candidate)
}

pub(super) fn validate_initial_scope(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    (candidate.contract_scope == GroupAgentScheduledNodeContractScope::ScheduleInitialNodeOnly)
        .then_some(())
        .ok_or_else(|| invalid("initial admission requires a schedule_initial_node_only candidate"))
}

pub(super) fn admission_request(
    input: &AdmitGroupAgentScheduledNodeContractInput,
    candidate: GroupAgentScheduledNodeContractCandidate,
    control: ExportGroupAgentGraphControl,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
) -> Result<
    AdmitGroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractServiceError,
> {
    let request = AdmitGroupAgentScheduledNodeContractCandidate {
        v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
        graph_run_id: input.graph_run_id.clone(),
        control_snapshot: control.snapshot,
        control_snapshot_json: control.snapshot_json,
        schedule: schedule.schedule.clone(),
        schedule_json: schedule.schedule_json.clone(),
        candidate,
        candidate_json: input.contract_json.clone(),
        idempotency_key: input.idempotency_key.clone(),
        admitted_at_ms: input.admitted_at_ms,
    };
    request
        .validate()
        .map_err(|error| invalid(&error.to_string()))?;
    Ok(request)
}

pub(super) fn validate_admit_result(
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
    result: AdmitGroupAgentScheduledNodeContractResult,
) -> Result<AdmitGroupAgentScheduledNodeContractResult, GroupAgentScheduledNodeContractServiceError>
{
    if result.v != GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION {
        return Err(corrupt(
            "store returned an unsupported scheduled contract version",
        ));
    }
    let inspection = checked_inspection(result.inspection)?;
    if inspection.candidate != request.candidate
        || inspection.candidate_json != request.candidate_json
        || inspection.record.graph_run_id != request.graph_run_id
        || inspection.record.schedule_id != request.schedule.schedule_id
    {
        return Err(corrupt(
            "store returned different scheduled contract semantics",
        ));
    }
    if result.disposition == AdmitGroupAgentScheduledNodeContractDisposition::Created
        && inspection.record.created_at_ms != request.admitted_at_ms
    {
        return Err(corrupt(
            "created contract does not preserve its admission time",
        ));
    }
    Ok(AdmitGroupAgentScheduledNodeContractResult {
        v: result.v,
        disposition: result.disposition,
        inspection,
    })
}

pub(super) fn checked_run(
    inspection: GroupAgentGraphRunInspection,
) -> Result<GroupAgentGraphRunInspection, GroupAgentScheduledNodeContractServiceError> {
    super::validation::checked_run(inspection).map_err(Into::into)
}

pub(super) fn checked_graph(
    inspection: GroupAgentGraphInspection,
) -> Result<GroupAgentGraphInspection, GroupAgentScheduledNodeContractServiceError> {
    super::validation::checked_graph(inspection).map_err(Into::into)
}

pub(super) fn checked_schedule(
    inspection: GroupAgentGraphExecutionScheduleInspection,
) -> Result<GroupAgentGraphExecutionScheduleInspection, GroupAgentScheduledNodeContractServiceError>
{
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn checked_inspection(
    inspection: GroupAgentScheduledNodeContractInspection,
) -> Result<GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractServiceError>
{
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn validate_sources(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    control: &ExportGroupAgentGraphControl,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    schedule
        .schedule
        .validate_against_control(&control.snapshot)
        .map_err(|error| corrupt(&error.to_string()))?;
    candidate
        .validate_against_control_and_schedule(&control.snapshot, &schedule.schedule)
        .map_err(|error| corrupt(&error.to_string()))
}

pub(super) fn validate_list_input(
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    if !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_LIST_LIMIT).contains(&limit) {
        return Err(invalid(
            "scheduled contract list limit is outside its bounds",
        ));
    }
    if let Some(id) = graph_run_id {
        validate_identifier(id, "Graph Run ID")?;
    }
    Ok(())
}

pub(super) fn validate_list(
    records: &[GroupAgentScheduledNodeContractRecord],
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    validate_contract_list(records, graph_run_id, limit, true)
}

pub(super) fn validate_successor_list(
    records: &[GroupAgentScheduledNodeContractRecord],
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    validate_contract_list(records, graph_run_id, limit, false)
}

fn validate_contract_list(
    records: &[GroupAgentScheduledNodeContractRecord],
    graph_run_id: Option<&str>,
    limit: usize,
    unique_runs: bool,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    if records.len() > limit {
        return Err(corrupt(
            "store returned more contract records than requested",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut runs = BTreeSet::new();
    let mut schedule_slots = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|error| corrupt(&error.to_string()))?;
        let slot = (
            record.schedule_id.as_str(),
            record.execution_ordinal,
            record.attempt,
        );
        let duplicate_run = unique_runs && !runs.insert(record.graph_run_id.as_str());
        if graph_run_id.is_some_and(|id| id != record.graph_run_id)
            || !ids.insert(record.contract_id.as_str())
            || duplicate_run
            || !schedule_slots.insert(slot)
        {
            return Err(corrupt(
                "store returned unfiltered or duplicate contract metadata",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_identifier(
    value: &str,
    subject: &str,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    super::validation::validate_identifier(value, subject).map_err(Into::into)
}

fn validate_text(
    value: &str,
    maximum: usize,
    subject: &str,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    let valid = !value.trim().is_empty()
        && value.len() <= maximum
        && !value.chars().any(unsupported_character);
    valid
        .then_some(())
        .ok_or_else(|| invalid(&format!("{subject} is invalid")))
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

pub(super) fn receipt_binding_matches(
    stored: &GroupAgentScheduledNodeTerminalReceipt,
    receipt: &GroupAgentScheduledNodePredecessorReceipt,
) -> bool {
    stored.node_id == receipt.predecessor_node_id
        && stored.attempt == receipt.predecessor_attempt
        && stored.receipt_id == receipt.terminal_receipt_id
        && stored.receipt_sha256 == receipt.terminal_receipt_sha256
        && stored.provider_request_id == receipt.provider_request_id
        && stored.dispatch_id == receipt.dispatch_id
        && stored.node_outcome == GroupAgentNodeTerminalOutcome::Completed
        && stored.artifact_kind == GroupAgentScheduledNodeTerminalArtifactKind::Result
        && !stored.retry_authorized
        && stored.lane_release_authorized
        && !stored.successor_advance_authorized
        && receipt.node_outcome == GroupAgentScheduledNodePredecessorOutcome::Completed
}

pub(super) fn verify_content_presence(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    supplied: Option<&str>,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    match (candidate.request.predecessor_content_included, supplied) {
        (false, None) => Ok(()),
        (false, Some(_)) => Err(invalid(
            "predecessor content supplied for a candidate that excludes it",
        )),
        (true, Some(content)) if !content.is_empty() => Ok(()),
        (true, _) => Err(invalid(
            "successor candidate embeds predecessor content; --predecessor-content is required",
        )),
    }
}

pub(super) fn validated_terminal_result_artifact(
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
) -> Result<&GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeContractServiceError> {
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    if inspection.status != GroupAgentScheduledNodeLifecycleStatus::Terminalized {
        return Err(invalid("predecessor lifecycle is not terminalized"));
    }
    let artifact = inspection
        .artifact
        .as_ref()
        .ok_or_else(|| corrupt("terminalized lifecycle has no artifact for predecessor content"))?;
    if artifact.artifact_kind != GroupAgentScheduledNodeTerminalArtifactKind::Result {
        return Err(invalid(
            "predecessor content can only be disclosed from a result-class artifact",
        ));
    }
    Ok(artifact)
}
