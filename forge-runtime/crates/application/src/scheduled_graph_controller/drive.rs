use crate::runtime_domain::{
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodeLifecycleStatus,
    ScheduledGraphControllerEventPayload as Payload, ScheduledGraphControllerJournal,
    ScheduledGraphControllerStopReason as StopReason, ScheduledGraphProgressNode,
    ScheduledGraphReconcileDisposition,
};
use crate::{AuthorizedScheduledReadyNodeRelease, ScheduledReadyNodeReleaseService};

use super::{
    ScheduledGraphControllerOutput, ScheduledGraphControllerService,
    ScheduledGraphControllerServiceError, recovery::Recovery,
};

enum DrivePrelude {
    Updated(Box<ScheduledGraphControllerJournal>),
    Observed(Box<crate::ScheduledGraphReconcileObservation>),
}

impl ScheduledGraphControllerService {
    pub(super) fn drive(
        &self,
        mut journal: ScheduledGraphControllerJournal,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        for _ in 0..crate::runtime_domain::MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS {
            if journal.is_terminal() {
                return Ok(ScheduledGraphControllerOutput::passive(journal));
            }
            let observation = match self.drive_prelude(&journal, observed_at_ms)? {
                DrivePrelude::Updated(updated) => {
                    journal = *updated;
                    continue;
                }
                DrivePrelude::Observed(observation) => observation,
            };
            match observation.decision.disposition {
                ScheduledGraphReconcileDisposition::Ready => {
                    let (updated, waiting) =
                        self.drive_ready(journal, &observation, observed_at_ms)?;
                    journal = updated;
                    if waiting {
                        return Ok(ScheduledGraphControllerOutput::passive(journal));
                    }
                }
                ScheduledGraphReconcileDisposition::Completed => {
                    journal = append_completed(self, &journal, &observation, observed_at_ms)?;
                }
                disposition => {
                    journal = append_disposition_stop(
                        self,
                        &journal,
                        &observation,
                        disposition,
                        observed_at_ms,
                    )?;
                }
            }
        }
        Err(ScheduledGraphControllerServiceError::CorruptEvidence)
    }

    fn drive_prelude(
        &self,
        journal: &ScheduledGraphControllerJournal,
        observed_at_ms: u64,
    ) -> Result<DrivePrelude, ScheduledGraphControllerServiceError> {
        let recovery = self.recover_dispatch(journal, None, observed_at_ms);
        if let Recovery::Updated(updated) =
            self.resolve_dispatch_recovery(journal, observed_at_ms, recovery)?
        {
            return Ok(DrivePrelude::Updated(updated));
        }
        if let Some(reason) = self.validate_reentry_schedule(&journal.header)? {
            let stopped = append_stop(self, journal, reason, None, None, observed_at_ms)?;
            return Ok(DrivePrelude::Updated(Box::new(stopped)));
        }
        let observation = self.observe_for_drive(&journal.header.graph_run_id)?;
        validate_observation_binding(journal, &observation)?;
        if let Recovery::Updated(updated) =
            self.recover_dispatch(journal, Some(&observation), observed_at_ms)?
        {
            return Ok(DrivePrelude::Updated(updated));
        }
        if cursor_advanced_without_controller_completion(journal, &observation)? {
            let stopped = append_stop(
                self,
                journal,
                StopReason::IncompatibleProgress,
                head_provider_request_id(journal),
                Some(&observation),
                observed_at_ms,
            )?;
            return Ok(DrivePrelude::Updated(Box::new(stopped)));
        }
        Ok(DrivePrelude::Observed(observation))
    }

