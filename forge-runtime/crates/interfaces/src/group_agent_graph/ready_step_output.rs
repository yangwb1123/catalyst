use std::io::{self, Write};

use crate::{
    runtime_application::{
        ExecuteGroupAgentScheduledReadyNodeDispatchResult,
        GroupAgentScheduledReadyNodeInvocationEffects, GroupAgentScheduledReadyNodeOwnerCleanup,
    },
    runtime_domain::{
        GroupAgentGraphRunStatus, GroupAgentNodeTerminalClassification,
        GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledReadyNodeLifecycleInspection,
    },
};
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScheduledReadyNodeStepCliOutput {
    #[serde(rename = "type")]
    kind: &'static str,
    v: u16,
    invocation_disposition: &'static str,
    metadata_only: bool,
    result_included: bool,
    graph_run_id: String,
    provider_request_id: String,
    dispatch_id: String,
    node_id: String,
    attempt: u16,
    graph_status: GroupAgentGraphRunStatus,
    lifecycle_status: GroupAgentScheduledNodeLifecycleStatus,
    classification: Option<GroupAgentNodeTerminalClassification>,
    provider_poll_started: bool,
    terminal_seen: bool,
    stream_eof_seen: bool,
    lane_active: bool,
    retry_authorized: bool,
    core_trust_boundary: CoreTrustBoundaryView,
    runtime_effect_facts: RuntimeEffectFactsView,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_text: Option<String>,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct CoreTrustBoundaryView {
    same_user_code: bool,
    operator_trust_required: bool,
    binary_identity_validated: bool,
    reconcile_handshake_validated: bool,
    ready_release_handshake_validated: bool,
    terminal_protocol_handshake_validated: bool,
    stored_terminal_receipt_validated: bool,
    empty_environment: bool,
    filesystem_isolation_enforced: bool,
    network_isolation_enforced: bool,
    effect_containment_enforced: bool,
    effect_attestation_present: bool,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct RuntimeEffectFactsView {
    effect_facts_scope: &'static str,
    schema_migration_observation: &'static str,
    preclaim_effects_observation: &'static str,
    fresh_off_machine_consent_supplied: bool,
    fresh_predecessor_content_consent_supplied: bool,
    pricing_source_read_this_invocation: bool,
    private_source_sent_to_core_this_invocation: Option<bool>,
    credential_read_this_invocation: Option<bool>,
    provider_constructed_this_invocation: Option<bool>,
    owner_sidecar_created_this_invocation: Option<bool>,
    owner_sidecar_cleanup_observation: &'static str,
    owner_sidecar_left_active_by_this_invocation: Option<bool>,
    catchable_signal_cancellation_armed: bool,
    project_lane_claimed_this_invocation: bool,
    provider_stream_polled_this_invocation: bool,
    remote_provider_request_observation: &'static str,
    logical_hub_mutated_this_invocation: bool,
    terminal_receipt_recorded_this_invocation: bool,
    result_produced_or_persisted_this_invocation: bool,
    automatic_recovery_retry_or_resend_performed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    conversation_prompt_memory_or_writeback_written: bool,
}

impl ScheduledReadyNodeStepCliOutput {
    pub(super) fn from_result(
        result: ExecuteGroupAgentScheduledReadyNodeDispatchResult,
        predecessor_consent: bool,
        include_result: bool,
    ) -> Self {
        let (inspection, effects, disposition) = match result {
            ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized {
                inspection,
                effects,
            } => (inspection, effects, "terminalized"),
            ExecuteGroupAgentScheduledReadyNodeDispatchResult::Quarantined {
                inspection,
                effects,
            } => (inspection, effects, "quarantined"),
            ExecuteGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed {
                inspection,
                effects,
            } => {
                let disposition = if effects.preclaim_effects_performed {
                    "claim_race_lost"
                } else {
                    "already_claimed"
                };
                (inspection, effects, disposition)
            }
        };
        Self::from_inspection(
            &inspection,
            predecessor_consent,
            include_result,
            effects,
            disposition,
        )
    }

    fn from_inspection(
        inspection: &GroupAgentScheduledReadyNodeLifecycleInspection,
        predecessor_consent: bool,
        include_result: bool,
        effects: GroupAgentScheduledReadyNodeInvocationEffects,
        disposition: &'static str,
    ) -> Self {
        let artifact = inspection.artifact.as_ref();
        let result_text = include_result
            .then(|| artifact.map(|value| value.output_text.clone()))
            .flatten();
        Self {
            kind: "group_agent_scheduled_ready_node_step",
            v: inspection.v,
            invocation_disposition: disposition,
            metadata_only: result_text.is_none(),
            result_included: result_text.is_some(),
            graph_run_id: inspection.graph_run.run.graph_run_id.clone(),
            provider_request_id: inspection.claim.provider_request_id.clone(),
            dispatch_id: inspection.claim.dispatch_id.clone(),
            node_id: inspection.claim.node_id.clone(),
            attempt: inspection.claim.attempt,
            graph_status: inspection.graph_run.run.status,
            lifecycle_status: inspection.status,
            classification: artifact.map(|value| value.classification),
            provider_poll_started: artifact.is_some_and(|value| value.provider_poll_started),
            terminal_seen: artifact.is_some_and(|value| value.terminal_seen),
            stream_eof_seen: artifact.is_some_and(|value| value.stream_eof_seen),
            lane_active: inspection.active_lane.is_some(),
            retry_authorized: artifact.is_some_and(|value| value.retry_authorized),
            core_trust_boundary: CoreTrustBoundaryView::validated(
                inspection.terminal_receipt.is_some(),
            ),
            runtime_effect_facts: RuntimeEffectFactsView::new(effects, predecessor_consent),
            result_text,
        }
    }
}

impl CoreTrustBoundaryView {
    const fn validated(terminal_receipt_present: bool) -> Self {
        Self {
            same_user_code: true,
            operator_trust_required: true,
            binary_identity_validated: true,
            reconcile_handshake_validated: true,
            ready_release_handshake_validated: true,
            terminal_protocol_handshake_validated: true,
            stored_terminal_receipt_validated: terminal_receipt_present,
            empty_environment: true,
            filesystem_isolation_enforced: false,
            network_isolation_enforced: false,
            effect_containment_enforced: false,
            effect_attestation_present: false,
        }
    }
}

impl RuntimeEffectFactsView {
    fn new(
        effects: GroupAgentScheduledReadyNodeInvocationEffects,
        predecessor_consent: bool,
    ) -> Self {
        let preclaim = effects.preclaim_effects_performed;
        Self {
            effect_facts_scope: "forge_runtime_this_invocation",
            schema_migration_observation: "not_observed",
            preclaim_effects_observation: if preclaim {
                "performed"
            } else {
                "not_performed"
            },
            fresh_off_machine_consent_supplied: true,
            fresh_predecessor_content_consent_supplied: predecessor_consent,
            pricing_source_read_this_invocation: true,
            private_source_sent_to_core_this_invocation: Some(preclaim),
            credential_read_this_invocation: Some(preclaim),
            provider_constructed_this_invocation: Some(preclaim),
            owner_sidecar_created_this_invocation: Some(preclaim),
            owner_sidecar_cleanup_observation: cleanup_observation(effects.owner_sidecar_cleanup),
            owner_sidecar_left_active_by_this_invocation: cleanup_presence(
                effects.owner_sidecar_cleanup,
            ),
            catchable_signal_cancellation_armed: true,
            project_lane_claimed_this_invocation: effects.project_lane_claimed,
            provider_stream_polled_this_invocation: effects.provider_stream_polled,
            remote_provider_request_observation: "not_attested",
            logical_hub_mutated_this_invocation: effects.logical_hub_mutated,
            terminal_receipt_recorded_this_invocation: effects.terminal_receipt_recorded,
            result_produced_or_persisted_this_invocation: effects.result_persisted,
            automatic_recovery_retry_or_resend_performed: false,
            workspace_accessed: false,
            tools_used: false,
            conversation_prompt_memory_or_writeback_written: false,
        }
    }
}

const fn cleanup_observation(value: GroupAgentScheduledReadyNodeOwnerCleanup) -> &'static str {
    match value {
        GroupAgentScheduledReadyNodeOwnerCleanup::NotApplicable => "not_applicable",
        GroupAgentScheduledReadyNodeOwnerCleanup::Succeeded => "succeeded",
        GroupAgentScheduledReadyNodeOwnerCleanup::Failed => "failed",
    }
}

const fn cleanup_presence(value: GroupAgentScheduledReadyNodeOwnerCleanup) -> Option<bool> {
    match value {
        GroupAgentScheduledReadyNodeOwnerCleanup::NotApplicable
        | GroupAgentScheduledReadyNodeOwnerCleanup::Succeeded => Some(false),
        GroupAgentScheduledReadyNodeOwnerCleanup::Failed => None,
    }
}

pub fn write_output(
    output: &ScheduledReadyNodeStepCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    write_summary(output, writer)?;
    if let Some(result) = &output.result_text {
        writeln!(writer, "result: {}", terminal_text(result))?;
    }
    Ok(())
}

fn write_summary(
    output: &ScheduledReadyNodeStepCliOutput,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "scheduled ready step {} — graph_run={} node={} request={} dispatch={}",
        lifecycle_label(output.lifecycle_status),
        terminal_text(&output.graph_run_id),
        terminal_text(&output.node_id),
        terminal_text(&output.provider_request_id),
        terminal_text(&output.dispatch_id),
    )?;
    writeln!(
        writer,
        "provider_poll_this_invocation={} remote_send_attested=false lifecycle_written_this_invocation={} owner_cleanup={} lane_active={} retry=false",
        output
            .runtime_effect_facts
            .provider_stream_polled_this_invocation,
        output
            .runtime_effect_facts
            .logical_hub_mutated_this_invocation,
        output
            .runtime_effect_facts
            .owner_sidecar_cleanup_observation,
        output.lane_active,
    )
}

const fn lifecycle_label(status: GroupAgentScheduledNodeLifecycleStatus) -> &'static str {
    match status {
        GroupAgentScheduledNodeLifecycleStatus::Claimed => "claimed",
        GroupAgentScheduledNodeLifecycleStatus::Terminalized => "terminalized",
        GroupAgentScheduledNodeLifecycleStatus::Quarantined => "quarantined",
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated => "adjudicated",
    }
}

#[cfg(test)]
#[path = "ready_step_output_tests.rs"]
mod tests;
