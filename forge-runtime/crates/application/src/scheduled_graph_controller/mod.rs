use std::{future::Future, pin::Pin, sync::Arc};

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore, GroupAgentGraphStore,
    GroupAgentNodeExecutionContractStore, GroupAgentScheduledNodeAnyLifecycleInspectionStore,
    GroupAgentScheduledNodeContractStore, GroupAgentScheduledNodeProviderRequestStore,
    GroupAgentScheduledNodeSuccessorStore, HubStoreError,
    MAX_SCHEDULED_GRAPH_CONTROLLER_EFFECTFUL_STEPS, SCHEDULED_GRAPH_CONTROLLER_PROTOCOL_VERSION,
    SCHEDULED_GRAPH_CONTROLLER_VERSION, ScheduledGraphControllerEvent,
    ScheduledGraphControllerEventPayload, ScheduledGraphControllerHeader,
    ScheduledGraphControllerStore, ScheduledGraphNodeMaterializationPort,
    ScheduledGraphProgressStore, ScheduledGraphReconcilePort, ScheduledReadyNodeReleasePort,
    ScheduledReadyNodeReleaseStore,
};
use crate::{
    ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    ExecuteGroupAgentScheduledReadyNodeDispatchResult, GroupAgentGraphExecutionScheduleService,
    GroupAgentGraphExecutionScheduleServiceError, GroupAgentNodeDispatchRequestCodec,
    GroupAgentScheduledReadyNodeDispatchExecutionService,
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError, ScheduledGraphReconcileObservation,
    ScheduledGraphReconcileService,
};

mod compatibility;
mod drive;
mod error;
mod events;
mod materialize;
mod recovery;
mod source_validation;
mod step;
mod storage;
mod types;
mod validation;

#[cfg(test)]
#[path = "test_scheduled_graph_controller.rs"]
mod tests;

pub use error::ScheduledGraphControllerServiceError;
pub use types::*;
use validation::{validate_digest, validate_identifier};

pub trait ScheduledGraphControllerHub:
    ScheduledGraphControllerStore
    + ScheduledGraphProgressStore
    + ScheduledReadyNodeReleaseStore
    + GroupAgentGraphExecutionScheduleStore
    + GroupAgentGraphStore
    + GroupAgentGraphRunStore
    + GroupAgentNodeExecutionContractStore
    + GroupAgentScheduledNodeContractStore
    + GroupAgentScheduledNodeSuccessorStore
    + GroupAgentScheduledNodeProviderRequestStore
    + GroupAgentScheduledNodeAnyLifecycleInspectionStore
    + Send
    + Sync
{
}

impl<T> ScheduledGraphControllerHub for T where
    T: ScheduledGraphControllerStore
        + ScheduledGraphProgressStore
        + ScheduledReadyNodeReleaseStore
        + GroupAgentGraphExecutionScheduleStore
        + GroupAgentGraphStore
        + GroupAgentGraphRunStore
        + GroupAgentNodeExecutionContractStore
        + GroupAgentScheduledNodeContractStore
        + GroupAgentScheduledNodeSuccessorStore
        + GroupAgentScheduledNodeProviderRequestStore
        + GroupAgentScheduledNodeAnyLifecycleInspectionStore
        + Send
        + Sync
{
}

type StepFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<
                    ExecuteGroupAgentScheduledReadyNodeDispatchResult,
                    GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
                >,
            > + Send
            + 'a,
    >,
>;

pub trait ScheduledGraphReadyNodeExecutor: Send + Sync {
    fn execute<'a>(
        &'a self,
        input: &'a ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    ) -> StepFuture<'a>;
}

impl ScheduledGraphReadyNodeExecutor for GroupAgentScheduledReadyNodeDispatchExecutionService {
    fn execute<'a>(
        &'a self,
        input: &'a ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    ) -> StepFuture<'a> {
        Box::pin(GroupAgentScheduledReadyNodeDispatchExecutionService::execute(self, input))
    }
}

pub struct ScheduledGraphControllerService {
    hub: Arc<dyn ScheduledGraphControllerHub>,
    reconcile: Arc<dyn ScheduledGraphReconcilePort>,
    authorize: Arc<dyn ScheduledReadyNodeReleasePort>,
    materializer: Arc<dyn ScheduledGraphNodeMaterializationPort>,
    codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
    executor: Option<Arc<dyn ScheduledGraphReadyNodeExecutor>>,
    clock: Arc<dyn ScheduledGraphControllerClock>,
}

pub struct ScheduledGraphControllerQueryService {
    store: Arc<dyn ScheduledGraphControllerStore>,
}

