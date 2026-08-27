use crate::runtime_domain::{
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodeAnyLifecycleInspection,
    GroupAgentScheduledNodeLifecycleStatus, HubStoreError,
    ScheduledGraphControllerEvent as ControllerEvent,
    ScheduledGraphControllerEventPayload as Payload, ScheduledGraphControllerHeader,
    ScheduledGraphControllerJournal, ScheduledGraphControllerStopReason as StopReason,
};

use super::{
    ScheduledGraphControllerService, ScheduledGraphControllerServiceError, drive::stop_payload,
};

pub(super) enum Recovery {
    Unchanged,
    Updated(Box<ScheduledGraphControllerJournal>),
}

impl ScheduledGraphControllerService {
    pub(super) fn recover_dispatch(
        &self,
        journal: &ScheduledGraphControllerJournal,
        observation: Option<&crate::ScheduledGraphReconcileObservation>,
        observed_at_ms: u64,
    ) -> Result<Recovery, ScheduledGraphControllerServiceError> {
        let Some(dispatch) = pending_dispatch(journal) else {
            return Ok(Recovery::Unchanged);
        };
        let source = dispatch_source(dispatch)?;
        let lifecycle = match self
            .hub
            .inspect_group_agent_scheduled_node_lifecycle_any_family(source.provider_request_id)
        {
            Ok(value) => value,
            Err(HubStoreError::NotFound { .. }) => return Ok(Recovery::Unchanged),
            Err(HubStoreError::Unavailable { .. }) => {
                return Err(ScheduledGraphControllerServiceError::StoreUnavailable);
            }
            Err(HubStoreError::Conflict { .. } | HubStoreError::Corrupt { .. }) => {
                return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
            }
        };
        lifecycle
            .validate()
            .map_err(|_| ScheduledGraphControllerServiceError::CorruptEvidence)?;
        validate_lifecycle_source(&lifecycle, &journal.header, &source)?;
        let payload = lifecycle_payload(
            &lifecycle,
            source.ordinal,
            source.node_id,
            source.provider_request_id,
            observation,
        )?;
        Ok(Recovery::Updated(Box::new(self.append_event(
            journal,
            payload,
            observed_at_ms,
        )?)))
    }

    pub(super) fn resolve_dispatch_recovery(
        &self,
        journal: &ScheduledGraphControllerJournal,
        observed_at_ms: u64,
        recovery: Result<Recovery, ScheduledGraphControllerServiceError>,
    ) -> Result<Recovery, ScheduledGraphControllerServiceError> {
        match recovery {
            Ok(Recovery::Updated(updated)) => Ok(Recovery::Updated(updated)),
            Ok(Recovery::Unchanged)
            | Err(ScheduledGraphControllerServiceError::StoreUnavailable) => {
                let Some(payload) = unclassified_dispatch_stop_payload(journal) else {
                    return Ok(Recovery::Unchanged);
                };
                self.append_event(journal, payload, observed_at_ms)
                    .map(|updated| Recovery::Updated(Box::new(updated)))
            }
            Err(error) => Err(error),
        }
    }
}

struct DispatchSource<'a> {
    ordinal: usize,
    node_id: &'a str,
    provider_request_id: &'a str,
    authorization_sha256: &'a str,
}

fn dispatch_source(
    event: &ControllerEvent,
) -> Result<DispatchSource<'_>, ScheduledGraphControllerServiceError> {
    let Payload::DispatchPlanned {
        execution_ordinal,
        node_id,
        provider_request_id,
        authorization_sha256,
        ..
    } = &event.payload
    else {
        return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
    };
    Ok(DispatchSource {
        ordinal: *execution_ordinal,
        node_id,
        provider_request_id,
        authorization_sha256,
    })
}

