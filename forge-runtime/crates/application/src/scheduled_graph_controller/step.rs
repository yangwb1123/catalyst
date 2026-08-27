use crate::runtime_domain::{
    MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS, ScheduledGraphControllerEventPayload as Payload,
    ScheduledGraphControllerJournal, ScheduledGraphControllerRetryableFailure as RetryableFailure,
    ScheduledGraphControllerStopReason,
};
use crate::{
    ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError as ExecutionError,
};

use super::{
    ScheduledGraphControllerOutput, ScheduledGraphControllerService,
    ScheduledGraphControllerServiceError, ScheduledGraphControllerState,
    StepScheduledGraphControllerInput, recovery::Recovery,
};

#[derive(Clone)]
struct ConsentAnchors {
    awaiting_event_sha256: String,
    execution_ordinal: usize,
    node_id: String,
    provider_request_id: String,
    authorization_sha256: String,
    snapshot_sha256: String,
    decision_sha256: String,
    predecessor_content_included: bool,
}

impl ScheduledGraphControllerService {
    /// Consumes one fresh exact-request consent and performs at most one
    /// ready-node executor invocation (and therefore at most one provider poll).
    ///
    /// # Errors
    ///
    /// Returns a redacted validation, stale-consent, storage, Core, or execution error.
    pub async fn step(
        &self,
        input: &StepScheduledGraphControllerInput,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        Self::preflight_step(input)?;
        let executor = self
            .executor
            .clone()
            .ok_or(ScheduledGraphControllerServiceError::ExecutionFailed)?;
        let journal = self.load(&input.graph_run_id)?;
        super::validate_reentry(&journal, &input.core_bin_sha256, input.observed_at_ms)?;
        let prepared = self.drive(journal, input.observed_at_ms)?;
        if prepared.journal.is_terminal() {
            return Ok(prepared);
        }
        let anchors = consent_anchors(&prepared.state)?;
        validate_step_consent(input, &anchors)?;
        let pricing_json = input
            .pricing_source
            .read_pricing_json()
            .map_err(|_| ScheduledGraphControllerServiceError::PricingUnavailable)?;
        let planned = append_dispatch_plan(
            self,
            &prepared.journal,
            &anchors,
            input.confirm_predecessor_content,
            input.observed_at_ms,
        )?;
        let execution = ExecuteGroupAgentScheduledReadyNodeDispatchInput {
            graph_run_id: input.graph_run_id.clone(),
            expected_provider_request_id: anchors.provider_request_id.clone(),
            expected_authorization_sha256: anchors.authorization_sha256.clone(),
            pricing_json,
            confirm_off_machine: input.confirm_off_machine,
            confirm_predecessor_content: input.confirm_predecessor_content,
            cancellation: input.cancellation.clone(),
        };
        match executor.execute(&execution).await {
            Ok(invocation) => {
                Ok(self.finish_successful_invocation(planned, invocation, input.observed_at_ms))
            }
            Err(error) => {
                self.finish_execution_error(planned, &anchors, &error, input.observed_at_ms)
            }
        }
    }

    fn finish_successful_invocation(
        &self,
        planned: ScheduledGraphControllerJournal,
        invocation: crate::ExecuteGroupAgentScheduledReadyNodeDispatchResult,
        observed_at_ms: u64,
    ) -> ScheduledGraphControllerOutput {
        let fallback = planned.clone();
        let mut output = match self.drive(planned, observed_at_ms) {
            Ok(output) => output,
            Err(error) => {
                let (journal, current) = match self.load(&fallback.header.graph_run_id) {
                    Ok(journal) => (journal, true),
                    Err(_) => (fallback, false),
                };
                let mut output = ScheduledGraphControllerOutput::passive(journal);
                output.post_invocation_error = Some(error);
                output.journal_current_observed = current;
                output
            }
        };
        output.invocation = Some(invocation);
        output
    }

