use std::sync::Arc;

use crate::runtime_domain::{
    Cancellation, GroupAgentGraphRunStore, GroupAgentGraphStore,
    GroupAgentNodeCoreTerminalReceiptPort, GroupAgentNodeDispatchProviderFactory,
    GroupAgentNodeDispatchRequestStore, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeLifecycleStore,
};

use crate::{GroupAgentNodeDispatchReleaseControlService, GroupAgentNodeDispatchRequestCodec};

#[path = "group_agent_node_dispatch_execution/build.rs"]
pub(crate) mod build;
#[path = "group_agent_node_dispatch_execution/collector.rs"]
mod collector;
#[path = "group_agent_node_dispatch_execution/error.rs"]
mod error;
#[path = "group_agent_node_dispatch_execution/service.rs"]
mod service;

pub(crate) use collector::{CollectedDispatchEvidence, DispatchCollectionLimits, collect_dispatch};

pub use error::GroupAgentNodeDispatchExecutionServiceError;
pub use service::validate_group_agent_node_dispatch_topology;

#[derive(Clone, Debug)]
pub struct ExecuteGroupAgentNodeDispatchInput {
    pub graph_run_id: String,
    pub authorization_json: String,
    pub pricing_json: String,
    pub confirm_off_machine: bool,
    pub cancellation: Cancellation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeDispatchClaimMetadata {
    pub dispatch_id: String,
    pub lane_ownership_id: String,
    pub released_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecuteGroupAgentNodeDispatchResult {
    Terminalized(GroupAgentNodeLifecycleInspection),
    AlreadyClaimed(GroupAgentNodeLifecycleInspection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeCredentialSourceError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeDispatchMetadataSourceError;

pub trait GroupAgentNodeCredentialSource: Send + Sync {
    /// Reads one invocation-scoped credential without exposing it in errors.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when no safe credential can be supplied.
    fn read_credential(&self) -> Result<String, GroupAgentNodeCredentialSourceError>;
}

pub trait GroupAgentNodeDispatchMetadataSource: Send + Sync {
    /// Creates unpredictable claim identities and one wall-clock timestamp.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when secure claim metadata cannot be created.
    fn claim_metadata(
        &self,
    ) -> Result<GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchMetadataSourceError>;

    /// Returns a terminal artifact/commit timestamp at or after the claim time.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when a bounded timestamp is unavailable.
    fn terminal_time_ms(&self) -> Result<u64, GroupAgentNodeDispatchMetadataSourceError>;
}

pub struct GroupAgentNodeDispatchExecutionService {
    release: GroupAgentNodeDispatchReleaseControlService,
    runs: Arc<dyn GroupAgentGraphRunStore>,
    lifecycles: Arc<dyn GroupAgentNodeLifecycleStore>,
    providers: Arc<dyn GroupAgentNodeDispatchProviderFactory>,
    credentials: Arc<dyn GroupAgentNodeCredentialSource>,
    core: Arc<dyn GroupAgentNodeCoreTerminalReceiptPort>,
    metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
}

impl GroupAgentNodeDispatchExecutionService {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        requests: Arc<dyn GroupAgentNodeDispatchRequestStore>,
        lifecycles: Arc<dyn GroupAgentNodeLifecycleStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
        providers: Arc<dyn GroupAgentNodeDispatchProviderFactory>,
        credentials: Arc<dyn GroupAgentNodeCredentialSource>,
        core: Arc<dyn GroupAgentNodeCoreTerminalReceiptPort>,
        metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
    ) -> Self {
        Self {
            release: GroupAgentNodeDispatchReleaseControlService::new(graphs, requests, codec),
            runs,
            lifecycles,
            providers,
            credentials,
            core,
            metadata,
        }
    }
}

impl std::fmt::Display for GroupAgentNodeCredentialSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Node Dispatch credential is unavailable")
    }
}

impl std::error::Error for GroupAgentNodeCredentialSourceError {}

impl std::fmt::Display for GroupAgentNodeDispatchMetadataSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Node Dispatch claim metadata is unavailable")
    }
}

impl std::error::Error for GroupAgentNodeDispatchMetadataSourceError {}