fn lifecycle_payload(
    lifecycle: &GroupAgentScheduledNodeAnyLifecycleInspection,
    ordinal: usize,
    node_id: &str,
    provider_request_id: &str,
    observation: Option<&crate::ScheduledGraphReconcileObservation>,
) -> Result<Payload, ScheduledGraphControllerServiceError> {
    match lifecycle.status() {
        GroupAgentScheduledNodeLifecycleStatus::Claimed => Ok(lifecycle_stop(
            StopReason::ClaimedUnknown,
            provider_request_id,
            observation,
        )),
        GroupAgentScheduledNodeLifecycleStatus::Quarantined => Ok(lifecycle_stop(
            StopReason::Quarantined,
            provider_request_id,
            observation,
        )),
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated => Ok(lifecycle_stop(
            StopReason::Adjudicated,
            provider_request_id,
            observation,
        )),
        GroupAgentScheduledNodeLifecycleStatus::Terminalized => terminal_payload(
            lifecycle,
            ordinal,
            node_id,
            provider_request_id,
            observation,
        ),
    }
}

fn terminal_payload(
    lifecycle: &GroupAgentScheduledNodeAnyLifecycleInspection,
    ordinal: usize,
    node_id: &str,
    provider_request_id: &str,
    observation: Option<&crate::ScheduledGraphReconcileObservation>,
) -> Result<Payload, ScheduledGraphControllerServiceError> {
    let receipt = lifecycle
        .terminal_receipt()
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)?;
    match receipt.node_outcome {
        GroupAgentNodeTerminalOutcome::Completed => Ok(Payload::NodeCompleted {
            execution_ordinal: ordinal,
            node_id: node_id.into(),
            provider_request_id: provider_request_id.into(),
            terminal_receipt_sha256: receipt.receipt_sha256.clone(),
        }),
        GroupAgentNodeTerminalOutcome::Failed => Ok(lifecycle_stop(
            StopReason::Failed,
            provider_request_id,
            observation,
        )),
        GroupAgentNodeTerminalOutcome::FailedUncertain => Ok(lifecycle_stop(
            StopReason::FailedUncertain,
            provider_request_id,
            observation,
        )),
    }
}

fn lifecycle_stop(
    reason: StopReason,
    provider_request_id: &str,
    observation: Option<&crate::ScheduledGraphReconcileObservation>,
) -> Payload {
    stop_payload(reason, Some(provider_request_id.into()), observation)
}

fn validate_lifecycle_source(
    lifecycle: &GroupAgentScheduledNodeAnyLifecycleInspection,
    header: &ScheduledGraphControllerHeader,
    source: &DispatchSource<'_>,
) -> Result<(), ScheduledGraphControllerServiceError> {
    let claim = lifecycle.claim();
    let common = claim.graph_run_id == header.graph_run_id
        && claim.provider_request_id == source.provider_request_id
        && claim.node_id == source.node_id;
    let family = match lifecycle {
        GroupAgentScheduledNodeAnyLifecycleInspection::Ready(value) => {
            claim.authorization_sha256 == source.authorization_sha256
                && value.authorization.execution_ordinal == source.ordinal
                && value.authorization.schedule_sha256 == header.schedule_sha256
        }
        GroupAgentScheduledNodeAnyLifecycleInspection::Legacy(value) => {
            value.provider_request.execution_ordinal == source.ordinal
                && value.provider_request.schedule_sha256 == header.schedule_sha256
        }
    };
    (common && family)
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn pending_dispatch(journal: &ScheduledGraphControllerJournal) -> Option<&ControllerEvent> {
    journal
        .events
        .iter()
        .rev()
        .find_map(|event| match event.payload {
            Payload::DispatchPlanned { .. } => Some(Some(event)),
            Payload::NodeCompleted { .. } => Some(None),
            _ => None,
        })?
}

pub(super) fn unclassified_dispatch_stop_payload(
    journal: &ScheduledGraphControllerJournal,
) -> Option<Payload> {
    let mut safely_released = false;
    for event in journal.events.iter().rev() {
        match &event.payload {
            Payload::NodeCompleted { .. } => return None,
            Payload::RetryablePreclaimFailure { .. } => safely_released = true,
            Payload::DispatchPlanned {
                provider_request_id,
                snapshot_sha256,
                decision_sha256,
                ..
            } => {
                return (!safely_released).then(|| Payload::Stopped {
                    reason: StopReason::ClaimedUnknown,
                    provider_request_id: Some(provider_request_id.clone()),
                    snapshot_sha256: Some(snapshot_sha256.clone()),
                    decision_sha256: Some(decision_sha256.clone()),
                });
            }
            _ => {}
        }
    }
    None
}
