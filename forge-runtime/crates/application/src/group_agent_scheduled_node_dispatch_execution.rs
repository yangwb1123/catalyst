#![allow(clippy::missing_errors_doc)]

use std::sync::Arc;

use thiserror::Error;

use crate::group_agent_node_execution::{
    ExportGroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeDispatchReleaseControlService,
    GroupAgentScheduledNodeDispatchReleaseControlServiceError,
};
use crate::runtime_domain::{
    Cancellation, ClaimGroupAgentScheduledNodeDispatch, ClaimGroupAgentScheduledNodeDispatchResult,
    GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION, GroupAgentGraphRunStore,
    GroupAgentGraphStore, GroupAgentNodePricingSnapshot, GroupAgentNodeTerminalClassification,
    GroupAgentScheduledNodeActiveLane, GroupAgentScheduledNodeContractStore,
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchClaimEvent, GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeLifecycleInspection, GroupAgentScheduledNodeLifecycleStore,
    GroupAgentScheduledNodeProviderFactory, GroupAgentScheduledNodeProviderRequestStore,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalArtifactKind,
    GroupAgentScheduledNodeTerminalControl, GroupAgentScheduledNodeTerminalReceiptPort,
    TerminalizeGroupAgentScheduledNodeDispatch, group_agent_scheduled_node_terminal_output_sha256,
};
use crate::{
    GroupAgentNodeCredentialSource, GroupAgentNodeDispatchClaimMetadata,
    GroupAgentNodeDispatchMetadataSource, GroupAgentNodeDispatchRequestCodec,
    group_agent_node_dispatch_execution::{
        CollectedDispatchEvidence, DispatchCollectionLimits, collect_dispatch,
    },
};

#[path = "group_agent_scheduled_node_dispatch_execution_helpers.rs"]
mod helpers;
use helpers::{build_artifact, build_claim_request, build_control, limits};

