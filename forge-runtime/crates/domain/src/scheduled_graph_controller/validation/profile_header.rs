use super::super::{
    MAX_SCHEDULED_GRAPH_CONTROLLER_EFFECTFUL_STEPS, MAX_SCHEDULED_GRAPH_CONTROLLER_HEADER_BYTES,
    SCHEDULED_GRAPH_CONTROLLER_PROTOCOL_VERSION, SCHEDULED_GRAPH_CONTROLLER_VERSION,
    ScheduledGraphControllerExecutionProfile, ScheduledGraphControllerHeader,
    ScheduledGraphControllerValidationError, codec,
};
use super::{in_u64_bound, invalid, is_digest, valid_identifier, valid_text};
use crate::{
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION,
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
    GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT, MAX_GROUP_AGENT_GRAPH_NODES,
    MAX_GROUP_AGENT_NODE_COST_USD_MICROS, MAX_GROUP_AGENT_NODE_MODEL_BYTES,
    MAX_GROUP_AGENT_NODE_MODEL_EVENTS, MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES,
    MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS, MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES,
    MAX_GROUP_AGENT_NODE_RESULT_BYTES, MAX_GROUP_AGENT_NODE_TIMEOUT_MS,
    SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
};

pub(in crate::scheduled_graph_controller) fn validate_profile(
    value: &ScheduledGraphControllerExecutionProfile,
) -> Result<(), ScheduledGraphControllerValidationError> {
    let valid = valid_text(
        &value.endpoint,
        MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES,
    ) && value.endpoint == GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT
        && valid_text(&value.model, MAX_GROUP_AGENT_NODE_MODEL_BYTES)
        && in_u64_bound(
            value.max_output_tokens,
            u64::from(MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS),
        )
        && in_u64_bound(
            value.max_model_output_bytes,
            MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES as u64,
        )
        && in_u64_bound(
            value.max_model_events,
            u64::from(MAX_GROUP_AGENT_NODE_MODEL_EVENTS),
        )
        && in_u64_bound(value.timeout_ms, MAX_GROUP_AGENT_NODE_TIMEOUT_MS)
        && in_u64_bound(
            value.max_cost_usd_micros,
            MAX_GROUP_AGENT_NODE_COST_USD_MICROS,
        )
        && is_digest(&value.pricing_snapshot_sha256)
        && in_u64_bound(
            value.max_result_bytes,
            MAX_GROUP_AGENT_NODE_RESULT_BYTES as u64,
        )
        && is_digest(&value.profile_sha256)
        && codec::profile_digest(value)? == value.profile_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled Graph controller execution profile is invalid"))
}

pub(in crate::scheduled_graph_controller) fn validate_header(
    value: &ScheduledGraphControllerHeader,
) -> Result<(), ScheduledGraphControllerValidationError> {
    value.execution_profile.validate()?;
    let valid = value.v == SCHEDULED_GRAPH_CONTROLLER_VERSION
        && value.controller_protocol_version == SCHEDULED_GRAPH_CONTROLLER_PROTOCOL_VERSION
        && valid_identifier(&value.graph_run_id)
        && is_digest(&value.schedule_sha256)
        && value.schedule_id == format!("graph-execution-schedule-{}", value.schedule_sha256)
        && value.schedule_version == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION
        && value.progress_protocol_version == SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION
        && value.progress_protocol_version == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION
        && is_digest(&value.core_bin_sha256)
        && (1..=MAX_GROUP_AGENT_GRAPH_NODES).contains(&value.node_count)
        && value.max_effectful_steps > 0
        && value.max_effectful_steps <= MAX_SCHEDULED_GRAPH_CONTROLLER_EFFECTFUL_STEPS
        && usize::from(value.max_effectful_steps) <= value.node_count
        && value.max_total_cost_usd_micros >= value.execution_profile.max_cost_usd_micros
        && value.max_total_cost_usd_micros
            <= value
                .execution_profile
                .max_cost_usd_micros
                .saturating_mul(u64::from(value.max_effectful_steps))
        && i64::try_from(value.created_at_ms).is_ok()
        && is_digest(&value.controller_sha256)
        && value.controller_id == format!("scheduled-graph-controller-{}", value.controller_sha256)
        && value.controller_sha256 == codec::header_digest(value)?
        && codec::canonical_json(value)?.len() <= MAX_SCHEDULED_GRAPH_CONTROLLER_HEADER_BYTES;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled Graph controller header is invalid"))
}