    fn drive_ready(
        &self,
        journal: ScheduledGraphControllerJournal,
        observation: &crate::ScheduledGraphReconcileObservation,
        observed_at_ms: u64,
    ) -> Result<(ScheduledGraphControllerJournal, bool), ScheduledGraphControllerServiceError> {
        let node = selected_ready_node(observation)?;
        if matches!(journal.head().payload, Payload::MaterializePlanned { .. }) {
            return self
                .materialize_ready(&journal, observation, node, observed_at_ms)
                .map(|value| (value, false));
        }
        if matches!(journal.head().payload, Payload::PreparePlanned { .. }) {
            return self
                .prepare_ready(&journal, observation, node, observed_at_ms)
                .map(|value| (value, false));
        }
        if node.candidate_id.is_none() {
            return self
                .materialize_ready(&journal, observation, node, observed_at_ms)
                .map(|value| (value, false));
        }
        if node.provider_request_id.is_none() {
            return self
                .prepare_ready(&journal, observation, node, observed_at_ms)
                .map(|value| (value, false));
        }
        if budget_exhausted(&journal) {
            let stopped = append_stop(
                self,
                &journal,
                StopReason::BudgetExhausted,
                node.provider_request_id.clone(),
                Some(observation),
                observed_at_ms,
            )?;
            return Ok((stopped, true));
        }
        let authorized = self.authorize_ready(&journal.header.graph_run_id)?;
        validate_authorized_ready(&journal, node, observation, &authorized)?;
        if awaiting_matches(&journal, &authorized) {
            return Ok((journal, true));
        }
        let payload = awaiting_payload(&authorized);
        Ok((self.append_event(&journal, payload, observed_at_ms)?, true))
    }

    fn authorize_ready(
        &self,
        graph_run_id: &str,
    ) -> Result<AuthorizedScheduledReadyNodeRelease, ScheduledGraphControllerServiceError> {
        ScheduledReadyNodeReleaseService::new(
            self.hub.clone(),
            self.hub.clone(),
            self.reconcile.clone(),
            self.authorize.clone(),
        )
        .authorize(graph_run_id)
        .map_err(|_| ScheduledGraphControllerServiceError::AuthorizationFailed)
    }
}

fn selected_ready_node(
    observation: &crate::ScheduledGraphReconcileObservation,
) -> Result<&ScheduledGraphProgressNode, ScheduledGraphControllerServiceError> {
    let (ordinal, node_id) = ready_selection(observation)?;
    let node = observation
        .snapshot
        .nodes
        .get(ordinal)
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)?;
    (node.node_id == node_id)
        .then_some(node)
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn validate_observation_binding(
    journal: &ScheduledGraphControllerJournal,
    observation: &crate::ScheduledGraphReconcileObservation,
) -> Result<(), ScheduledGraphControllerServiceError> {
    let snapshot = &observation.snapshot;
    let header = &journal.header;
    if snapshot.graph_run_id != header.graph_run_id {
        return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
    }
    if snapshot.schedule_id != header.schedule_id
        || snapshot.schedule_sha256 != header.schedule_sha256
        || snapshot.node_count != header.node_count
        || snapshot.progress_protocol_version != header.progress_protocol_version
    {
        return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
    }
    Ok(())
}

fn cursor_advanced_without_controller_completion(
    journal: &ScheduledGraphControllerJournal,
    observation: &crate::ScheduledGraphReconcileObservation,
) -> Result<bool, ScheduledGraphControllerServiceError> {
    if let Payload::NodeCompleted {
        execution_ordinal, ..
    } = journal.head().payload
    {
        return cursor_after_node_completion(journal, observation, execution_ordinal);
    }
    if let Payload::Started {
        snapshot_sha256,
        decision_sha256,
    } = &journal.head().payload
    {
        return Ok(snapshot_sha256 != &observation.snapshot.snapshot_sha256
            || decision_sha256 != &observation.decision.decision_sha256);
    }
    let Some(current) = head_ordinal(journal) else {
        return Ok(false);
    };
    match observation.decision.disposition {
        ScheduledGraphReconcileDisposition::Ready => {
            let (selected, _) = ready_selection(observation)?;
            if selected < current {
                return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
            }
            Ok(selected > current)
        }
        ScheduledGraphReconcileDisposition::Completed => Ok(true),
        _ => Ok(false),
    }
}

fn cursor_after_node_completion(
    journal: &ScheduledGraphControllerJournal,
    observation: &crate::ScheduledGraphReconcileObservation,
    completed_ordinal: usize,
) -> Result<bool, ScheduledGraphControllerServiceError> {
    let expected = completed_ordinal
        .checked_add(1)
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)?;
    match observation.decision.disposition {
        ScheduledGraphReconcileDisposition::Ready => {
            let (selected, _) = ready_selection(observation)?;
            if selected < expected {
                return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
            }
            Ok(selected > expected)
        }
        ScheduledGraphReconcileDisposition::Completed => Ok(expected != journal.header.node_count),
        _ => Ok(false),
    }
}

