use std::sync::Arc;

use crate::runtime_domain::{
    GroupAgentGraphStore, GroupAgentNodeCoreTerminalReceiptPort, GroupAgentNodeDispatchRequestStore,
    GroupAgentNodeLifecycleInspection, GroupAgentNodeLifecycleStore,
};

use crate::{
    GroupAgentNodeDispatchMetadataSource, GroupAgentNodeDispatchReleaseControlService,
    GroupAgentNodeDispatchRequestCodec,
};


#[path = "error.rs"]
mod error;
pub use error::{AdjudicationRefused, GroupAgentNodeDispatchAdjudicationServiceError};

#[derive(Clone, Debug)]
pub struct AdjudicateGroupAgentNodeDispatchInput {
    pub graph_run_id: String,
    pub authorization_json: String,
    pub pricing_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdjudicateGroupAgentNodeDispatchResult {
    Adjudicated(GroupAgentNodeLifecycleInspection),
}

/// Operator-invoked one-shot remedy for a hard-crash-quarantined v4 claim.
///
/// The constructor takes **only** store/codec/bridge/metadata — no provider
/// factory and no credential source — so "no credential" is a type-level
/// guarantee: there is no code path that can read an API key or construct a
/// provider stream. The only database writes are confined to the terminalize
/// CAS transaction. Adjudication never sends a provider request; it writes a
/// deterministic `failed_uncertain` terminal from an operator-constructed
/// `HardCrash` no-evidence artifact.
///
/// Re-entry convention (settled, REFUSE): any invocation that does not observe
/// a stranded claim (run `dispatch_unknown` with claim, active lane, and no
/// artifact/receipt) returns `AdjudicationRefused::NotStranded` with zero
/// mutation — the closest analogue is the scheduled-family adjudication and
/// the v4 terminalize `reject_terminal_replay`, both of which refuse rather
/// than no-op.
pub struct GroupAgentNodeDispatchAdjudicationService {
    release: GroupAgentNodeDispatchReleaseControlService,
    lifecycles: Arc<dyn GroupAgentNodeLifecycleStore>,
    core: Arc<dyn GroupAgentNodeCoreTerminalReceiptPort>,
    metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
}

impl GroupAgentNodeDispatchAdjudicationService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        requests: Arc<dyn GroupAgentNodeDispatchRequestStore>,
        lifecycles: Arc<dyn GroupAgentNodeLifecycleStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
        core: Arc<dyn GroupAgentNodeCoreTerminalReceiptPort>,
        metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
    ) -> Self {
        Self {
            release: GroupAgentNodeDispatchReleaseControlService::new(graphs, requests, codec),
            lifecycles,
            core,
            metadata,
        }
    }

    /// Adjudicates one stranded claim into a deterministic terminal, or refuses.
    ///
    /// Ordering: stranded guard → digest preflight → build no-evidence
    /// `HardCrash` control → pinned Core decision → single `BEGIN IMMEDIATE`
    /// terminalize CAS. Refusals are zero-mutation and retryable.
    ///
    /// # Errors
    ///
    /// Returns a distinct `AdjudicationRefused` cause for every refusal, or a
    /// structured input/store error. It never produces `DispatchQuarantined`.
    pub fn adjudicate(
        &self,
        input: &AdjudicateGroupAgentNodeDispatchInput,
    ) -> Result<
        AdjudicateGroupAgentNodeDispatchResult,
        GroupAgentNodeDispatchAdjudicationServiceError,
    > {
        adjudicate(self, input)
    }
}

#[path = "build.rs"]
mod build;
use build::build_hard_crash_artifact;
use crate::group_agent_node_dispatch_execution::build::{
    build_terminal_control, build_terminalize_request,
};
use crate::runtime_domain::{
    GroupAgentGraphRunStatus, GroupAgentNodeDispatchAuthorization, GroupAgentNodePricingSnapshot,
    GroupAgentNodeTerminalControl, HubStoreError, HubStoreError::Conflict,
    MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES, TerminalizeGroupAgentNodeDispatch,
};



pub(super) fn adjudicate(
    service: &GroupAgentNodeDispatchAdjudicationService,
    input: &AdjudicateGroupAgentNodeDispatchInput,
) -> Result<
    AdjudicateGroupAgentNodeDispatchResult,
    GroupAgentNodeDispatchAdjudicationServiceError,
> {
    let inspection = service
        .lifecycles
        .inspect_group_agent_node_lifecycle(&input.graph_run_id)
        .map_err(store_error)?;
    ensure_stranded(&inspection)?;
    let authorization = decode_authorization(input)?;
    let pricing = decode_pricing(input)?;
    ensure_claimed_digest(
        &inspection.claim.authorization_sha256,
        &authorization.authorization_sha256,
        "authorization",
    )?;
    ensure_claimed_digest(
        &inspection.claim.pricing_snapshot_sha256,
        &pricing.pricing_snapshot_sha256,
        "pricing",
    )?;
    let terminal = build_terminal(service, input, &inspection, authorization, pricing)?;
    persist_terminal(service, &terminal)
}

