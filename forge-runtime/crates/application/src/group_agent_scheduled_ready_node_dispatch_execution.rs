#![allow(clippy::missing_errors_doc)]

use std::sync::Arc;

use thiserror::Error;

use crate::group_agent_node_dispatch_execution::{CollectedDispatchEvidence, collect_dispatch};
use crate::group_agent_scheduled_node_dispatch_execution::helpers::{build_artifact, limits};
use crate::runtime_domain::{
    Cancellation, ClaimGroupAgentScheduledReadyNodeDispatchResult, GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeDispatchClaim, GroupAgentScheduledNodeLifecycleStatus,
    GroupAgentScheduledNodeResolvedDispatch, GroupAgentScheduledNodeTerminalReceiptPort,
    GroupAgentScheduledReadyNodeLifecycleInspection, GroupAgentScheduledReadyNodeLifecycleStore,
    GroupAgentScheduledReadyNodeProviderFactory, HubStoreError, PreparedModelProvider,
    TerminalizeGroupAgentScheduledNodeDispatch,
};
use crate::{
    GroupAgentNodeCredentialSource, GroupAgentNodeDispatchClaimMetadata,
    GroupAgentNodeDispatchMetadataSource, ScheduledReadyNodeReleaseService,
    ScheduledReadyNodeReleaseServiceError,
};

#[path = "group_agent_scheduled_ready_node_dispatch_execution/build.rs"]
mod build;
#[path = "group_agent_scheduled_ready_node_dispatch_execution/result.rs"]
mod result;
#[path = "group_agent_scheduled_ready_node_dispatch_execution/validation.rs"]
mod validation;
pub use result::{
    ExecuteGroupAgentScheduledReadyNodeDispatchResult,
    GroupAgentScheduledReadyNodeInvocationEffects, GroupAgentScheduledReadyNodeOwnerCleanup,
};
use result::{already_claimed, cleanup_owner, finish_released};