fn head_ordinal(journal: &ScheduledGraphControllerJournal) -> Option<usize> {
    use Payload::{
        AwaitingFreshConsent, DispatchPlanned, MaterializeObserved, MaterializePlanned,
        PrepareObserved, PreparePlanned, RetryablePreclaimFailure,
    };
    match &journal.head().payload {
        MaterializePlanned {
            execution_ordinal, ..
        }
        | MaterializeObserved {
            execution_ordinal, ..
        }
        | PreparePlanned {
            execution_ordinal, ..
        }
        | PrepareObserved {
            execution_ordinal, ..
        }
        | AwaitingFreshConsent {
            execution_ordinal, ..
        }
        | DispatchPlanned {
            execution_ordinal, ..
        }
        | RetryablePreclaimFailure {
            execution_ordinal, ..
        } => Some(*execution_ordinal),
        Payload::Started { .. }
        | Payload::NodeCompleted { .. }
        | Payload::Stopped { .. }
        | Payload::Completed { .. } => None,
    }
}

fn head_provider_request_id(journal: &ScheduledGraphControllerJournal) -> Option<String> {
    match &journal.head().payload {
        Payload::PrepareObserved {
            provider_request_id,
            ..
        }
        | Payload::AwaitingFreshConsent {
            provider_request_id,
            ..
        }
        | Payload::DispatchPlanned {
            provider_request_id,
            ..
        }
        | Payload::RetryablePreclaimFailure {
            provider_request_id,
            ..
        } => Some(provider_request_id.clone()),
        _ => None,
    }
}

fn ready_selection(
    observation: &crate::ScheduledGraphReconcileObservation,
) -> Result<(usize, &str), ScheduledGraphControllerServiceError> {
    observation
        .decision
        .next_execution_ordinal
        .zip(observation.decision.next_node_id.as_deref())
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn append_completed(
    service: &ScheduledGraphControllerService,
    journal: &ScheduledGraphControllerJournal,
    observation: &crate::ScheduledGraphReconcileObservation,
    observed_at_ms: u64,
) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
    service.append_event(
        journal,
        Payload::Completed {
            snapshot_sha256: observation.snapshot.snapshot_sha256.clone(),
            decision_sha256: observation.decision.decision_sha256.clone(),
        },
        observed_at_ms,
    )
}

fn append_disposition_stop(
    service: &ScheduledGraphControllerService,
    journal: &ScheduledGraphControllerJournal,
    observation: &crate::ScheduledGraphReconcileObservation,
    disposition: ScheduledGraphReconcileDisposition,
    observed_at_ms: u64,
) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
    let node = first_incomplete(&observation.snapshot.nodes);
    let provider = node.and_then(|value| value.provider_request_id.clone());
    let reason = stop_reason(disposition, node)?;
    append_stop(
        service,
        journal,
        reason,
        provider,
        Some(observation),
        observed_at_ms,
    )
}

fn stop_reason(
    disposition: ScheduledGraphReconcileDisposition,
    node: Option<&ScheduledGraphProgressNode>,
) -> Result<StopReason, ScheduledGraphControllerServiceError> {
    match disposition {
        ScheduledGraphReconcileDisposition::ClaimedUnknown => Ok(StopReason::ClaimedUnknown),
        ScheduledGraphReconcileDisposition::ManualRecoveryRequired => {
            match node.and_then(|value| value.lifecycle_status) {
                Some(GroupAgentScheduledNodeLifecycleStatus::Quarantined) => {
                    Ok(StopReason::Quarantined)
                }
                Some(GroupAgentScheduledNodeLifecycleStatus::Adjudicated) => {
                    Ok(StopReason::Adjudicated)
                }
                _ => Err(ScheduledGraphControllerServiceError::CorruptEvidence),
            }
        }
        ScheduledGraphReconcileDisposition::Failed => Ok(StopReason::Failed),
        ScheduledGraphReconcileDisposition::FailedUncertain => Ok(StopReason::FailedUncertain),
        ScheduledGraphReconcileDisposition::IncompatibleProgress => {
            Ok(StopReason::IncompatibleProgress)
        }
        ScheduledGraphReconcileDisposition::Ready
        | ScheduledGraphReconcileDisposition::Completed => {
            Err(ScheduledGraphControllerServiceError::CorruptEvidence)
        }
    }
}

