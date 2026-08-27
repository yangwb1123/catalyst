use std::io::{self, Write};

use crate::runtime_application::{
    ScheduledGraphControllerOutput, ScheduledGraphControllerRecoveryPhase,
    ScheduledGraphControllerServiceError, ScheduledGraphControllerState,
};
use crate::runtime_domain::{
    ScheduledGraphControllerEvent, ScheduledGraphControllerRetryableFailure,
    ScheduledGraphControllerStopReason,
};
use serde::Serialize;

use super::ready_step_output::ScheduledReadyNodeStepCliOutput;

#[derive(Serialize)]
pub struct ScheduledGraphControllerCliOutput {
    #[serde(rename = "type")]
    kind: &'static str,
    v: u16,
    metadata_only: bool,
    controller_id: String,
    graph_run_id: String,
    schedule_id: String,
    schedule_sha256: String,
    head_sequence: usize,
    head_event_sha256: String,
    state: &'static str,
    recovery_phase: Option<&'static str>,
    awaiting_fresh_consent: Option<AwaitingConsentView>,
    stop_reason: Option<ScheduledGraphControllerStopReason>,
    stop_provider_request_id: Option<String>,
    effectful_steps_reserved: u16,
    max_effectful_steps: u16,
    cost_usd_micros_reserved: u64,
    max_total_cost_usd_micros: u64,
    retryable_failure: Option<ScheduledGraphControllerRetryableFailure>,
    post_invocation_error: Option<&'static str>,
    journal_current_observed: bool,
    automatic_retry_or_resend_performed: bool,
    core_trust_boundary: CoreTrustBoundaryView,
    #[serde(skip_serializing_if = "Option::is_none")]
    events: Option<Vec<ScheduledGraphControllerEvent>>,
    invocation: Option<ScheduledReadyNodeStepCliOutput>,
}

#[derive(Serialize)]
struct AwaitingConsentView {
    awaiting_event_sha256: String,
    execution_ordinal: usize,
    node_id: String,
    provider_request_id: String,
    authorization_sha256: String,
    snapshot_sha256: String,
    decision_sha256: String,
    predecessor_content_included: bool,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)] // Independent trust claims are stable CLI wire fields.
struct CoreTrustBoundaryView {
    same_user_code: bool,
    operator_trust_required: bool,
    binary_identity_validated: bool,
    reconcile_handshake_validated: bool,
    materialization_handshake_validated: bool,
    ready_release_handshake_validated: bool,
    terminal_protocol_handshake_validated: bool,
    empty_environment: bool,
    filesystem_isolation_enforced: bool,
    network_isolation_enforced: bool,
    effect_containment_enforced: bool,
    effect_attestation_present: bool,
}

#[derive(Clone, Copy)]
enum OutputMode {
    Invocation {
        predecessor_consent: bool,
        include_result: bool,
        core_validation_observed: bool,
    },
    Show,
}

impl OutputMode {
    const fn facts(self) -> (bool, bool, bool, bool) {
        match self {
            Self::Invocation {
                predecessor_consent,
                include_result,
                core_validation_observed,
            } => (
                predecessor_consent,
                include_result,
                core_validation_observed,
                false,
            ),
            Self::Show => (false, false, false, true),
        }
    }
}

impl ScheduledGraphControllerCliOutput {
    pub(super) fn new(
        output: ScheduledGraphControllerOutput,
        predecessor_consent: bool,
        include_result: bool,
        core_validation_observed: bool,
    ) -> Self {
        Self::build(
            output,
            OutputMode::Invocation {
                predecessor_consent,
                include_result,
                core_validation_observed,
            },
        )
    }

    pub(super) fn from_show(output: ScheduledGraphControllerOutput) -> Self {
        Self::build(output, OutputMode::Show)
    }

    fn build(mut output: ScheduledGraphControllerOutput, mode: OutputMode) -> Self {
        let (predecessor_consent, include_result, core_validation_observed, include_events) =
            mode.facts();
        let invocation = invocation_output(
            output.invocation.take(),
            predecessor_consent,
            include_result,
        );
        let events = include_events.then(|| output.journal.events.clone());
        let journal = output.journal;
        let head = journal.head();
        let (state, phase, awaiting, stop_reason, stop_provider) = project_state(output.state);
        Self {
            kind: "scheduled_graph_controller",
            v: journal.header.v,
            metadata_only: invocation
                .as_ref()
                .is_none_or(ScheduledReadyNodeStepCliOutput::metadata_only),
            controller_id: journal.header.controller_id.clone(),
            graph_run_id: journal.header.graph_run_id.clone(),
            schedule_id: journal.header.schedule_id.clone(),
            schedule_sha256: journal.header.schedule_sha256.clone(),
            head_sequence: head.sequence,
            head_event_sha256: head.event_sha256.clone(),
            state,
            recovery_phase: phase,
            awaiting_fresh_consent: awaiting,
            stop_reason,
            stop_provider_request_id: stop_provider,
            effectful_steps_reserved: journal.effectful_steps_reserved(),
            max_effectful_steps: journal.header.max_effectful_steps,
            cost_usd_micros_reserved: journal.cost_usd_micros_reserved(),
            max_total_cost_usd_micros: journal.header.max_total_cost_usd_micros,
            retryable_failure: output.retryable_failure,
            post_invocation_error: output
                .post_invocation_error
                .as_ref()
                .map(service_error_name),
            journal_current_observed: output.journal_current_observed,
            automatic_retry_or_resend_performed: false,
            core_trust_boundary: CoreTrustBoundaryView::observed(core_validation_observed),
            events,
            invocation,
        }
    }
}