#[derive(Clone, Debug)]
pub struct ExecuteGroupAgentScheduledReadyNodeDispatchInput {
    pub graph_run_id: String,
    pub expected_provider_request_id: String,
    pub expected_authorization_sha256: String,
    pub pricing_json: String,
    pub confirm_off_machine: bool,
    pub confirm_predecessor_content: bool,
    pub cancellation: Cancellation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledExecutorOwnerError;

pub trait GroupAgentScheduledExecutorOwner: Send {
    fn preserve_on_drop(&mut self);
    fn cleanup(self: Box<Self>) -> Result<(), GroupAgentScheduledExecutorOwnerError>;
}

pub trait GroupAgentScheduledExecutorOwnerFactory: Send + Sync {
    fn create(
        &self,
        provider_request_id: &str,
        lane_ownership_id: &str,
    ) -> Result<Box<dyn GroupAgentScheduledExecutorOwner>, GroupAgentScheduledExecutorOwnerError>;
}

#[derive(Debug, Error)]
pub enum GroupAgentScheduledReadyNodeDispatchExecutionServiceError {
    #[error("scheduled ready-node step input is invalid")]
    InvalidInput,
    #[error("fresh exact-request --confirm-off-machine consent is required")]
    ConsentRequired,
    #[error("fresh predecessor-content consent is required")]
    PredecessorContentConsentRequired,
    #[error("scheduled ready-node credential is unavailable")]
    CredentialUnavailable,
    #[error("registered scheduled ready-node provider is unavailable")]
    ProviderUnavailable,
    #[error("scheduled ready-node executor owner evidence is unavailable")]
    OwnerEvidenceUnavailable,
    #[error(
        "scheduled ready-node claim outcome is uncertain; preclaim_effects_performed=true; project_lane_claim_observation=unknown; provider_stream_polled=false; remote_send_attested=false; automatic_retry_or_resend=false"
    )]
    ClaimOutcomeUncertain,
    #[error(
        "scheduled ready-node post-claim outcome is uncertain; preclaim_effects_performed=true; project_lane_claimed=true; provider_stream_polled=true; remote_send_attested=false; automatic_retry_or_resend=false"
    )]
    PostClaimOutcomeUncertain,
    #[error("scheduled ready-node store failed: {0}")]
    Store(#[from] HubStoreError),
    #[error("scheduled ready-node release failed: {0}")]
    Release(#[from] ScheduledReadyNodeReleaseServiceError),
}

pub struct GroupAgentScheduledReadyNodeDispatchExecutionService {
    release: ScheduledReadyNodeReleaseService,
    lifecycles: Arc<dyn GroupAgentScheduledReadyNodeLifecycleStore>,
    providers: Arc<dyn GroupAgentScheduledReadyNodeProviderFactory>,
    credentials: Arc<dyn GroupAgentNodeCredentialSource>,
    core: Arc<dyn GroupAgentScheduledNodeTerminalReceiptPort>,
    metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
    owners: Arc<dyn GroupAgentScheduledExecutorOwnerFactory>,
}

impl GroupAgentScheduledReadyNodeDispatchExecutionService {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        progress: Arc<dyn crate::runtime_domain::ScheduledGraphProgressStore>,
        sources: Arc<dyn crate::runtime_domain::ScheduledReadyNodeReleaseStore>,
        reconcile: Arc<dyn crate::runtime_domain::ScheduledGraphReconcilePort>,
        authorize: Arc<dyn crate::runtime_domain::ScheduledReadyNodeReleasePort>,
        lifecycles: Arc<dyn GroupAgentScheduledReadyNodeLifecycleStore>,
        providers: Arc<dyn GroupAgentScheduledReadyNodeProviderFactory>,
        credentials: Arc<dyn GroupAgentNodeCredentialSource>,
        core: Arc<dyn GroupAgentScheduledNodeTerminalReceiptPort>,
        metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
        owners: Arc<dyn GroupAgentScheduledExecutorOwnerFactory>,
    ) -> Self {
        Self {
            release: ScheduledReadyNodeReleaseService::new(progress, sources, reconcile, authorize),
            lifecycles,
            providers,
            credentials,
            core,
            metadata,
            owners,
        }
    }

    pub async fn execute(
        &self,
        input: &ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    ) -> Result<
        ExecuteGroupAgentScheduledReadyNodeDispatchResult,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        let pricing = validation::validate_input(input)?;
        if let Some(existing) = self.existing(input, &pricing)? {
            return Ok(already_claimed(
                existing,
                false,
                GroupAgentScheduledReadyNodeOwnerCleanup::NotApplicable,
            ));
        }
        let preflight = self.preflight(input, pricing)?;
        let metadata = self
            .metadata
            .claim_metadata()
            .map_err(|_| GroupAgentScheduledReadyNodeDispatchExecutionServiceError::InvalidInput)?;
        let mut owner = self.create_owner(&preflight, &metadata)?;
        let provider = self.build_provider(&preflight)?;
        let request = build::claim_request(&preflight, &metadata)?;
        owner.preserve_on_drop();
        match self
            .lifecycles
            .claim_group_agent_scheduled_ready_node_dispatch(&request)
        {
            Ok(ClaimGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed { inspection }) => {
                let cleanup = cleanup_owner(owner);
                Ok(already_claimed(inspection, true, cleanup))
            }
            Ok(ClaimGroupAgentScheduledReadyNodeDispatchResult::Claimed { authority }) => {
                self.dispatch(input, preflight, provider, authority, owner)
                    .await
            }
            Err(_) => {
                self.cleanup_losing_owner(owner, &metadata, input);
                Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ClaimOutcomeUncertain)
            }
        }
    }

    fn existing(
        &self,
        input: &ExecuteGroupAgentScheduledReadyNodeDispatchInput,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<
        Option<GroupAgentScheduledReadyNodeLifecycleInspection>,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        match self
            .lifecycles
            .inspect_group_agent_scheduled_ready_node_lifecycle(&input.expected_provider_request_id)
        {
            Ok(value) => validation::validate_existing(input, pricing, value).map(Some),
            Err(HubStoreError::NotFound { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn preflight(
        &self,
        input: &ExecuteGroupAgentScheduledReadyNodeDispatchInput,
        pricing: GroupAgentNodePricingSnapshot,
    ) -> Result<Preflight, GroupAgentScheduledReadyNodeDispatchExecutionServiceError> {
        let authorized = self.release.authorize(&input.graph_run_id)?;
        let authorization = &authorized.authorization;
        if authorization.authorization_sha256 != input.expected_authorization_sha256
            || authorization.scheduled_provider_request_id != input.expected_provider_request_id
        {
            return Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::InvalidInput);
        }
        let content = authorized
            .release_control
            .scheduled_contract
            .request
            .predecessor_content_included;
        if content && !input.confirm_predecessor_content {
            return Err(
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PredecessorContentConsentRequired,
            );
        }
        let quote = pricing
            .verify_scheduled_ready_authorization(authorization)
            .map_err(|_| {
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ProviderUnavailable
            })?;
        let resolved = self
            .providers
            .resolve_ready(authorization, &pricing)
            .map_err(|_| {
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ProviderUnavailable
            })?;
        if quote != resolved.quote {
            return Err(
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ProviderUnavailable,
            );
        }
        Ok(Preflight {
            authorized,
            pricing,
            pricing_json: input.pricing_json.clone(),
            resolved,
        })
    }

    fn create_owner(
        &self,
        preflight: &Preflight,
        metadata: &GroupAgentNodeDispatchClaimMetadata,
    ) -> Result<
        Box<dyn GroupAgentScheduledExecutorOwner>,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        self.owners
            .create(
                &preflight
                    .authorized
                    .authorization
                    .scheduled_provider_request_id,
                &metadata.lane_ownership_id,
            )
            .map_err(|_| {
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::OwnerEvidenceUnavailable
            })
    }

    fn build_provider(
        &self,
        preflight: &Preflight,
    ) -> Result<
        Box<dyn PreparedModelProvider>,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        let credential = self.credentials.read_credential().map_err(|_| {
            GroupAgentScheduledReadyNodeDispatchExecutionServiceError::CredentialUnavailable
        })?;
        self.providers
            .build_ready(preflight.resolved.clone(), credential)
            .map_err(|_| {
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ProviderUnavailable
            })
    }

    async fn dispatch(
        &self,
        input: &ExecuteGroupAgentScheduledReadyNodeDispatchInput,
        preflight: Preflight,
        provider: Box<dyn PreparedModelProvider>,
        authority: crate::runtime_domain::GroupAgentScheduledNodeDispatchAuthority,
        owner: Box<dyn GroupAgentScheduledExecutorOwner>,
    ) -> Result<
        ExecuteGroupAgentScheduledReadyNodeDispatchResult,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        let (claim, body) = authority.into_parts();
        let evidence = collect_dispatch(
            provider.as_ref(),
            body,
            &input.cancellation,
            limits(
                &preflight.authorized.authorization.budgets,
                preflight
                    .authorized
                    .release_control
                    .scheduled_contract
                    .result
                    .max_result_bytes,
            ),
        )
        .await;
        self.finish_dispatch(&preflight, &claim, &evidence, owner)
    }

    fn finish_dispatch(
        &self,
        preflight: &Preflight,
        claim: &GroupAgentScheduledNodeDispatchClaim,
        evidence: &CollectedDispatchEvidence,
        owner: Box<dyn GroupAgentScheduledExecutorOwner>,
    ) -> Result<
        ExecuteGroupAgentScheduledReadyNodeDispatchResult,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        let terminalized_at_ms = self.metadata.terminal_time_ms().map_err(|_| {
            GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
        })?;
        self.verify_lane(claim)?;
        let artifact = build_artifact(claim, evidence, terminalized_at_ms).map_err(|_| {
            GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
        })?;
        let artifact_json = artifact.canonical_json().map_err(|_| {
            GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
        })?;
        let control =
            build::terminal_control(&preflight.authorized.release_control, claim, artifact)?;
        let terminal = if let Ok(receipt) = self.core.decide(&control) {
            build::terminal_request(control, artifact_json, receipt, terminalized_at_ms)?
        } else {
            let quarantine = build::quarantine_request(artifact_json, terminalized_at_ms);
            let (inspection, cleanup) = self.persist_quarantine(claim, &quarantine, owner)?;
            return Ok(
                ExecuteGroupAgentScheduledReadyNodeDispatchResult::Quarantined {
                    inspection,
                    effects: GroupAgentScheduledReadyNodeInvocationEffects::durable(
                        evidence.provider_poll_started,
                        false,
                        cleanup,
                    ),
                },
            );
        };
        let result = self
            .lifecycles
            .terminalize_group_agent_scheduled_ready_node_dispatch(&terminal);
        self.finish_terminal_result(claim, owner, result, evidence.provider_poll_started)
    }

    fn verify_lane(
        &self,
        claim: &GroupAgentScheduledNodeDispatchClaim,
    ) -> Result<(), GroupAgentScheduledReadyNodeDispatchExecutionServiceError> {
        let value = self
            .lifecycles
            .inspect_group_agent_scheduled_ready_node_lifecycle(&claim.provider_request_id)
            .map_err(|_| {
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
            })?;
        (value.claim == *claim && value.status == GroupAgentScheduledNodeLifecycleStatus::Claimed)
            .then_some(())
            .ok_or(
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain,
            )
    }

    fn persist_quarantine(
        &self,
        claim: &GroupAgentScheduledNodeDispatchClaim,
        request: &TerminalizeGroupAgentScheduledNodeDispatch,
        owner: Box<dyn GroupAgentScheduledExecutorOwner>,
    ) -> Result<
        (
            GroupAgentScheduledReadyNodeLifecycleInspection,
            GroupAgentScheduledReadyNodeOwnerCleanup,
        ),
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        let result = self
            .lifecycles
            .terminalize_group_agent_scheduled_ready_node_dispatch(request);
        match result {
            Ok(value) if value.inspection.active_lane.is_none() => {
                let cleanup = cleanup_owner(owner);
                Ok((value.inspection, cleanup))
            }
            Ok(_) => {
                Err(
                    GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain,
                )
            }
            Err(_) => {
                let inspection = self.inspect_released(claim)?;
                let cleanup = cleanup_owner(owner);
                Ok((inspection, cleanup))
            }
        }
    }

    fn finish_terminal_result(
        &self,
        claim: &GroupAgentScheduledNodeDispatchClaim,
        owner: Box<dyn GroupAgentScheduledExecutorOwner>,
        result: Result<
            crate::runtime_domain::TerminalizeGroupAgentScheduledReadyNodeDispatchResult,
            HubStoreError,
        >,
        provider_polled: bool,
    ) -> Result<
        ExecuteGroupAgentScheduledReadyNodeDispatchResult,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        match result {
            Ok(value) if value.inspection.active_lane.is_none() => {
                finish_released(owner, value.inspection, provider_polled)
            }
            Ok(_) => {
                Err(
                    GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain,
                )
            }
            Err(_) => {
                let inspection = self.inspect_released(claim)?;
                finish_released(owner, inspection, provider_polled)
            }
        }
    }

    fn inspect_released(
        &self,
        claim: &GroupAgentScheduledNodeDispatchClaim,
    ) -> Result<
        GroupAgentScheduledReadyNodeLifecycleInspection,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    > {
        let inspection = self
            .lifecycles
            .inspect_group_agent_scheduled_ready_node_lifecycle(&claim.provider_request_id)
            .map_err(|_| {
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
            })?;
        (inspection.claim == *claim && inspection.active_lane.is_none())
            .then_some(inspection)
            .ok_or(
                GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain,
            )
    }

    fn cleanup_losing_owner(
        &self,
        owner: Box<dyn GroupAgentScheduledExecutorOwner>,
        metadata: &GroupAgentNodeDispatchClaimMetadata,
        input: &ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    ) {
        match self
            .lifecycles
            .inspect_group_agent_scheduled_node_any_lifecycle(&input.expected_provider_request_id)
        {
            Ok(value) if value.claim().lane_ownership_id == metadata.lane_ownership_id => {}
            Ok(_) | Err(HubStoreError::NotFound { .. }) => {
                let _ = owner.cleanup();
            }
            Err(_) => {}
        }
    }
}

pub(crate) struct Preflight {
    authorized: crate::AuthorizedScheduledReadyNodeRelease,
    pricing: GroupAgentNodePricingSnapshot,
    pricing_json: String,
    resolved: GroupAgentScheduledNodeResolvedDispatch,
}