impl ScheduledGraphControllerQueryService {
    #[must_use]
    pub fn new(store: Arc<dyn ScheduledGraphControllerStore>) -> Self {
        Self { store }
    }

    /// Validates an inspection identifier without reading storage.
    ///
    /// # Errors
    ///
    /// Returns `InvalidInput` when the identifier is not domain-safe.
    pub fn preflight_inspect(
        graph_run_id: &str,
    ) -> Result<(), ScheduledGraphControllerServiceError> {
        validate_identifier(graph_run_id)
    }

    /// Loads one exact journal without calling Core or performing an effect.
    ///
    /// # Errors
    ///
    /// Returns a redacted input or storage error.
    pub fn inspect(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        Self::preflight_inspect(graph_run_id)?;
        self.store
            .inspect_scheduled_graph_controller(graph_run_id)
            .map(ScheduledGraphControllerOutput::passive)
            .map_err(|error| map_hub_error(&error))
    }
}

impl ScheduledGraphControllerService {
    #[must_use]
    pub fn new(
        hub: Arc<dyn ScheduledGraphControllerHub>,
        reconcile: Arc<dyn ScheduledGraphReconcilePort>,
        authorize: Arc<dyn ScheduledReadyNodeReleasePort>,
        materializer: Arc<dyn ScheduledGraphNodeMaterializationPort>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
        executor: Arc<dyn ScheduledGraphReadyNodeExecutor>,
        clock: Arc<dyn ScheduledGraphControllerClock>,
    ) -> Self {
        Self {
            hub,
            reconcile,
            authorize,
            materializer,
            codec,
            executor: Some(executor),
            clock,
        }
    }

    #[must_use]
    pub fn new_passive(
        hub: Arc<dyn ScheduledGraphControllerHub>,
        reconcile: Arc<dyn ScheduledGraphReconcilePort>,
        authorize: Arc<dyn ScheduledReadyNodeReleasePort>,
        materializer: Arc<dyn ScheduledGraphNodeMaterializationPort>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
        clock: Arc<dyn ScheduledGraphControllerClock>,
    ) -> Self {
        Self {
            hub,
            reconcile,
            authorize,
            materializer,
            codec,
            executor: None,
            clock,
        }
    }

    /// Validates every start field that is independent of durable Graph state.
    ///
    /// # Errors
    ///
    /// Returns `InvalidInput` before a caller opens storage or invokes Core.
    pub fn preflight_start(
        input: &StartScheduledGraphControllerInput,
    ) -> Result<(), ScheduledGraphControllerServiceError> {
        validate_start_input(input)
    }

    /// Validates advance arguments without reading storage or invoking Core.
    /// # Errors
    /// Returns `InvalidInput` for malformed identifiers, pins, or timestamps.
    pub fn preflight_advance(
        input: &AdvanceScheduledGraphControllerInput,
    ) -> Result<(), ScheduledGraphControllerServiceError> {
        validate_reentry_input(
            &input.graph_run_id,
            &input.core_bin_sha256,
            input.observed_at_ms,
        )
    }

    /// Validates step arguments without reading storage or invoking Core.
    /// # Errors
    /// Returns `InvalidInput` for malformed identifiers, anchors, pins, or timestamps.
    pub fn preflight_step(
        input: &StepScheduledGraphControllerInput,
    ) -> Result<(), ScheduledGraphControllerServiceError> {
        validate_reentry_input(
            &input.graph_run_id,
            &input.core_bin_sha256,
            input.observed_at_ms,
        )?;
        validate_digest(&input.expected_awaiting_event_sha256)?;
        validate_identifier(&input.expected_provider_request_id)?;
        validate_digest(&input.expected_authorization_sha256)
    }

    /// Starts or reenters one immutable controller and passively advances it.
    ///
    /// # Errors
    ///
    /// Returns a redacted validation, Core, storage, or passive-operation error.
    pub fn start(
        &self,
        input: &StartScheduledGraphControllerInput,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        Self::preflight_start(input)?;
        if let Some(journal) = self.existing(&input.graph_run_id)? {
            validate_reentry(&journal, &input.core_bin_sha256, input.observed_at_ms)?;
            validate_existing_start(&journal.header, input)?;
            return self.drive(journal, input.observed_at_ms);
        }
        let observation = self.observe(&input.graph_run_id)?;
        let schedule = self.load_schedule(&observation)?;
        validate_schedule_start(input, &observation, &schedule)?;
        let header = build_header(input, &observation)?;
        self.validate_start_source(&header, &observation)?;
        let started_at_ms = self.event_time(None, input.observed_at_ms);
        let started = start_event(&header, &observation, started_at_ms)?;
        let result = match self.hub.start_scheduled_graph_controller(&header, &started) {
            Ok(result) => result,
            Err(HubStoreError::Conflict { .. }) => {
                return Err(self.classify_concurrent_start(&header));
            }
            Err(error) => return Err(map_hub_error(&error)),
        };
        validate_start_result(&header, &started, &result.journal)?;
        let reentry_at_ms = self.sample_time(input.observed_at_ms);
        validate_reentry(&result.journal, &input.core_bin_sha256, reentry_at_ms)?;
        self.drive(result.journal, input.observed_at_ms)
    }

