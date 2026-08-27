use crate::runtime_domain::{
    GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledReadyNodeLifecycleInspection,
};

use super::{
    GroupAgentScheduledExecutorOwner, GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecuteGroupAgentScheduledReadyNodeDispatchResult {
    Terminalized {
        inspection: GroupAgentScheduledReadyNodeLifecycleInspection,
        effects: GroupAgentScheduledReadyNodeInvocationEffects,
    },
    Quarantined {
        inspection: GroupAgentScheduledReadyNodeLifecycleInspection,
        effects: GroupAgentScheduledReadyNodeInvocationEffects,
    },
    AlreadyClaimed {
        inspection: GroupAgentScheduledReadyNodeLifecycleInspection,
        effects: GroupAgentScheduledReadyNodeInvocationEffects,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledReadyNodeInvocationEffects {
    pub preclaim_effects_performed: bool,
    pub project_lane_claimed: bool,
    pub provider_stream_polled: bool,
    pub logical_hub_mutated: bool,
    pub terminal_receipt_recorded: bool,
    pub result_persisted: bool,
    pub owner_sidecar_cleanup: GroupAgentScheduledReadyNodeOwnerCleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupAgentScheduledReadyNodeOwnerCleanup {
    NotApplicable,
    Succeeded,
    Failed,
}

impl GroupAgentScheduledReadyNodeInvocationEffects {
    const fn already_claimed(
        preclaim_effects_performed: bool,
        owner_sidecar_cleanup: GroupAgentScheduledReadyNodeOwnerCleanup,
    ) -> Self {
        Self {
            preclaim_effects_performed,
            project_lane_claimed: false,
            provider_stream_polled: false,
            logical_hub_mutated: false,
            terminal_receipt_recorded: false,
            result_persisted: false,
            owner_sidecar_cleanup,
        }
    }

    pub(super) const fn durable(
        provider_stream_polled: bool,
        terminal_receipt_recorded: bool,
        owner_sidecar_cleanup: GroupAgentScheduledReadyNodeOwnerCleanup,
    ) -> Self {
        Self {
            preclaim_effects_performed: true,
            project_lane_claimed: true,
            provider_stream_polled,
            logical_hub_mutated: true,
            terminal_receipt_recorded,
            result_persisted: true,
            owner_sidecar_cleanup,
        }
    }
}

pub(super) fn already_claimed(
    inspection: GroupAgentScheduledReadyNodeLifecycleInspection,
    preclaim_effects_performed: bool,
    owner_sidecar_cleanup: GroupAgentScheduledReadyNodeOwnerCleanup,
) -> ExecuteGroupAgentScheduledReadyNodeDispatchResult {
    ExecuteGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed {
        inspection,
        effects: GroupAgentScheduledReadyNodeInvocationEffects::already_claimed(
            preclaim_effects_performed,
            owner_sidecar_cleanup,
        ),
    }
}

pub(super) fn finish_released(
    owner: Box<dyn GroupAgentScheduledExecutorOwner>,
    inspection: GroupAgentScheduledReadyNodeLifecycleInspection,
    provider_polled: bool,
) -> Result<
    ExecuteGroupAgentScheduledReadyNodeDispatchResult,
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
> {
    let owner_sidecar_cleanup = cleanup_owner(owner);
    let receipt = inspection.status == GroupAgentScheduledNodeLifecycleStatus::Terminalized;
    let effects = GroupAgentScheduledReadyNodeInvocationEffects::durable(
        provider_polled,
        receipt,
        owner_sidecar_cleanup,
    );
    match inspection.status {
        GroupAgentScheduledNodeLifecycleStatus::Terminalized => Ok(
            ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized {
                inspection,
                effects,
            },
        ),
        GroupAgentScheduledNodeLifecycleStatus::Quarantined => Ok(
            ExecuteGroupAgentScheduledReadyNodeDispatchResult::Quarantined {
                inspection,
                effects,
            },
        ),
        _ => Err(
            GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain,
        ),
    }
}

pub(super) fn cleanup_owner(
    owner: Box<dyn GroupAgentScheduledExecutorOwner>,
) -> GroupAgentScheduledReadyNodeOwnerCleanup {
    if owner.cleanup().is_ok() {
        GroupAgentScheduledReadyNodeOwnerCleanup::Succeeded
    } else {
        GroupAgentScheduledReadyNodeOwnerCleanup::Failed
    }
}