fn invocation_output(
    value: Option<crate::runtime_application::ExecuteGroupAgentScheduledReadyNodeDispatchResult>,
    predecessor_consent: bool,
    include_result: bool,
) -> Option<ScheduledReadyNodeStepCliOutput> {
    value.map(|result| {
        ScheduledReadyNodeStepCliOutput::from_result(result, predecessor_consent, include_result)
    })
}

type StateProjection = (
    &'static str,
    Option<&'static str>,
    Option<AwaitingConsentView>,
    Option<ScheduledGraphControllerStopReason>,
    Option<String>,
);

fn project_state(state: ScheduledGraphControllerState) -> StateProjection {
    match state {
        ScheduledGraphControllerState::PassiveRecovery { phase } => (
            "passive_recovery",
            Some(phase_name(phase)),
            None,
            None,
            None,
        ),
        ScheduledGraphControllerState::AwaitingFreshConsent {
            awaiting_event_sha256,
            execution_ordinal,
            node_id,
            provider_request_id,
            authorization_sha256,
            snapshot_sha256,
            decision_sha256,
            predecessor_content_included,
        } => (
            "awaiting_fresh_consent",
            None,
            Some(AwaitingConsentView {
                awaiting_event_sha256,
                execution_ordinal,
                node_id,
                provider_request_id,
                authorization_sha256,
                snapshot_sha256,
                decision_sha256,
                predecessor_content_included,
            }),
            None,
            None,
        ),
        ScheduledGraphControllerState::Stopped {
            reason,
            provider_request_id,
        } => ("stopped", None, None, Some(reason), provider_request_id),
        ScheduledGraphControllerState::Completed => ("completed", None, None, None, None),
    }
}

const fn phase_name(value: ScheduledGraphControllerRecoveryPhase) -> &'static str {
    match value {
        ScheduledGraphControllerRecoveryPhase::Started => "started",
        ScheduledGraphControllerRecoveryPhase::Materialize => "materialize",
        ScheduledGraphControllerRecoveryPhase::Prepare => "prepare",
        ScheduledGraphControllerRecoveryPhase::Dispatch => "dispatch",
        ScheduledGraphControllerRecoveryPhase::Observe => "observe",
    }
}

impl CoreTrustBoundaryView {
    const fn observed(core_validation_observed: bool) -> Self {
        Self {
            same_user_code: true,
            operator_trust_required: true,
            binary_identity_validated: core_validation_observed,
            reconcile_handshake_validated: core_validation_observed,
            materialization_handshake_validated: core_validation_observed,
            ready_release_handshake_validated: core_validation_observed,
            terminal_protocol_handshake_validated: core_validation_observed,
            empty_environment: core_validation_observed,
            filesystem_isolation_enforced: false,
            network_isolation_enforced: false,
            effect_containment_enforced: false,
            effect_attestation_present: false,
        }
    }
}

pub fn write_output(
    output: &ScheduledGraphControllerCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer(&mut *writer, output)?;
        return writeln!(writer);
    }
    write_identity_and_budget(output, writer)?;
    write_consent(output, writer)?;
    write_status(output, writer)?;
    writeln!(
        writer,
        "  Core is operator-trusted same-user code; no effect containment or attestation"
    )?;
    if let Some(invocation) = &output.invocation {
        super::ready_step_output::write_output(invocation, false, writer)?;
    }
    Ok(())
}

fn write_identity_and_budget(
    output: &ScheduledGraphControllerCliOutput,
    writer: &mut impl Write,
) -> io::Result<()> {
    writeln!(writer, "scheduled Graph controller — {}", output.state)?;
    writeln!(writer, "  controller_id: {}", output.controller_id)?;
    writeln!(writer, "  graph_run_id: {}", output.graph_run_id)?;
    writeln!(writer, "  head_sequence: {}", output.head_sequence)?;
    writeln!(
        writer,
        "  effectful_steps_reserved: {}/{}",
        output.effectful_steps_reserved, output.max_effectful_steps
    )?;
    writeln!(
        writer,
        "  cost_usd_micros_reserved: {}/{}",
        output.cost_usd_micros_reserved, output.max_total_cost_usd_micros
    )
}

