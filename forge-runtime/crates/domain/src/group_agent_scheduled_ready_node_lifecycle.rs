#![allow(clippy::missing_errors_doc)]

use crate::{
    ClaimGroupAgentScheduledNodeDispatchResult, GroupAgentGraphRunInspection,
    GroupAgentNodePricingSnapshot, GroupAgentScheduledNodeActiveLane,
    GroupAgentScheduledNodeDispatchAuthority, GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchClaimEvent, GroupAgentScheduledNodeLifecycleInspection,
    GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledNodeProviderFactoryError,
    GroupAgentScheduledNodeProviderRequestRecord, GroupAgentScheduledNodeResolvedDispatch,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalControl,
    GroupAgentScheduledNodeTerminalReceipt, GroupAgentScheduledReadyNodeDispatchAuthorization,
    GroupAgentScheduledReadyNodeDispatchReleaseControl, HubStoreError, PreparedModelProvider,
    TerminalizeGroupAgentScheduledNodeDispatch,
};

#[path = "group_agent_scheduled_ready_node_lifecycle_validation.rs"]
mod validation;

pub const GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION: u16 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimGroupAgentScheduledReadyNodeDispatch {
    pub v: u16,
    pub release_control: GroupAgentScheduledReadyNodeDispatchReleaseControl,
    pub release_control_json: String,
    pub authorization: GroupAgentScheduledReadyNodeDispatchAuthorization,
    pub authorization_json: String,
    pub pricing: GroupAgentNodePricingSnapshot,
    pub pricing_json: String,
    pub provider_request: GroupAgentScheduledNodeProviderRequestRecord,
    pub provider_request_body: Vec<u8>,
    pub claim: GroupAgentScheduledNodeDispatchClaim,
    pub claim_json: String,
    pub active_lane: GroupAgentScheduledNodeActiveLane,
    pub active_lane_json: String,
    pub claim_event: GroupAgentScheduledNodeDispatchClaimEvent,
    pub claim_event_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledReadyNodeLifecycleInspection {
    pub v: u16,
    pub graph_run: GroupAgentGraphRunInspection,
    pub release_control: GroupAgentScheduledReadyNodeDispatchReleaseControl,
    pub authorization: GroupAgentScheduledReadyNodeDispatchAuthorization,
    pub pricing: GroupAgentNodePricingSnapshot,
    pub provider_request: GroupAgentScheduledNodeProviderRequestRecord,
    pub provider_request_body: Vec<u8>,
    pub claim: GroupAgentScheduledNodeDispatchClaim,
    pub claim_json: String,
    pub active_lane: Option<GroupAgentScheduledNodeActiveLane>,
    pub active_lane_json: Option<String>,
    pub artifact: Option<GroupAgentScheduledNodeTerminalArtifact>,
    pub artifact_json: Option<String>,
    pub terminal_control: Option<GroupAgentScheduledNodeTerminalControl>,
    pub terminal_control_json: Option<String>,
    pub terminal_receipt: Option<GroupAgentScheduledNodeTerminalReceipt>,
    pub terminal_receipt_json: Option<String>,
    pub status: GroupAgentScheduledNodeLifecycleStatus,
    pub adjudicated_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupAgentScheduledNodeAnyLifecycleInspection {
    Legacy(Box<GroupAgentScheduledNodeLifecycleInspection>),
    Ready(Box<GroupAgentScheduledReadyNodeLifecycleInspection>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeLifecycleProgressInspection {
    pub graph_run: GroupAgentGraphRunInspection,
    pub provider_request: GroupAgentScheduledNodeProviderRequestRecord,
    pub claim: GroupAgentScheduledNodeDispatchClaim,
    pub active_lane: Option<GroupAgentScheduledNodeActiveLane>,
    pub artifact: Option<GroupAgentScheduledNodeTerminalArtifact>,
    pub terminal_control: Option<GroupAgentScheduledNodeTerminalControl>,
    pub terminal_receipt: Option<GroupAgentScheduledNodeTerminalReceipt>,
    pub status: GroupAgentScheduledNodeLifecycleStatus,
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ClaimGroupAgentScheduledReadyNodeDispatchResult {
    Claimed {
        authority: GroupAgentScheduledNodeDispatchAuthority,
    },
    AlreadyClaimed {
        inspection: GroupAgentScheduledReadyNodeLifecycleInspection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalizeGroupAgentScheduledReadyNodeDispatchResult {
    pub v: u16,
    pub inspection: GroupAgentScheduledReadyNodeLifecycleInspection,
}

pub trait GroupAgentScheduledReadyNodeLifecycleStore:
    GroupAgentScheduledNodeAnyLifecycleStore + Send + Sync
{
    fn claim_group_agent_scheduled_ready_node_dispatch(
        &self,
        request: &ClaimGroupAgentScheduledReadyNodeDispatch,
    ) -> Result<ClaimGroupAgentScheduledReadyNodeDispatchResult, HubStoreError>;

    fn terminalize_group_agent_scheduled_ready_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentScheduledNodeDispatch,
    ) -> Result<TerminalizeGroupAgentScheduledReadyNodeDispatchResult, HubStoreError>;

    fn inspect_group_agent_scheduled_ready_node_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledReadyNodeLifecycleInspection, HubStoreError>;
}

pub trait GroupAgentScheduledNodeAnyLifecycleStore: Send + Sync {
    fn inspect_group_agent_scheduled_node_any_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError>;

    fn adjudicate_group_agent_scheduled_node_any_dispatch(
        &self,
        request: &crate::AdjudicateGroupAgentScheduledNodeDispatch,
    ) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError>;
}

pub trait GroupAgentScheduledReadyNodeProviderFactory: Send + Sync {
    fn resolve_ready(
        &self,
        authorization: &GroupAgentScheduledReadyNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentScheduledNodeResolvedDispatch, GroupAgentScheduledNodeProviderFactoryError>;

    fn build_ready(
        &self,
        resolved: GroupAgentScheduledNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentScheduledNodeProviderFactoryError>;
}

impl ClaimGroupAgentScheduledReadyNodeDispatch {
    pub fn validate(&self) -> Result<(), crate::GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_claim_request(self)
    }
}

impl GroupAgentScheduledReadyNodeLifecycleInspection {
    pub fn validate(&self) -> Result<(), crate::GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_inspection(self)
    }
}

impl GroupAgentScheduledNodeAnyLifecycleInspection {
    #[must_use]
    pub fn claim(&self) -> &GroupAgentScheduledNodeDispatchClaim {
        match self {
            Self::Legacy(value) => &value.claim,
            Self::Ready(value) => &value.claim,
        }
    }

    #[must_use]
    pub fn status(&self) -> GroupAgentScheduledNodeLifecycleStatus {
        match self {
            Self::Legacy(value) => value.status,
            Self::Ready(value) => value.status,
        }
    }

    #[must_use]
    pub fn into_legacy_claim_result(self) -> Option<ClaimGroupAgentScheduledNodeDispatchResult> {
        match self {
            Self::Legacy(inspection) => {
                Some(ClaimGroupAgentScheduledNodeDispatchResult::AlreadyClaimed {
                    inspection: *inspection,
                })
            }
            Self::Ready(_) => None,
        }
    }
}

impl From<GroupAgentScheduledNodeLifecycleInspection>
    for GroupAgentScheduledNodeLifecycleProgressInspection
{
    fn from(value: GroupAgentScheduledNodeLifecycleInspection) -> Self {
        Self {
            graph_run: value.graph_run,
            provider_request: value.provider_request,
            claim: value.claim,
            active_lane: value.active_lane,
            artifact: value.artifact,
            terminal_control: value.terminal_control,
            terminal_receipt: value.terminal_receipt,
            status: value.status,
        }
    }
}

impl From<GroupAgentScheduledReadyNodeLifecycleInspection>
    for GroupAgentScheduledNodeLifecycleProgressInspection
{
    fn from(value: GroupAgentScheduledReadyNodeLifecycleInspection) -> Self {
        Self {
            graph_run: value.graph_run,
            provider_request: value.provider_request,
            claim: value.claim,
            active_lane: value.active_lane,
            artifact: value.artifact,
            terminal_control: value.terminal_control,
            terminal_receipt: value.terminal_receipt,
            status: value.status,
        }
    }
}