    fn finish_execution_error(
        &self,
        journal: ScheduledGraphControllerJournal,
        anchors: &ConsentAnchors,
        error: &ExecutionError,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        if let Some(reason) = retryable_reason(error) {
            let journal = self.append_event(
                &journal,
                Payload::RetryablePreclaimFailure {
                    execution_ordinal: anchors.execution_ordinal,
                    node_id: anchors.node_id.clone(),
                    provider_request_id: anchors.provider_request_id.clone(),
                    reason,
                },
                observed_at_ms,
            )?;
            let mut output = self.drive(journal, observed_at_ms)?;
            output.retryable_failure = Some(reason);
            return Ok(output);
        }
        if matches!(
            error,
            ExecutionError::ClaimOutcomeUncertain | ExecutionError::PostClaimOutcomeUncertain
        ) {
            return self.finish_uncertain_execution(&journal, anchors, observed_at_ms);
        }
        let _recovered = self.drive(journal, observed_at_ms)?;
        Err(ScheduledGraphControllerServiceError::ExecutionFailed)
    }

    fn finish_uncertain_execution(
        &self,
        journal: &ScheduledGraphControllerJournal,
        anchors: &ConsentAnchors,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        let mut current = journal.clone();
        for _ in 0..MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS {
            if dispatch_completed_since(journal, &current, anchors) {
                return self.drive(current, observed_at_ms);
            }
            if current.is_terminal() {
                return uncertainty_terminal_output(current, anchors);
            }
            match self.recover_dispatch(&current, None, observed_at_ms) {
                Ok(Recovery::Updated(updated)) => return self.drive(*updated, observed_at_ms),
                Ok(Recovery::Unchanged)
                | Err(ScheduledGraphControllerServiceError::StoreUnavailable) => {}
                Err(ScheduledGraphControllerServiceError::ConcurrentUpdate) => {
                    current = self.reload_uncertainty_descendant(journal, &current)?;
                    continue;
                }
                Err(error) => return Err(error),
            }
            match self.append_claimed_unknown(&current, anchors, observed_at_ms) {
                Ok(output) => return Ok(output),
                Err(ScheduledGraphControllerServiceError::ConcurrentUpdate) => {
                    current = self.reload_uncertainty_descendant(journal, &current)?;
                }
                Err(error) => return Err(error),
            }
        }
        Err(ScheduledGraphControllerServiceError::ConcurrentUpdate)
    }

    fn append_claimed_unknown(
        &self,
        journal: &ScheduledGraphControllerJournal,
        anchors: &ConsentAnchors,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
        self.append_event(journal, claimed_unknown_payload(anchors), observed_at_ms)
            .map(ScheduledGraphControllerOutput::passive)
    }

    fn reload_uncertainty_descendant(
        &self,
        original: &ScheduledGraphControllerJournal,
        previous: &ScheduledGraphControllerJournal,
    ) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
        let current = self.load(&original.header.graph_run_id)?;
        let valid = current.validate().is_ok()
            && current.header == original.header
            && current.events.len() > original.events.len()
            && current.events.starts_with(&original.events)
            && current.events.len() > previous.events.len()
            && current.events.starts_with(&previous.events);
        valid
            .then_some(current)
            .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
    }
}

fn uncertainty_terminal_output(
    journal: ScheduledGraphControllerJournal,
    anchors: &ConsentAnchors,
) -> Result<ScheduledGraphControllerOutput, ScheduledGraphControllerServiceError> {
    let valid = matches!(
        &journal.head().payload,
        Payload::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown
                | ScheduledGraphControllerStopReason::Quarantined
                | ScheduledGraphControllerStopReason::Adjudicated
                | ScheduledGraphControllerStopReason::Failed
                | ScheduledGraphControllerStopReason::FailedUncertain,
            provider_request_id: Some(provider_request_id),
            ..
        } if provider_request_id == &anchors.provider_request_id
    );
    valid
        .then(|| ScheduledGraphControllerOutput::passive(journal))
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn claimed_unknown_payload(anchors: &ConsentAnchors) -> Payload {
    Payload::Stopped {
        reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
        provider_request_id: Some(anchors.provider_request_id.clone()),
        snapshot_sha256: Some(anchors.snapshot_sha256.clone()),
        decision_sha256: Some(anchors.decision_sha256.clone()),
    }
}

