use crate::runtime_domain::{
    ClaimGroupAgentNodeDispatchResult, GroupAgentGraphRunStatus,
    GroupAgentNodeDispatchAuthorization, GroupAgentNodePricingSnapshot,
    TerminalizeGroupAgentNodeDispatch,
};

use super::{
    ExecuteGroupAgentNodeDispatchInput, ExecuteGroupAgentNodeDispatchResult,
    GroupAgentNodeDispatchExecutionService, GroupAgentNodeDispatchExecutionServiceError,
    build::{
        build_artifact, build_claim_request, build_terminal_control, build_terminalize_request,
    },
    collector::{DispatchCollectionLimits, collect_dispatch},
};

struct EffectFreePreflight {
    export: crate::ExportGroupAgentNodeDispatchReleaseControl,
    authorization: GroupAgentNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
    resolved: crate::runtime_domain::GroupAgentNodeResolvedDispatch,
}

struct ClaimedDispatch {
    authority: crate::runtime_domain::GroupAgentNodeDispatchAuthority,
    provider: Box<dyn crate::runtime_domain::PreparedModelProvider>,
    release: crate::runtime_domain::GroupAgentNodeDispatchReleaseControl,
    authorization: GroupAgentNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
}

enum DispatchClaimStart {
    Already(Box<crate::runtime_domain::GroupAgentNodeLifecycleInspection>),
    Claimed(Box<ClaimedDispatch>),
}

impl GroupAgentNodeDispatchExecutionService {
    /// Executes one complete, one-shot single-node Graph dispatch lifecycle.
    ///
    /// # Errors
    ///
    /// Returns a pre-claim validation error without effects, or a quarantined
    /// diagnosis after authority has been durably claimed. It never resends.
    pub async fn execute(
        &self,
        input: &ExecuteGroupAgentNodeDispatchInput,
    ) -> Result<ExecuteGroupAgentNodeDispatchResult, GroupAgentNodeDispatchExecutionServiceError>
    {
        if let Some(existing) = self.existing_lifecycle(&input.graph_run_id)? {
            return Ok(ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(
                existing,
            ));
        }
        let preflight = self.preflight(input)?;
        match self.claim(input, preflight)? {
            DispatchClaimStart::Already(inspection) => Ok(
                ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(*inspection),
            ),
            DispatchClaimStart::Claimed(claimed) => self.dispatch_claimed(*claimed, input).await,
        }
    }

    fn preflight(
        &self,
        input: &ExecuteGroupAgentNodeDispatchInput,
    ) -> Result<EffectFreePreflight, GroupAgentNodeDispatchExecutionServiceError> {
        let export = self
            .release
            .export(&input.graph_run_id)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::InvalidState)?;
        validate_group_agent_node_dispatch_topology(&export.release_control)?;
        validate_invocation_authority(input)?;
        let authorization = decode_authorization(input, &export.release_control)?;
        let pricing = GroupAgentNodePricingSnapshot::decode_exact(&input.pricing_json)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::InvalidInput)?;
        let resolved = self
            .providers
            .resolve(&authorization, &pricing)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::ProviderUnavailable)?;
        validate_resolved(&resolved, &authorization, &pricing)?;
        Ok(EffectFreePreflight {
            export,
            authorization,
            pricing,
            resolved,
        })
    }

    fn claim(
        &self,
        input: &ExecuteGroupAgentNodeDispatchInput,
        preflight: EffectFreePreflight,
    ) -> Result<DispatchClaimStart, GroupAgentNodeDispatchExecutionServiceError> {
        let credential = self
            .credentials
            .read_credential()
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::CredentialUnavailable)?;
        let provider = self
            .providers
            .build(preflight.resolved.clone(), credential)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::ProviderUnavailable)?;
        let metadata = self
            .metadata
            .claim_metadata()
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::InvalidInput)?;
        let release = preflight.export.release_control.clone();
        let request = build_claim_request(
            preflight.export,
            preflight.authorization.clone(),
            input.authorization_json.clone(),
            preflight.pricing.clone(),
            input.pricing_json.clone(),
            &metadata,
        )?;
        match self.lifecycles.claim_group_agent_node_dispatch(&request)? {
            ClaimGroupAgentNodeDispatchResult::AlreadyClaimed { inspection } => {
                Ok(DispatchClaimStart::Already(Box::new(inspection)))
            }
            ClaimGroupAgentNodeDispatchResult::Claimed { authority } => {
                Ok(DispatchClaimStart::Claimed(Box::new(ClaimedDispatch {
                    authority,
                    provider,
                    release,
                    authorization: preflight.authorization,
                    pricing: preflight.pricing,
                })))
            }
        }
    }

    fn existing_lifecycle(
        &self,
        graph_run_id: &str,
    ) -> Result<
        Option<crate::runtime_domain::GroupAgentNodeLifecycleInspection>,
        GroupAgentNodeDispatchExecutionServiceError,
    > {
        let run = self.runs.inspect_group_agent_graph_run(graph_run_id)?;
        match run.run.status {
            GroupAgentGraphRunStatus::DispatchUnknown
            | GroupAgentGraphRunStatus::Completed
            | GroupAgentGraphRunStatus::Failed
            | GroupAgentGraphRunStatus::FailedUncertain => self
                .lifecycles
                .inspect_group_agent_node_lifecycle(graph_run_id)
                .map(Some)
                .map_err(Into::into),
            GroupAgentGraphRunStatus::AwaitingDispatchAuthorization => Ok(None),
            _ => Err(GroupAgentNodeDispatchExecutionServiceError::InvalidState),
        }
    }

    async fn dispatch_claimed(
        &self,
        claimed: ClaimedDispatch,
        input: &ExecuteGroupAgentNodeDispatchInput,
    ) -> Result<ExecuteGroupAgentNodeDispatchResult, GroupAgentNodeDispatchExecutionServiceError>
    {
        let (claim, body) = claimed.authority.into_parts();
        let evidence = collect_dispatch(
            claimed.provider.as_ref(),
            body,
            &input.cancellation,
            collection_limits(&claimed.release.contract),
        )
        .await;
        let terminalized_at_ms = self
            .metadata
            .terminal_time_ms()
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined)?;
        let lifecycle = self
            .lifecycles
            .inspect_group_agent_node_lifecycle(&claim.graph_run_id)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined)?;
        if lifecycle.claim != claim {
            return Err(GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined);
        }
        let artifact = build_artifact(
            &claim,
            &claimed.authorization,
            &claimed.pricing,
            evidence,
            terminalized_at_ms,
        )
        .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined)?;
        let control = build_terminal_control(
            &claimed.release,
            claimed.authorization,
            claimed.pricing,
            &lifecycle,
            artifact,
        )
        .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined)?;
        let receipt = self
            .core
            .decide(&control)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined)?;
        let terminal = build_terminalize_request(control, receipt, terminalized_at_ms)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined)?;
        self.persist_terminal(&terminal)
    }

    fn persist_terminal(
        &self,
        terminal: &TerminalizeGroupAgentNodeDispatch,
    ) -> Result<ExecuteGroupAgentNodeDispatchResult, GroupAgentNodeDispatchExecutionServiceError>
    {
        let result = self
            .lifecycles
            .terminalize_group_agent_node_dispatch(terminal)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined)?;
        Ok(ExecuteGroupAgentNodeDispatchResult::Terminalized(
            result.inspection,
        ))
    }
}

