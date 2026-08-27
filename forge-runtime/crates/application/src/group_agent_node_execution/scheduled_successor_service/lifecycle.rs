use std::sync::Arc;

use crate::runtime_domain::{
    GroupAgentScheduledNodeAnyLifecycleInspection,
    GroupAgentScheduledNodeAnyLifecycleInspectionStore, GroupAgentScheduledNodeLifecycleStore,
    HubStoreError,
};

struct LegacyLifecycleInspectionStore(Arc<dyn GroupAgentScheduledNodeLifecycleStore>);

impl GroupAgentScheduledNodeAnyLifecycleInspectionStore for LegacyLifecycleInspectionStore {
    fn inspect_group_agent_scheduled_node_lifecycle_any_family(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError> {
        self.0
            .inspect_group_agent_scheduled_node_lifecycle(provider_request_id)
            .map(Box::new)
            .map(GroupAgentScheduledNodeAnyLifecycleInspection::Legacy)
    }
}

pub(super) fn legacy_inspection_store(
    store: Arc<dyn GroupAgentScheduledNodeLifecycleStore>,
) -> Arc<dyn GroupAgentScheduledNodeAnyLifecycleInspectionStore> {
    Arc::new(LegacyLifecycleInspectionStore(store))
}