fn write_consent(
    output: &ScheduledGraphControllerCliOutput,
    writer: &mut impl Write,
) -> io::Result<()> {
    if let Some(awaiting) = &output.awaiting_fresh_consent {
        writeln!(
            writer,
            "  awaiting_event_sha256: {}",
            awaiting.awaiting_event_sha256
        )?;
        writeln!(
            writer,
            "  provider_request_id: {}",
            awaiting.provider_request_id
        )?;
        writeln!(
            writer,
            "  authorization_sha256: {}",
            awaiting.authorization_sha256
        )?;
        writeln!(
            writer,
            "  predecessor_content_included: {}",
            awaiting.predecessor_content_included
        )?;
    }
    Ok(())
}

fn write_status(
    output: &ScheduledGraphControllerCliOutput,
    writer: &mut impl Write,
) -> io::Result<()> {
    if let Some(reason) = output.stop_reason {
        writeln!(writer, "  stop_reason: {}", stop_reason_name(reason))?;
    }
    if let Some(provider_request_id) = &output.stop_provider_request_id {
        writeln!(writer, "  stop_provider_request_id: {provider_request_id}")?;
    }
    if let Some(reason) = output.retryable_failure {
        writeln!(
            writer,
            "  retryable_failure: {}",
            retryable_failure_name(reason)
        )?;
    }
    if let Some(error) = output.post_invocation_error {
        writeln!(writer, "  post_invocation_error: {error}")?;
    }
    writeln!(
        writer,
        "  journal_current_observed: {}",
        output.journal_current_observed
    )
}

const fn stop_reason_name(value: ScheduledGraphControllerStopReason) -> &'static str {
    match value {
        ScheduledGraphControllerStopReason::ClaimedUnknown => "claimed_unknown",
        ScheduledGraphControllerStopReason::Quarantined => "quarantined",
        ScheduledGraphControllerStopReason::Adjudicated => "adjudicated",
        ScheduledGraphControllerStopReason::Failed => "failed",
        ScheduledGraphControllerStopReason::FailedUncertain => "failed_uncertain",
        ScheduledGraphControllerStopReason::IncompatibleProgress => "incompatible_progress",
        ScheduledGraphControllerStopReason::IncompatibleSchedule => "incompatible_schedule",
        ScheduledGraphControllerStopReason::BudgetExhausted => "budget_exhausted",
    }
}

const fn retryable_failure_name(value: ScheduledGraphControllerRetryableFailure) -> &'static str {
    match value {
        ScheduledGraphControllerRetryableFailure::CredentialUnavailable => "credential_unavailable",
        ScheduledGraphControllerRetryableFailure::ProviderUnavailable => "provider_unavailable",
        ScheduledGraphControllerRetryableFailure::OwnerEvidenceUnavailable => {
            "owner_evidence_unavailable"
        }
    }
}

const fn service_error_name(value: &ScheduledGraphControllerServiceError) -> &'static str {
    match value {
        ScheduledGraphControllerServiceError::InvalidInput => "invalid_input",
        ScheduledGraphControllerServiceError::CorePinMismatch => "core_pin_mismatch",
        ScheduledGraphControllerServiceError::IncompatibleSchedule => "incompatible_schedule",
        ScheduledGraphControllerServiceError::StaleConsent => "stale_consent",
        ScheduledGraphControllerServiceError::ConsentRequired => "consent_required",
        ScheduledGraphControllerServiceError::PredecessorContentConsentRequired => {
            "predecessor_content_consent_required"
        }
        ScheduledGraphControllerServiceError::StoreUnavailable => "store_unavailable",
        ScheduledGraphControllerServiceError::ConcurrentUpdate => "concurrent_update",
        ScheduledGraphControllerServiceError::ReconcileFailed => "reconcile_failed",
        ScheduledGraphControllerServiceError::MaterializationFailed => "materialization_failed",
        ScheduledGraphControllerServiceError::AdmissionFailed => "admission_failed",
        ScheduledGraphControllerServiceError::PreparationFailed => "preparation_failed",
        ScheduledGraphControllerServiceError::AuthorizationFailed => "authorization_failed",
        ScheduledGraphControllerServiceError::PricingUnavailable => "pricing_unavailable",
        ScheduledGraphControllerServiceError::ExecutionFailed => "execution_failed",
        ScheduledGraphControllerServiceError::CorruptEvidence => "corrupt_evidence",
    }
}

#[cfg(test)]
#[path = "controller_output_tests.rs"]
mod tests;
