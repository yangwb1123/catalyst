use std::sync::Arc;

use thiserror::Error;

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
    GroupAgentScheduledReadyNodeDispatchAuthorization,
    GroupAgentScheduledReadyNodeDispatchReleaseControl, HubStoreError,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, ScheduledGraphProgressSnapshot,
    ScheduledGraphProgressStore, ScheduledGraphReconcileDecision,
    ScheduledGraphReconcileDisposition, ScheduledGraphReconcilePort,
    ScheduledGraphReconcilePortError, ScheduledReadyNodeReleasePort,
    ScheduledReadyNodeReleasePortError, ScheduledReadyNodeReleaseSource,
    ScheduledReadyNodeReleaseStore,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ScheduledReadyNodeReleaseServiceError {
    #[error("scheduled ready release input is invalid")]
    InvalidInput,
    #[error("scheduled ready release source was not found")]
    NotFound,
    #[error("scheduled ready release storage is unavailable")]
    StorageUnavailable,
    #[error("scheduled ready release durable source is corrupt")]
    CorruptSource,
    #[error("scheduled ready release Core reconcile is unavailable")]
    ReconcileUnavailable,
    #[error("scheduled ready release Core decision is invalid")]
    InvalidReconcileDecision,
    #[error("scheduled Graph has no releasable ready node")]
    NotReady,
    #[error("scheduled ready release Core authorization is unavailable")]
    AuthorizationUnavailable,
    #[error("scheduled ready release Core authorization is invalid")]
    InvalidAuthorization,
    #[error("scheduled ready release source changed during authorization")]
    SourceChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedScheduledReadyNodeRelease {
    pub release_control: GroupAgentScheduledReadyNodeDispatchReleaseControl,
    pub authorization: GroupAgentScheduledReadyNodeDispatchAuthorization,
}

pub struct ScheduledReadyNodeReleaseService {
    progress: Arc<dyn ScheduledGraphProgressStore>,
    sources: Arc<dyn ScheduledReadyNodeReleaseStore>,
    reconcile: Arc<dyn ScheduledGraphReconcilePort>,
    authorize: Arc<dyn ScheduledReadyNodeReleasePort>,
}

impl ScheduledReadyNodeReleaseService {
    #[must_use]
    pub fn new(
        progress: Arc<dyn ScheduledGraphProgressStore>,
        sources: Arc<dyn ScheduledReadyNodeReleaseStore>,
        reconcile: Arc<dyn ScheduledGraphReconcilePort>,
        authorize: Arc<dyn ScheduledReadyNodeReleasePort>,
    ) -> Self {
        Self {
            progress,
            sources,
            reconcile,
            authorize,
        }
    }

    /// Issues a staleable, future-only policy after exact A/Core/B validation.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed error when the durable source, either Core result,
    /// or the second atomic source bundle is unavailable, invalid, or changed.
    pub fn authorize(
        &self,
        graph_run_id: &str,
    ) -> Result<AuthorizedScheduledReadyNodeRelease, ScheduledReadyNodeReleaseServiceError> {
        validate_identifier(graph_run_id)?;
        let snapshot = self.load_progress(graph_run_id)?;
        let decision = self.reconcile(&snapshot)?;
        let (ordinal, node_id) = ready_selection(&decision)?;
        let source_a = self.load_source(&snapshot, ordinal, node_id)?;
        let control_a = build_control(&source_a, &decision)?;
        let authorization = self
            .authorize
            .authorize(&control_a)
            .map_err(map_authorization_error)?;
        let source_b = self.load_source(&snapshot, ordinal, node_id)?;
        if source_a != source_b {
            return Err(ScheduledReadyNodeReleaseServiceError::SourceChanged);
        }
        let control_b = build_control(&source_b, &decision)?;
        if control_a != control_b {
            return Err(ScheduledReadyNodeReleaseServiceError::SourceChanged);
        }
        authorization
            .validate_against_release_control(&control_b)
            .map_err(|_| ScheduledReadyNodeReleaseServiceError::InvalidAuthorization)?;
        Ok(AuthorizedScheduledReadyNodeRelease {
            release_control: control_b,
            authorization,
        })
    }

    fn load_progress(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphProgressSnapshot, ScheduledReadyNodeReleaseServiceError> {
        let snapshot = self
            .progress
            .snapshot_scheduled_graph_progress(graph_run_id)
            .map_err(|error| map_store_error(&error))?;
        snapshot
            .validate()
            .map_err(|_| ScheduledReadyNodeReleaseServiceError::CorruptSource)?;
        (snapshot.graph_run_id == graph_run_id)
            .then_some(snapshot)
            .ok_or(ScheduledReadyNodeReleaseServiceError::CorruptSource)
    }

    fn reconcile(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledReadyNodeReleaseServiceError> {
        let decision = self
            .reconcile
            .decide(snapshot)
            .map_err(map_reconcile_error)?;
        decision
            .validate_against_snapshot(snapshot)
            .map_err(|_| ScheduledReadyNodeReleaseServiceError::InvalidReconcileDecision)?;
        Ok(decision)
    }

    fn load_source(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
        ordinal: usize,
        node_id: &str,
    ) -> Result<ScheduledReadyNodeReleaseSource, ScheduledReadyNodeReleaseServiceError> {
        let source = self
            .sources
            .inspect_scheduled_ready_node_release(
                &snapshot.graph_run_id,
                &snapshot.snapshot_sha256,
                ordinal,
                node_id,
            )
            .map_err(|error| map_source_store_error(&error))?;
        (source.progress_snapshot == *snapshot)
            .then_some(source)
            .ok_or(ScheduledReadyNodeReleaseServiceError::SourceChanged)
    }
}

fn build_control(
    source: &ScheduledReadyNodeReleaseSource,
    decision: &ScheduledGraphReconcileDecision,
) -> Result<GroupAgentScheduledReadyNodeDispatchReleaseControl, ScheduledReadyNodeReleaseServiceError>
{
    let base = super::snapshot::historical_base(&source.graph_run, &source.graph)
        .map_err(|_| ScheduledReadyNodeReleaseServiceError::CorruptSource)?;
    let request = &source.selected_provider_request;
    let request_json = String::from_utf8(request.provider_request_body.clone())
        .map_err(|_| ScheduledReadyNodeReleaseServiceError::CorruptSource)?;
    GroupAgentScheduledReadyNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: source.graph_run.run.clone(),
        journal_events: source.graph_run.events.clone(),
        control_snapshot: base.snapshot,
        schedule_record: source.schedule.record.clone(),
        schedule: source.schedule.schedule.clone(),
        progress_snapshot: source.progress_snapshot.clone(),
        reconcile_decision: decision.clone(),
        scheduled_contract_record: request.scheduled_contract.record.clone(),
        scheduled_contract: request.scheduled_contract.candidate.clone(),
        direct_predecessor_receipts: source.direct_predecessor_receipts.clone(),
        predecessor_content_artifact: source.predecessor_content_artifact.clone(),
        provider_request: request.record.clone(),
        provider_request_json: request_json,
        snapshot_sha256: String::new(),
    }
    .seal()
    .map_err(|_| ScheduledReadyNodeReleaseServiceError::CorruptSource)
}

fn ready_selection(
    decision: &ScheduledGraphReconcileDecision,
) -> Result<(usize, &str), ScheduledReadyNodeReleaseServiceError> {
    if decision.disposition != ScheduledGraphReconcileDisposition::Ready {
        return Err(ScheduledReadyNodeReleaseServiceError::NotReady);
    }
    decision
        .next_execution_ordinal
        .zip(decision.next_node_id.as_deref())
        .ok_or(ScheduledReadyNodeReleaseServiceError::InvalidReconcileDecision)
}

fn map_reconcile_error(
    error: ScheduledGraphReconcilePortError,
) -> ScheduledReadyNodeReleaseServiceError {
    match error {
        ScheduledGraphReconcilePortError::Unavailable => {
            ScheduledReadyNodeReleaseServiceError::ReconcileUnavailable
        }
        ScheduledGraphReconcilePortError::InvalidDecision => {
            ScheduledReadyNodeReleaseServiceError::InvalidReconcileDecision
        }
    }
}

fn map_authorization_error(
    error: ScheduledReadyNodeReleasePortError,
) -> ScheduledReadyNodeReleaseServiceError {
    match error {
        ScheduledReadyNodeReleasePortError::Unavailable => {
            ScheduledReadyNodeReleaseServiceError::AuthorizationUnavailable
        }
        ScheduledReadyNodeReleasePortError::InvalidAuthorization => {
            ScheduledReadyNodeReleaseServiceError::InvalidAuthorization
        }
    }
}

fn map_store_error(error: &HubStoreError) -> ScheduledReadyNodeReleaseServiceError {
    match error {
        HubStoreError::NotFound { .. } => ScheduledReadyNodeReleaseServiceError::NotFound,
        HubStoreError::Unavailable { .. } => {
            ScheduledReadyNodeReleaseServiceError::StorageUnavailable
        }
        HubStoreError::Conflict { .. } | HubStoreError::Corrupt { .. } => {
            ScheduledReadyNodeReleaseServiceError::CorruptSource
        }
    }
}

fn map_source_store_error(error: &HubStoreError) -> ScheduledReadyNodeReleaseServiceError {
    if matches!(error, HubStoreError::Conflict { .. }) {
        return ScheduledReadyNodeReleaseServiceError::SourceChanged;
    }
    map_store_error(error)
}

fn validate_identifier(value: &str) -> Result<(), ScheduledReadyNodeReleaseServiceError> {
    let valid = !value.trim().is_empty()
        && value.len() <= MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES
        && !value.chars().any(unsupported_character);
    valid
        .then_some(())
        .ok_or(ScheduledReadyNodeReleaseServiceError::InvalidInput)
}

fn unsupported_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}