#[derive(Clone, Debug)]
pub struct ExecuteGroupAgentScheduledNodeDispatchInput {
    pub provider_request_id: String,
    pub authorization_json: String,
    pub pricing_json: String,
    pub confirm_off_machine: bool,
    /// Required exactly when the admitted candidate embeds predecessor
    /// content: sending another node's produced text off-machine is an
    /// independent consent, never inferred from --confirm-off-machine.
    pub confirm_predecessor_content: bool,
    pub cancellation: Cancellation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecuteGroupAgentScheduledNodeDispatchResult {
    Terminalized(GroupAgentScheduledNodeLifecycleInspection),
    AlreadyClaimed(GroupAgentScheduledNodeLifecycleInspection),
}

#[derive(Debug, Error)]
pub enum GroupAgentScheduledNodeDispatchExecutionServiceError {
    #[error("scheduled Node Dispatch input is invalid")]
    InvalidInput,
    #[error("fresh --confirm-off-machine consent is required")]
    ConsentRequired,
    #[error("scheduled Node Dispatch credential is unavailable")]
    CredentialUnavailable,
    #[error("registered scheduled Node provider is unavailable")]
    ProviderUnavailable,
    #[error("scheduled Node Dispatch state is not ready")]
    InvalidState,
    #[error("scheduled Node Dispatch is durably claimed and quarantined; resend is forbidden")]
    DispatchQuarantined,
    #[error("scheduled Node Dispatch store failed: {0}")]
    Store(#[from] forge_runtime_domain::HubStoreError),
    #[error("scheduled Node Dispatch release verification failed: {0}")]
    Release(#[from] GroupAgentScheduledNodeDispatchReleaseControlServiceError),
}

pub struct GroupAgentScheduledNodeDispatchExecutionService {
    release: GroupAgentScheduledNodeDispatchReleaseControlService,
    lifecycles: Arc<dyn GroupAgentScheduledNodeLifecycleStore>,
    providers: Arc<dyn GroupAgentScheduledNodeProviderFactory>,
    credentials: Arc<dyn GroupAgentNodeCredentialSource>,
    core: Arc<dyn GroupAgentScheduledNodeTerminalReceiptPort>,
    metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
}

impl GroupAgentScheduledNodeDispatchExecutionService {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        schedules: Arc<dyn forge_runtime_domain::GroupAgentGraphExecutionScheduleStore>,
        contracts: Arc<dyn GroupAgentScheduledNodeContractStore>,
        requests: Arc<dyn GroupAgentScheduledNodeProviderRequestStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
        lifecycles: Arc<dyn GroupAgentScheduledNodeLifecycleStore>,
        providers: Arc<dyn GroupAgentScheduledNodeProviderFactory>,
        credentials: Arc<dyn GroupAgentNodeCredentialSource>,
        core: Arc<dyn GroupAgentScheduledNodeTerminalReceiptPort>,
        metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
    ) -> Self {
        Self {
            release: GroupAgentScheduledNodeDispatchReleaseControlService::new(
                graphs, runs, schedules, contracts, requests, codec,
            ),
            lifecycles,
            providers,
            credentials,
            core,
            metadata,
        }
    }

    pub async fn execute(
        &self,
        input: &ExecuteGroupAgentScheduledNodeDispatchInput,
    ) -> Result<
        ExecuteGroupAgentScheduledNodeDispatchResult,
        GroupAgentScheduledNodeDispatchExecutionServiceError,
    > {
        if let Some(existing) = self.existing(&input.provider_request_id)? {
            return Ok(ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(existing));
        }
        let prepared = self.preflight(input)?;
        let claimed = self.claim(&prepared)?;
        match claimed {
            ClaimStart::Already(existing) => {
                Ok(ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(existing))
            }
            ClaimStart::Claimed(value) => self.dispatch(value, input).await,
        }
    }

    fn preflight(
        &self,
        input: &ExecuteGroupAgentScheduledNodeDispatchInput,
    ) -> Result<Preflight, GroupAgentScheduledNodeDispatchExecutionServiceError> {
        if !input.confirm_off_machine {
            return Err(GroupAgentScheduledNodeDispatchExecutionServiceError::ConsentRequired);
        }
        if input.cancellation.is_cancelled() {
            return Err(GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput);
        }
        let export = self.release.export(&input.provider_request_id)?;
        Self::require_predecessor_content_consent(input, &export)?;
        let authorization =
            GroupAgentScheduledNodeDispatchAuthorization::decode_exact(&input.authorization_json)
                .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
        authorization
            .validate_against_release_control(&export.release_control)
            .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
        let pricing = GroupAgentNodePricingSnapshot::decode_exact(&input.pricing_json)
            .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
        let resolved = self
            .providers
            .resolve(&authorization, &pricing)
            .map_err(|_| {
                GroupAgentScheduledNodeDispatchExecutionServiceError::ProviderUnavailable
            })?;
        let quote = pricing
            .verify_scheduled_authorization(&authorization)
            .map_err(|_| {
                GroupAgentScheduledNodeDispatchExecutionServiceError::ProviderUnavailable
            })?;
        if resolved.quote != quote {
            return Err(GroupAgentScheduledNodeDispatchExecutionServiceError::ProviderUnavailable);
        }
        Ok(Preflight {
            export,
            authorization,
            authorization_json: input.authorization_json.clone(),
            pricing,
            pricing_json: input.pricing_json.clone(),
            resolved,
        })
    }

    fn claim(
        &self,
        preflight: &Preflight,
    ) -> Result<ClaimStart, GroupAgentScheduledNodeDispatchExecutionServiceError> {
        let provider = self.build_provider(preflight)?;
        let metadata = self
            .metadata
            .claim_metadata()
            .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
        let request = build_claim_request(preflight, &metadata)?;
        match self
            .lifecycles
            .claim_group_agent_scheduled_node_dispatch(&request)?
        {
            ClaimGroupAgentScheduledNodeDispatchResult::AlreadyClaimed { inspection } => {
                Ok(ClaimStart::Already(inspection))
            }
            ClaimGroupAgentScheduledNodeDispatchResult::Claimed { authority } => {
                Ok(ClaimStart::Claimed(Claimed {
                    authority,
                    provider,
                    release: request.release_control,
                }))
            }
        }
    }

    /// The candidate bound to the provider request may embed predecessor
    /// content; disclosing it off-machine needs its own explicit consent,
    /// never inferred from --confirm-off-machine.
    fn require_predecessor_content_consent(
        input: &ExecuteGroupAgentScheduledNodeDispatchInput,
        export: &ExportGroupAgentScheduledNodeDispatchReleaseControl,
    ) -> Result<(), GroupAgentScheduledNodeDispatchExecutionServiceError> {
        if export
            .release_control
            .scheduled_contract
            .request
            .predecessor_content_included
            && !input.confirm_predecessor_content
        {
            return Err(GroupAgentScheduledNodeDispatchExecutionServiceError::ConsentRequired);
        }
        Ok(())
    }

    fn build_provider(
        &self,
        preflight: &Preflight,
    ) -> Result<
        Box<dyn crate::runtime_domain::PreparedModelProvider>,
        GroupAgentScheduledNodeDispatchExecutionServiceError,
    > {
        let credential = self.credentials.read_credential().map_err(|_| {
            GroupAgentScheduledNodeDispatchExecutionServiceError::CredentialUnavailable
        })?;
        self.providers
            .build(preflight.resolved.clone(), credential)
            .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::ProviderUnavailable)
    }