fn validate_invocation_authority(
    input: &ExecuteGroupAgentNodeDispatchInput,
) -> Result<(), GroupAgentNodeDispatchExecutionServiceError> {
    if !input.confirm_off_machine {
        return Err(GroupAgentNodeDispatchExecutionServiceError::ConsentRequired);
    }
    if input.cancellation.is_cancelled() {
        return Err(GroupAgentNodeDispatchExecutionServiceError::InvalidInput);
    }
    Ok(())
}

fn decode_authorization(
    input: &ExecuteGroupAgentNodeDispatchInput,
    control: &crate::runtime_domain::GroupAgentNodeDispatchReleaseControl,
) -> Result<GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchExecutionServiceError> {
    let authorization =
        GroupAgentNodeDispatchAuthorization::decode_exact(&input.authorization_json)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::InvalidInput)?;
    authorization
        .validate_against_release_control(control)
        .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::InvalidInput)?;
    Ok(authorization)
}

/// Verifies the protocol-v1 one-node, one-wave, zero-edge execution fence.
///
/// # Errors
///
/// Returns `UnsupportedTopology` when the release control describes any other
/// topology.
pub fn validate_group_agent_node_dispatch_topology(
    control: &crate::runtime_domain::GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeDispatchExecutionServiceError> {
    let node = &control.contract.node.node_id;
    let single = control.plan.authored_node_ids.len() == 1
        && control.plan.authored_node_ids[0] == *node
        && control.plan.edges.is_empty()
        && control.plan.waves.len() == 1
        && control.plan.waves[0].len() == 1
        && control.plan.waves[0][0] == *node
        && control.manifest.nodes.len() == 1
        && control.manifest.edges.is_empty()
        && control.manifest.waves.len() == 1
        && control.manifest.waves[0].len() == 1
        && control.manifest.waves[0][0] == *node;
    single
        .then_some(())
        .ok_or(GroupAgentNodeDispatchExecutionServiceError::UnsupportedTopology)
}

fn validate_resolved(
    resolved: &crate::runtime_domain::GroupAgentNodeResolvedDispatch,
    authorization: &GroupAgentNodeDispatchAuthorization,
    pricing: &GroupAgentNodePricingSnapshot,
) -> Result<(), GroupAgentNodeDispatchExecutionServiceError> {
    let quote = pricing
        .verify_authorization(authorization)
        .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::ProviderUnavailable)?;
    let valid = resolved.authorization_sha256 == authorization.authorization_sha256
        && resolved.provider_kind == authorization.provider_kind
        && resolved.endpoint == authorization.endpoint
        && resolved.model == authorization.model
        && resolved.destination_sha256 == quote.destination_sha256
        && resolved.pricing_snapshot_sha256 == quote.pricing_snapshot_sha256
        && resolved.max_input_tokens == quote.max_input_tokens
        && resolved.max_output_tokens == quote.max_output_tokens
        && resolved.max_cost_usd_micros == quote.max_cost_usd_micros;
    valid
        .then_some(())
        .ok_or(GroupAgentNodeDispatchExecutionServiceError::ProviderUnavailable)
}

fn collection_limits(
    contract: &crate::runtime_domain::GroupAgentNodeExecutionContract,
) -> DispatchCollectionLimits {
    DispatchCollectionLimits {
        model_output_bytes: contract.budgets.max_model_output_bytes,
        result_bytes: contract.result.max_result_bytes,
        output_tokens: contract.budgets.max_output_tokens,
        events: contract.budgets.max_model_events,
        timeout_ms: contract.budgets.timeout_ms,
    }
}
