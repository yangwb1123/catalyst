use crate::runtime_domain::{
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractInspection,
    ScheduledGraphControllerHeader, ScheduledGraphProgressNode,
};

use super::ScheduledGraphControllerServiceError;

pub(super) fn validate_materialized_candidate(
    header: &ScheduledGraphControllerHeader,
    node: &ScheduledGraphProgressNode,
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<(), ScheduledGraphControllerServiceError> {
    let valid = candidate.graph_run_id == header.graph_run_id
        && candidate.schedule_sha256 == header.schedule_sha256
        && candidate.node.execution_ordinal == node.execution_ordinal
        && candidate.node.node_id == node.node_id
        && candidate_matches_profile(header, candidate);
    valid
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

pub(super) fn validate_candidate_inspection(
    header: &ScheduledGraphControllerHeader,
    node: &ScheduledGraphProgressNode,
    inspection: &GroupAgentScheduledNodeContractInspection,
) -> Result<(), ScheduledGraphControllerServiceError> {
    validate_materialized_candidate(header, node, &inspection.candidate)
}

fn candidate_matches_profile(
    header: &ScheduledGraphControllerHeader,
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> bool {
    let profile = &header.execution_profile;
    candidate.provider.endpoint == profile.endpoint
        && candidate.provider.model == profile.model
        && u64::from(candidate.budgets.max_output_tokens) == profile.max_output_tokens
        && u64::try_from(candidate.budgets.max_model_output_bytes).ok()
            == Some(profile.max_model_output_bytes)
        && u64::from(candidate.budgets.max_model_events) == profile.max_model_events
        && candidate.budgets.timeout_ms == profile.timeout_ms
        && candidate.budgets.max_cost_usd_micros == profile.max_cost_usd_micros
        && candidate.budgets.pricing_snapshot_sha256 == profile.pricing_snapshot_sha256
        && u64::try_from(candidate.result.max_result_bytes).ok() == Some(profile.max_result_bytes)
}