fn first_incomplete(nodes: &[ScheduledGraphProgressNode]) -> Option<&ScheduledGraphProgressNode> {
    nodes.iter().find(|node| {
        node.lifecycle_status != Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized)
            || node.terminal_outcome != Some(GroupAgentNodeTerminalOutcome::Completed)
    })
}

fn append_stop(
    service: &ScheduledGraphControllerService,
    journal: &ScheduledGraphControllerJournal,
    reason: StopReason,
    provider_request_id: Option<String>,
    observation: Option<&crate::ScheduledGraphReconcileObservation>,
    observed_at_ms: u64,
) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
    service.append_event(
        journal,
        stop_payload(reason, provider_request_id, observation),
        observed_at_ms,
    )
}

pub(super) fn stop_payload(
    reason: StopReason,
    provider_request_id: Option<String>,
    observation: Option<&crate::ScheduledGraphReconcileObservation>,
) -> Payload {
    Payload::Stopped {
        reason,
        provider_request_id,
        snapshot_sha256: observation.map(|value| value.snapshot.snapshot_sha256.clone()),
        decision_sha256: observation.map(|value| value.decision.decision_sha256.clone()),
    }
}

fn awaiting_payload(authorized: &AuthorizedScheduledReadyNodeRelease) -> Payload {
    let control = &authorized.release_control;
    let authorization = &authorized.authorization;
    Payload::AwaitingFreshConsent {
        execution_ordinal: authorization.execution_ordinal,
        node_id: authorization.node_id.clone(),
        provider_request_id: authorization.scheduled_provider_request_id.clone(),
        authorization_sha256: authorization.authorization_sha256.clone(),
        snapshot_sha256: control.progress_snapshot.snapshot_sha256.clone(),
        decision_sha256: control.reconcile_decision.decision_sha256.clone(),
        predecessor_content_included: control
            .scheduled_contract
            .request
            .predecessor_content_included,
    }
}

fn validate_authorized_ready(
    journal: &ScheduledGraphControllerJournal,
    node: &ScheduledGraphProgressNode,
    observation: &crate::ScheduledGraphReconcileObservation,
    authorized: &AuthorizedScheduledReadyNodeRelease,
) -> Result<(), ScheduledGraphControllerServiceError> {
    super::source_validation::validate_materialized_candidate(
        &journal.header,
        node,
        &authorized.release_control.scheduled_contract,
    )?;
    let authorization = &authorized.authorization;
    let valid = authorization.execution_ordinal == node.execution_ordinal
        && authorization.node_id == node.node_id
        && Some(&authorization.scheduled_provider_request_id) == node.provider_request_id.as_ref()
        && authorized.release_control.progress_snapshot == observation.snapshot
        && authorized.release_control.reconcile_decision == observation.decision;
    valid
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::StaleConsent)
}

fn awaiting_matches(
    journal: &ScheduledGraphControllerJournal,
    authorized: &AuthorizedScheduledReadyNodeRelease,
) -> bool {
    let Payload::AwaitingFreshConsent {
        provider_request_id,
        authorization_sha256,
        snapshot_sha256,
        decision_sha256,
        ..
    } = &journal.head().payload
    else {
        return false;
    };
    provider_request_id == &authorized.authorization.scheduled_provider_request_id
        && authorization_sha256 == &authorized.authorization.authorization_sha256
        && snapshot_sha256 == &authorized.release_control.progress_snapshot.snapshot_sha256
        && decision_sha256
            == &authorized
                .release_control
                .reconcile_decision
                .decision_sha256
}

fn budget_exhausted(journal: &ScheduledGraphControllerJournal) -> bool {
    let header = &journal.header;
    journal.effectful_steps_reserved() >= header.max_effectful_steps
        || journal
            .cost_usd_micros_reserved()
            .checked_add(header.execution_profile.max_cost_usd_micros)
            .is_none_or(|value| value > header.max_total_cost_usd_micros)
}