fn build_terminal(
    service: &GroupAgentNodeDispatchAdjudicationService,
    input: &AdjudicateGroupAgentNodeDispatchInput,
    inspection: &GroupAgentNodeLifecycleInspection,
    authorization: GroupAgentNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
) -> Result<TerminalizeGroupAgentNodeDispatch, GroupAgentNodeDispatchAdjudicationServiceError> {
    let export = service
        .release
        .export(&input.graph_run_id)
        .map_err(|_| GroupAgentNodeDispatchAdjudicationServiceError::InvalidState)?;
    let terminalized_at_ms = service
        .metadata
        .terminal_time_ms()
        .map_err(|_| GroupAgentNodeDispatchAdjudicationServiceError::InvalidState)?;
    let artifact = build_hard_crash_artifact(&inspection.claim, terminalized_at_ms)?;
    let control = build_terminal_control(
        &export.release_control,
        authorization,
        pricing,
        inspection,
        artifact,
    )
    .map_err(|_| GroupAgentNodeDispatchAdjudicationServiceError::InvalidInput)?;
    decide(service, control, terminalized_at_ms)
}

fn decide(
    service: &GroupAgentNodeDispatchAdjudicationService,
    control: GroupAgentNodeTerminalControl,
    terminalized_at_ms: u64,
) -> Result<TerminalizeGroupAgentNodeDispatch, GroupAgentNodeDispatchAdjudicationServiceError> {
    let envelope = service
        .core
        .decide(&control)
        .map_err(|error| AdjudicationRefused::CoreRefused {
            detail: error.message,
        })?;
    build_terminalize_request(control, envelope, terminalized_at_ms).map_err(|_| {
        AdjudicationRefused::CoreRefused {
            detail: "Core receipt does not match the hard-crash control".into(),
        }
        .into()
    })
}

fn persist_terminal(
    service: &GroupAgentNodeDispatchAdjudicationService,
    terminal: &TerminalizeGroupAgentNodeDispatch,
) -> Result<
    AdjudicateGroupAgentNodeDispatchResult,
    GroupAgentNodeDispatchAdjudicationServiceError,
> {
    let result = service
        .lifecycles
        .terminalize_group_agent_node_dispatch(terminal)
        .map_err(|error| -> GroupAgentNodeDispatchAdjudicationServiceError {
            match error {
                Conflict { .. } => AdjudicationRefused::CasConflict.into(),
                other => other.into(),
            }
        })?;
    Ok(AdjudicateGroupAgentNodeDispatchResult::Adjudicated(
        result.inspection,
    ))
}

fn ensure_stranded(
    inspection: &GroupAgentNodeLifecycleInspection,
) -> Result<(), GroupAgentNodeDispatchAdjudicationServiceError> {
    let stranded = inspection.graph_run.run.status == GroupAgentGraphRunStatus::DispatchUnknown
        && inspection.active_lane.is_some()
        && inspection.artifact.is_none()
        && inspection.terminal_receipt.is_none();
    if stranded {
        return Ok(());
    }
    Err(AdjudicationRefused::NotStranded {
        reason: format!(
            "{} is not a stranded hard-crash claim (status={}, lane_active={}, \
             artifact_present={}, receipt_present={})",
            inspection.graph_run.run.graph_run_id,
            status_text(inspection.graph_run.run.status),
            inspection.active_lane.is_some(),
            inspection.artifact.is_some(),
            inspection.terminal_receipt.is_some(),
        ),
    }
    .into())
}

fn store_error(
    error: HubStoreError,
) -> GroupAgentNodeDispatchAdjudicationServiceError {
    match error {
        HubStoreError::NotFound { .. } => AdjudicationRefused::NotStranded {
            reason: "run has no stranded dispatch claim".into(),
        }
        .into(),
        other => other.into(),
    }
}

fn decode_authorization(
    input: &AdjudicateGroupAgentNodeDispatchInput,
) -> Result<GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchAdjudicationServiceError>
{
    if !(1..=MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES)
        .contains(&input.authorization_json.len())
    {
        return Err(GroupAgentNodeDispatchAdjudicationServiceError::InvalidInput);
    }
    let authorization = GroupAgentNodeDispatchAuthorization::decode_exact(
        &input.authorization_json,
    )
    .map_err(|_| GroupAgentNodeDispatchAdjudicationServiceError::InvalidInput)?;
    authorization
        .validate()
        .map_err(|_| GroupAgentNodeDispatchAdjudicationServiceError::InvalidInput)?;
    Ok(authorization)
}

fn decode_pricing(
    input: &AdjudicateGroupAgentNodeDispatchInput,
) -> Result<GroupAgentNodePricingSnapshot, GroupAgentNodeDispatchAdjudicationServiceError> {
    GroupAgentNodePricingSnapshot::decode_exact(&input.pricing_json)
        .map_err(|_| GroupAgentNodeDispatchAdjudicationServiceError::InvalidInput)
}

fn ensure_claimed_digest(
    claimed: &str,
    operator: &str,
    field: &'static str,
) -> Result<(), GroupAgentNodeDispatchAdjudicationServiceError> {
    if claimed == operator {
        Ok(())
    } else {
        Err(AdjudicationRefused::DigestMismatch { field }.into())
    }
}

fn status_text(status: GroupAgentGraphRunStatus) -> &'static str {
    match status {
        GroupAgentGraphRunStatus::AwaitingExecutionContract => "awaiting_execution_contract",
        GroupAgentGraphRunStatus::AwaitingCoreDispatch => "awaiting_core_dispatch",
        GroupAgentGraphRunStatus::AwaitingDispatchAuthorization => {
            "awaiting_dispatch_authorization"
        }
        GroupAgentGraphRunStatus::DispatchUnknown => "dispatch_unknown",
        GroupAgentGraphRunStatus::Completed => "completed",
        GroupAgentGraphRunStatus::Failed => "failed",
        GroupAgentGraphRunStatus::FailedUncertain => "failed_uncertain",
    }
}
