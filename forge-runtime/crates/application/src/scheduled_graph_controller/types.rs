use std::sync::Arc;

use crate::ExecuteGroupAgentScheduledReadyNodeDispatchResult;
use crate::runtime_domain::{
    Cancellation, ScheduledGraphControllerEventPayload, ScheduledGraphControllerExecutionProfile,
    ScheduledGraphControllerJournal, ScheduledGraphControllerRetryableFailure,
    ScheduledGraphControllerStopReason,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartScheduledGraphControllerInput {
    pub graph_run_id: String,
    pub expected_schedule_sha256: String,
    pub core_bin_sha256: String,
    pub execution_profile: ScheduledGraphControllerExecutionProfile,
    pub max_effectful_steps: u16,
    pub max_total_cost_usd_micros: u64,
    pub observed_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvanceScheduledGraphControllerInput {
    pub graph_run_id: String,
    pub core_bin_sha256: String,
    pub observed_at_ms: u64,
}

pub struct StepScheduledGraphControllerInput {
    pub graph_run_id: String,
    pub core_bin_sha256: String,
    pub expected_awaiting_event_sha256: String,
    pub expected_provider_request_id: String,
    pub expected_authorization_sha256: String,
    pub pricing_source: Arc<dyn ScheduledGraphControllerPricingSource>,
    pub confirm_off_machine: bool,
    pub confirm_predecessor_content: bool,
    pub cancellation: Cancellation,
    pub observed_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduledGraphControllerPricingSourceError;

pub trait ScheduledGraphControllerClock: Send + Sync {
    /// Returns the current Unix time in milliseconds for durable event evidence.
    fn now_ms(&self) -> u64;
}

pub trait ScheduledGraphControllerPricingSource: Send + Sync {
    /// Reads the caller-selected pricing artifact only when a dispatch may proceed.
    ///
    /// # Errors
    ///
    /// Returns an error when the artifact cannot be read in full.
    fn read_pricing_json(&self) -> Result<String, ScheduledGraphControllerPricingSourceError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScheduledGraphControllerState {
    PassiveRecovery {
        phase: ScheduledGraphControllerRecoveryPhase,
    },
    AwaitingFreshConsent {
        awaiting_event_sha256: String,
        execution_ordinal: usize,
        node_id: String,
        provider_request_id: String,
        authorization_sha256: String,
        snapshot_sha256: String,
        decision_sha256: String,
        predecessor_content_included: bool,
    },
    Stopped {
        reason: ScheduledGraphControllerStopReason,
        provider_request_id: Option<String>,
    },
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledGraphControllerRecoveryPhase {
    Started,
    Materialize,
    Prepare,
    Dispatch,
    Observe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledGraphControllerOutput {
    pub journal: ScheduledGraphControllerJournal,
    pub state: ScheduledGraphControllerState,
    pub invocation: Option<ExecuteGroupAgentScheduledReadyNodeDispatchResult>,
    pub retryable_failure: Option<ScheduledGraphControllerRetryableFailure>,
    pub post_invocation_error: Option<super::ScheduledGraphControllerServiceError>,
    pub journal_current_observed: bool,
}

impl ScheduledGraphControllerOutput {
    pub(super) fn passive(journal: ScheduledGraphControllerJournal) -> Self {
        Self {
            state: state_from(&journal),
            journal,
            invocation: None,
            retryable_failure: None,
            post_invocation_error: None,
            journal_current_observed: true,
        }
    }
}

pub(super) fn state_from(
    journal: &ScheduledGraphControllerJournal,
) -> ScheduledGraphControllerState {
    use ScheduledGraphControllerEventPayload as Payload;
    let head = journal.head();
    match &head.payload {
        Payload::AwaitingFreshConsent {
            execution_ordinal,
            node_id,
            provider_request_id,
            authorization_sha256,
            snapshot_sha256,
            decision_sha256,
            predecessor_content_included,
        } => ScheduledGraphControllerState::AwaitingFreshConsent {
            awaiting_event_sha256: head.event_sha256.clone(),
            execution_ordinal: *execution_ordinal,
            node_id: node_id.clone(),
            provider_request_id: provider_request_id.clone(),
            authorization_sha256: authorization_sha256.clone(),
            snapshot_sha256: snapshot_sha256.clone(),
            decision_sha256: decision_sha256.clone(),
            predecessor_content_included: *predecessor_content_included,
        },
        Payload::Stopped {
            reason,
            provider_request_id,
            ..
        } => ScheduledGraphControllerState::Stopped {
            reason: *reason,
            provider_request_id: provider_request_id.clone(),
        },
        Payload::Completed { .. } => ScheduledGraphControllerState::Completed,
        payload => ScheduledGraphControllerState::PassiveRecovery {
            phase: recovery_phase(payload),
        },
    }
}

fn recovery_phase(
    payload: &ScheduledGraphControllerEventPayload,
) -> ScheduledGraphControllerRecoveryPhase {
    use ScheduledGraphControllerEventPayload as Payload;
    match payload {
        Payload::Started { .. } => ScheduledGraphControllerRecoveryPhase::Started,
        Payload::MaterializePlanned { .. } | Payload::MaterializeObserved { .. } => {
            ScheduledGraphControllerRecoveryPhase::Materialize
        }
        Payload::PreparePlanned { .. } | Payload::PrepareObserved { .. } => {
            ScheduledGraphControllerRecoveryPhase::Prepare
        }
        Payload::DispatchPlanned { .. } | Payload::RetryablePreclaimFailure { .. } => {
            ScheduledGraphControllerRecoveryPhase::Dispatch
        }
        Payload::NodeCompleted { .. } => ScheduledGraphControllerRecoveryPhase::Observe,
        Payload::AwaitingFreshConsent { .. }
        | Payload::Stopped { .. }
        | Payload::Completed { .. } => unreachable!("terminal states handled above"),
    }
}