fn dispatch_completed_since(
    original: &ScheduledGraphControllerJournal,
    current: &ScheduledGraphControllerJournal,
    anchors: &ConsentAnchors,
) -> bool {
    current.events[original.events.len()..].iter().any(|event| {
        matches!(
            &event.payload,
            Payload::NodeCompleted {
                provider_request_id,
                ..
            } if provider_request_id == &anchors.provider_request_id
        )
    })
}

fn consent_anchors(
    state: &ScheduledGraphControllerState,
) -> Result<ConsentAnchors, ScheduledGraphControllerServiceError> {
    let ScheduledGraphControllerState::AwaitingFreshConsent {
        awaiting_event_sha256,
        execution_ordinal,
        node_id,
        provider_request_id,
        authorization_sha256,
        snapshot_sha256,
        decision_sha256,
        predecessor_content_included,
    } = state
    else {
        return Err(ScheduledGraphControllerServiceError::InvalidInput);
    };
    Ok(ConsentAnchors {
        awaiting_event_sha256: awaiting_event_sha256.clone(),
        execution_ordinal: *execution_ordinal,
        node_id: node_id.clone(),
        provider_request_id: provider_request_id.clone(),
        authorization_sha256: authorization_sha256.clone(),
        snapshot_sha256: snapshot_sha256.clone(),
        decision_sha256: decision_sha256.clone(),
        predecessor_content_included: *predecessor_content_included,
    })
}

fn validate_step_consent(
    input: &StepScheduledGraphControllerInput,
    anchors: &ConsentAnchors,
) -> Result<(), ScheduledGraphControllerServiceError> {
    if input.expected_awaiting_event_sha256 != anchors.awaiting_event_sha256
        || input.expected_provider_request_id != anchors.provider_request_id
        || input.expected_authorization_sha256 != anchors.authorization_sha256
    {
        return Err(ScheduledGraphControllerServiceError::StaleConsent);
    }
    if !input.confirm_off_machine {
        return Err(ScheduledGraphControllerServiceError::ConsentRequired);
    }
    if anchors.predecessor_content_included && !input.confirm_predecessor_content {
        return Err(ScheduledGraphControllerServiceError::PredecessorContentConsentRequired);
    }
    Ok(())
}

fn append_dispatch_plan(
    service: &ScheduledGraphControllerService,
    journal: &ScheduledGraphControllerJournal,
    anchors: &ConsentAnchors,
    predecessor_content_consent_observed: bool,
    observed_at_ms: u64,
) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
    let reservation = journal
        .effectful_steps_reserved()
        .checked_add(1)
        .ok_or(ScheduledGraphControllerServiceError::InvalidInput)?;
    let reserved_cost = journal.header.execution_profile.max_cost_usd_micros;
    let aggregate_cost = journal
        .cost_usd_micros_reserved()
        .checked_add(reserved_cost)
        .ok_or(ScheduledGraphControllerServiceError::InvalidInput)?;
    if reservation > journal.header.max_effectful_steps
        || aggregate_cost > journal.header.max_total_cost_usd_micros
    {
        return Err(ScheduledGraphControllerServiceError::InvalidInput);
    }
    service.append_event(
        journal,
        Payload::DispatchPlanned {
            execution_ordinal: anchors.execution_ordinal,
            node_id: anchors.node_id.clone(),
            provider_request_id: anchors.provider_request_id.clone(),
            authorization_sha256: anchors.authorization_sha256.clone(),
            snapshot_sha256: anchors.snapshot_sha256.clone(),
            decision_sha256: anchors.decision_sha256.clone(),
            effectful_step_reservation: reservation,
            reserved_cost_usd_micros: reserved_cost,
            off_machine_consent_observed: true,
            predecessor_content_consent_observed,
        },
        observed_at_ms,
    )
}

fn retryable_reason(error: &ExecutionError) -> Option<RetryableFailure> {
    match error {
        ExecutionError::CredentialUnavailable => Some(RetryableFailure::CredentialUnavailable),
        ExecutionError::ProviderUnavailable => Some(RetryableFailure::ProviderUnavailable),
        ExecutionError::OwnerEvidenceUnavailable => {
            Some(RetryableFailure::OwnerEvidenceUnavailable)
        }
        _ => None,
    }
}