    /// Reconciles and passively advances a previously started controller.
    ///
    /// # Errors
    ///
    /// Returns a redacted validation, Core, storage, or passive-operation error.
    pub fn advance(
        &self,
        input: &AdvanceScheduledGraphControllerInput,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        Self::preflight_advance(input)?;
        let journal = self.load(&input.graph_run_id)?;
        validate_reentry(&journal, &input.core_bin_sha256, input.observed_at_ms)?;
        self.drive(journal, input.observed_at_ms)
    }

    /// Loads the exact journal without calling Core or performing an effect.
    ///
    /// # Errors
    ///
    /// Returns a redacted input or storage error.
    pub fn inspect(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        validate_identifier(graph_run_id)?;
        self.load(graph_run_id)
            .map(ScheduledGraphControllerOutput::passive)
    }

    fn observe(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphReconcileObservation, ScheduledGraphControllerServiceError> {
        ScheduledGraphReconcileService::new(self.hub.clone(), self.reconcile.clone())
            .observe(graph_run_id)
            .map_err(|_| ScheduledGraphControllerServiceError::ReconcileFailed)
    }

    fn load_schedule(
        &self,
        observation: &ScheduledGraphReconcileObservation,
    ) -> Result<
        crate::GroupAgentGraphExecutionScheduleInspection,
        ScheduledGraphControllerServiceError,
    > {
        GroupAgentGraphExecutionScheduleService::new(
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
        )
        .inspect(&observation.snapshot.schedule_id)
        .map_err(|error| map_start_schedule_error(&error))
    }
}

fn map_hub_error(error: &HubStoreError) -> ScheduledGraphControllerServiceError {
    match error {
        HubStoreError::Conflict { .. } | HubStoreError::Corrupt { .. } => {
            ScheduledGraphControllerServiceError::CorruptEvidence
        }
        HubStoreError::NotFound { .. } | HubStoreError::Unavailable { .. } => {
            ScheduledGraphControllerServiceError::StoreUnavailable
        }
    }
}

fn map_start_schedule_error(
    error: &GroupAgentGraphExecutionScheduleServiceError,
) -> ScheduledGraphControllerServiceError {
    match error {
        GroupAgentGraphExecutionScheduleServiceError::NotFound { .. }
        | GroupAgentGraphExecutionScheduleServiceError::Unavailable { .. } => {
            ScheduledGraphControllerServiceError::StoreUnavailable
        }
        GroupAgentGraphExecutionScheduleServiceError::InvalidInput { .. }
        | GroupAgentGraphExecutionScheduleServiceError::Conflict { .. }
        | GroupAgentGraphExecutionScheduleServiceError::Corrupt { .. } => {
            ScheduledGraphControllerServiceError::CorruptEvidence
        }
    }
}

fn validate_start_input(
    input: &StartScheduledGraphControllerInput,
) -> Result<(), ScheduledGraphControllerServiceError> {
    validate_identifier(&input.graph_run_id)?;
    validate_digest(&input.expected_schedule_sha256)?;
    validate_digest(&input.core_bin_sha256)?;
    input
        .execution_profile
        .validate()
        .map_err(|_| ScheduledGraphControllerServiceError::InvalidInput)?;
    let maximum_cost = input
        .execution_profile
        .max_cost_usd_micros
        .saturating_mul(u64::from(input.max_effectful_steps));
    let valid = i64::try_from(input.observed_at_ms).is_ok()
        && (1..=MAX_SCHEDULED_GRAPH_CONTROLLER_EFFECTFUL_STEPS)
            .contains(&input.max_effectful_steps)
        && input.max_total_cost_usd_micros >= input.execution_profile.max_cost_usd_micros
        && input.max_total_cost_usd_micros <= maximum_cost;
    valid
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::InvalidInput)
}

fn validate_reentry(
    journal: &crate::runtime_domain::ScheduledGraphControllerJournal,
    core_bin_sha256: &str,
    observed_at_ms: u64,
) -> Result<(), ScheduledGraphControllerServiceError> {
    validate_digest(core_bin_sha256)?;
    if journal.header.core_bin_sha256 != core_bin_sha256 {
        return Err(ScheduledGraphControllerServiceError::CorePinMismatch);
    }
    i64::try_from(observed_at_ms)
        .is_ok()
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::InvalidInput)
}