    async fn dispatch(
        &self,
        claimed: Claimed,
        input: &ExecuteGroupAgentScheduledNodeDispatchInput,
    ) -> Result<
        ExecuteGroupAgentScheduledNodeDispatchResult,
        GroupAgentScheduledNodeDispatchExecutionServiceError,
    > {
        let (claim, body) = claimed.authority.into_parts();
        let evidence = collect_dispatch(
            claimed.provider.as_ref(),
            body,
            &input.cancellation,
            limits(
                &claimed.release.scheduled_contract.budgets,
                claimed.release.scheduled_contract.result.max_result_bytes,
            ),
        )
        .await;
        let terminalized_at_ms = self.metadata.terminal_time_ms().map_err(|_| {
            GroupAgentScheduledNodeDispatchExecutionServiceError::DispatchQuarantined
        })?;
        self.verify_lane(&claim)?;
        let artifact = build_artifact(&claim, &evidence, terminalized_at_ms)?;
        let artifact_json = artifact.canonical_json().map_err(|_| {
            GroupAgentScheduledNodeDispatchExecutionServiceError::DispatchQuarantined
        })?;
        let inspection = self.terminalize(
            &claimed.release,
            &claim,
            artifact,
            artifact_json,
            terminalized_at_ms,
        )?;
        Ok(ExecuteGroupAgentScheduledNodeDispatchResult::Terminalized(
            inspection,
        ))
    }

    /// Re-inspects the durable lifecycle and rejects a claim that no longer
    /// matches the persisted lane before any terminal evidence is built.
    fn verify_lane(
        &self,
        claim: &GroupAgentScheduledNodeDispatchClaim,
    ) -> Result<(), GroupAgentScheduledNodeDispatchExecutionServiceError> {
        let lifecycle = self
            .lifecycles
            .inspect_group_agent_scheduled_node_lifecycle(&claim.provider_request_id)
            .map_err(|_| {
                GroupAgentScheduledNodeDispatchExecutionServiceError::DispatchQuarantined
            })?;
        if lifecycle.claim != *claim {
            return Err(GroupAgentScheduledNodeDispatchExecutionServiceError::DispatchQuarantined);
        }
        Ok(())
    }

    /// Builds the terminal control, asks the pinned Core for a receipt, and
    /// atomically persists evidence plus receipt in one terminalize call.
    fn terminalize(
        &self,
        release: &GroupAgentScheduledNodeDispatchReleaseControl,
        claim: &GroupAgentScheduledNodeDispatchClaim,
        artifact: GroupAgentScheduledNodeTerminalArtifact,
        artifact_json: String,
        terminalized_at_ms: u64,
    ) -> Result<
        GroupAgentScheduledNodeLifecycleInspection,
        GroupAgentScheduledNodeDispatchExecutionServiceError,
    > {
        let control = build_control(release, claim, artifact)?;
        let control_json = control.canonical_json().map_err(|_| {
            GroupAgentScheduledNodeDispatchExecutionServiceError::DispatchQuarantined
        })?;
        let Ok(receipt) = self.core.decide(&control) else {
            self.persist_quarantine(artifact_json, terminalized_at_ms)?;
            return Err(GroupAgentScheduledNodeDispatchExecutionServiceError::DispatchQuarantined);
        };
        let terminal = TerminalizeGroupAgentScheduledNodeDispatch {
            v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
            control: Some(control),
            control_json: Some(control_json),
            artifact_json,
            receipt: Some(receipt.receipt),
            receipt_json: Some(receipt.receipt_json),
            terminalized_at_ms,
        };
        let result = self
            .lifecycles
            .terminalize_group_agent_scheduled_node_dispatch(&terminal)?;
        Ok(result.inspection)
    }

    fn persist_quarantine(
        &self,
        artifact_json: String,
        terminalized_at_ms: u64,
    ) -> Result<(), GroupAgentScheduledNodeDispatchExecutionServiceError> {
        let request = TerminalizeGroupAgentScheduledNodeDispatch {
            v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
            control: None,
            control_json: None,
            artifact_json,
            receipt: None,
            receipt_json: None,
            terminalized_at_ms,
        };
        let _ = self
            .lifecycles
            .terminalize_group_agent_scheduled_node_dispatch(&request)
            .map_err(|_| {
                GroupAgentScheduledNodeDispatchExecutionServiceError::DispatchQuarantined
            })?;
        Ok(())
    }

    fn existing(
        &self,
        provider_request_id: &str,
    ) -> Result<
        Option<GroupAgentScheduledNodeLifecycleInspection>,
        GroupAgentScheduledNodeDispatchExecutionServiceError,
    > {
        match self
            .lifecycles
            .inspect_group_agent_scheduled_node_lifecycle(provider_request_id)
        {
            Ok(value) => Ok(Some(value)),
            Err(forge_runtime_domain::HubStoreError::NotFound { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

struct Preflight {
    export: crate::group_agent_node_execution::ExportGroupAgentScheduledNodeDispatchReleaseControl,
    authorization: GroupAgentScheduledNodeDispatchAuthorization,
    authorization_json: String,
    pricing: GroupAgentNodePricingSnapshot,
    pricing_json: String,
    resolved: crate::runtime_domain::GroupAgentScheduledNodeResolvedDispatch,
}

struct Claimed {
    authority: crate::runtime_domain::GroupAgentScheduledNodeDispatchAuthority,
    provider: Box<dyn crate::runtime_domain::PreparedModelProvider>,
    release: GroupAgentScheduledNodeDispatchReleaseControl,
}

#[allow(clippy::large_enum_variant)]
enum ClaimStart {
    Already(GroupAgentScheduledNodeLifecycleInspection),
    Claimed(Claimed),
}
