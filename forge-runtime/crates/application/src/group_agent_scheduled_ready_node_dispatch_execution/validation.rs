use crate::group_agent_scheduled_ready_node_dispatch_execution::{
    ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError as Error,
};
use crate::runtime_domain::{
    GroupAgentNodePricingSnapshot, GroupAgentScheduledReadyNodeLifecycleInspection, HubEntity,
    HubStoreError, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
};

pub(super) fn validate_input(
    input: &ExecuteGroupAgentScheduledReadyNodeDispatchInput,
) -> Result<GroupAgentNodePricingSnapshot, Error> {
    let valid = is_identifier(&input.graph_run_id)
        && is_identifier(&input.expected_provider_request_id)
        && is_digest(&input.expected_authorization_sha256)
        && !input.cancellation.is_cancelled();
    if !valid {
        return Err(Error::InvalidInput);
    }
    input
        .confirm_off_machine
        .then_some(())
        .ok_or(Error::ConsentRequired)?;
    GroupAgentNodePricingSnapshot::decode_exact(&input.pricing_json)
        .map_err(|_| Error::InvalidInput)
}

pub(super) fn validate_existing(
    input: &ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    pricing: &GroupAgentNodePricingSnapshot,
    value: GroupAgentScheduledReadyNodeLifecycleInspection,
) -> Result<GroupAgentScheduledReadyNodeLifecycleInspection, Error> {
    let exact = value.authorization.authorization_sha256 == input.expected_authorization_sha256
        && value.authorization.scheduled_provider_request_id == input.expected_provider_request_id
        && value.graph_run.run.graph_run_id == input.graph_run_id
        && value.pricing == *pricing;
    if !exact {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            message: "expected ready-node immutable evidence disagrees with lifecycle".into(),
        }
        .into());
    }
    let includes_content = value
        .release_control
        .scheduled_contract
        .request
        .predecessor_content_included;
    if includes_content && !input.confirm_predecessor_content {
        return Err(Error::PredecessorContentConsentRequired);
    }
    Ok(value)
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
}
