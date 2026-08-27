use forge_runtime_application::GroupAgentNodeDispatchClaimMetadata;
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::Args,
    group_agent_graph::{
        scheduled_dispatch_execution_output::ScheduledExecutorOwnerCleanup,
        scheduled_executor_sidecar::ScheduledExecutorSidecar,
    },
    runtime_domain::{
        GroupAgentScheduledNodeAnyLifecycleInspection, GroupAgentScheduledNodeAnyLifecycleStore,
        GroupAgentScheduledNodeLifecycleStatus,
    },
    state_path::hub_database_path,
};

pub(super) enum DispatchErrorReconciliation {
    Released {
        inspection: GroupAgentScheduledNodeAnyLifecycleInspection,
        cleanup: ScheduledExecutorOwnerCleanup,
    },
    NotClaimed,
    Uncertain,
}

pub(super) fn reconcile_owner_after_error(
    args: &Args,
    provider_request_id: &str,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
    owner: ScheduledExecutorSidecar,
) -> DispatchErrorReconciliation {
    let Ok(database) = hub_database_path(args.state_dir.as_deref()) else {
        return DispatchErrorReconciliation::Uncertain;
    };
    let Ok(store) = SqliteHubStore::open_existing_current_read_only(database) else {
        return DispatchErrorReconciliation::Uncertain;
    };
    match store.inspect_group_agent_scheduled_node_any_lifecycle(provider_request_id) {
        Ok(inspection) if inspection.claim().lane_ownership_id != metadata.lane_ownership_id => {
            let _ = owner.cleanup();
            DispatchErrorReconciliation::NotClaimed
        }
        Ok(inspection) if released_by_invocation(&inspection, metadata) => {
            DispatchErrorReconciliation::Released {
                inspection,
                cleanup: cleanup_observation(owner),
            }
        }
        Err(crate::runtime_domain::HubStoreError::NotFound { .. }) => {
            let _ = owner.cleanup();
            DispatchErrorReconciliation::NotClaimed
        }
        Ok(_) | Err(_) => DispatchErrorReconciliation::Uncertain,
    }
}

fn released_by_invocation(
    inspection: &GroupAgentScheduledNodeAnyLifecycleInspection,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
) -> bool {
    let released = matches!(
        inspection.status(),
        GroupAgentScheduledNodeLifecycleStatus::Terminalized
            | GroupAgentScheduledNodeLifecycleStatus::Quarantined
    );
    released
        && inspection.claim().lane_ownership_id == metadata.lane_ownership_id
        && !lane_active(inspection)
}

fn lane_active(inspection: &GroupAgentScheduledNodeAnyLifecycleInspection) -> bool {
    match inspection {
        GroupAgentScheduledNodeAnyLifecycleInspection::Legacy(value) => value.active_lane.is_some(),
        GroupAgentScheduledNodeAnyLifecycleInspection::Ready(value) => value.active_lane.is_some(),
    }
}

fn cleanup_observation(owner: ScheduledExecutorSidecar) -> ScheduledExecutorOwnerCleanup {
    if owner.cleanup().is_ok() {
        ScheduledExecutorOwnerCleanup::Succeeded
    } else {
        ScheduledExecutorOwnerCleanup::Failed
    }
}