fn validate_reentry_input(
    graph_run_id: &str,
    core_bin_sha256: &str,
    observed_at_ms: u64,
) -> Result<(), ScheduledGraphControllerServiceError> {
    validate_identifier(graph_run_id)?;
    validate_digest(core_bin_sha256)?;
    i64::try_from(observed_at_ms)
        .map(|_| ())
        .map_err(|_| ScheduledGraphControllerServiceError::InvalidInput)
}

fn validate_existing_start(
    header: &ScheduledGraphControllerHeader,
    input: &StartScheduledGraphControllerInput,
) -> Result<(), ScheduledGraphControllerServiceError> {
    let valid = header.graph_run_id == input.graph_run_id
        && header.schedule_sha256 == input.expected_schedule_sha256
        && header.core_bin_sha256 == input.core_bin_sha256
        && header.execution_profile == input.execution_profile
        && header.max_effectful_steps == input.max_effectful_steps
        && header.max_total_cost_usd_micros == input.max_total_cost_usd_micros;
    valid
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::InvalidInput)
}

fn validate_schedule_start(
    input: &StartScheduledGraphControllerInput,
    observation: &ScheduledGraphReconcileObservation,
    schedule: &crate::GroupAgentGraphExecutionScheduleInspection,
) -> Result<(), ScheduledGraphControllerServiceError> {
    let snapshot = &observation.snapshot;
    let valid = snapshot.schedule_sha256 == input.expected_schedule_sha256
        && schedule.record.schedule_id == snapshot.schedule_id
        && schedule.record.schedule_sha256 == snapshot.schedule_sha256
        && schedule.record.graph_run_id == input.graph_run_id
        && schedule.schedule.node_count == snapshot.node_count
        && schedule.schedule.max_in_flight_nodes == 1
        && usize::from(input.max_effectful_steps) <= snapshot.node_count;
    valid
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::IncompatibleSchedule)
}

fn build_header(
    input: &StartScheduledGraphControllerInput,
    observation: &ScheduledGraphReconcileObservation,
) -> Result<ScheduledGraphControllerHeader, ScheduledGraphControllerServiceError> {
    ScheduledGraphControllerHeader {
        v: SCHEDULED_GRAPH_CONTROLLER_VERSION,
        controller_protocol_version: SCHEDULED_GRAPH_CONTROLLER_PROTOCOL_VERSION,
        graph_run_id: input.graph_run_id.clone(),
        schedule_id: observation.snapshot.schedule_id.clone(),
        schedule_sha256: observation.snapshot.schedule_sha256.clone(),
        schedule_version: crate::GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        progress_protocol_version: observation.snapshot.progress_protocol_version,
        core_bin_sha256: input.core_bin_sha256.clone(),
        node_count: observation.snapshot.node_count,
        max_effectful_steps: input.max_effectful_steps,
        max_total_cost_usd_micros: input.max_total_cost_usd_micros,
        execution_profile: input.execution_profile.clone(),
        created_at_ms: input.observed_at_ms,
        controller_id: String::new(),
        controller_sha256: String::new(),
    }
    .seal()
    .map_err(|_| ScheduledGraphControllerServiceError::InvalidInput)
}

fn start_event(
    header: &ScheduledGraphControllerHeader,
    observation: &ScheduledGraphReconcileObservation,
    created_at_ms: u64,
) -> Result<ScheduledGraphControllerEvent, ScheduledGraphControllerServiceError> {
    ScheduledGraphControllerEvent {
        v: SCHEDULED_GRAPH_CONTROLLER_VERSION,
        controller_id: header.controller_id.clone(),
        graph_run_id: header.graph_run_id.clone(),
        sequence: 1,
        previous_event_sha256: None,
        payload: ScheduledGraphControllerEventPayload::Started {
            snapshot_sha256: observation.snapshot.snapshot_sha256.clone(),
            decision_sha256: observation.decision.decision_sha256.clone(),
        },
        created_at_ms,
        event_sha256: String::new(),
    }
    .seal()
    .map_err(|_| ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn validate_start_result(
    header: &ScheduledGraphControllerHeader,
    event: &ScheduledGraphControllerEvent,
    journal: &crate::runtime_domain::ScheduledGraphControllerJournal,
) -> Result<(), ScheduledGraphControllerServiceError> {
    (journal.validate().is_ok()
        && journal.header == *header
        && journal.events.first() == Some(event))
    .then_some(())
    .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}
