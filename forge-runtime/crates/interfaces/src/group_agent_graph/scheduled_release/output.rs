use std::io::{self, Write};

use forge_runtime_application::VerifiedGroupAgentScheduledNodeDispatchAuthorization;
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeDispatchAuthorizationCliOutput {
    r#type: &'static str,
    v: u16,
    metadata_only: bool,
    authorization_validated_against_current_state: bool,
    all_current_effect_facts_false: bool,
    authorization_decisions: AuthorizationDecisionView,
    required_future_preconditions: RequiredFuturePreconditionsView,
    current_effect_facts: CurrentEffectFactsView,
    authorization: AuthorizationMetadataView,
    authorization_bytes_included: bool,
    private_release_control_included: bool,
    request_or_prompt_included: bool,
    endpoint_model_budget_lane_pricing_or_standalone_digest_included: bool,
}

#[derive(Serialize)]
struct AuthorizationDecisionView {
    lifecycle_contract_admission_authorized: bool,
    execution_authority_release_authorized: bool,
    dispatch_authority_release_authorized: bool,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct RequiredFuturePreconditionsView {
    fresh_off_machine_consent: bool,
    header_safe_credential_preflight: bool,
    exact_registered_destination_preflight: bool,
    exact_pricing_snapshot_within_max_cost: bool,
    global_project_lane_claim_until_terminal: bool,
    atomic_pristine_head_admission_release_lane_claim: bool,
    verified_intermediate_terminal_receipt_before_successor: bool,
    provider_health_check_forbidden: bool,
}

#[derive(Default, Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct CurrentEffectFactsView {
    lifecycle_contract_admitted: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    project_lane_claimed: bool,
    provider_request_sent: bool,
    progress_observed: bool,
    terminal_receipt_recorded: bool,
    successor_advance_authorized: bool,
    fresh_off_machine_consent_obtained: bool,
    credential_read: bool,
    credential_preflight_performed: bool,
    provider_constructed: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    result_produced_or_persisted: bool,
    database_mutated: bool,
    conversation_or_prompt_written: bool,
    memory_written: bool,
    writeback_performed: bool,
}

#[derive(Serialize)]
struct AuthorizationMetadataView {
    v: u16,
    authorization_id: String,
    graph_run_id: String,
    schedule_id: String,
    scheduled_contract_id: String,
    scheduled_provider_request_id: String,
    execution_ordinal: usize,
    node_id: String,
    attempt: u16,
}

impl GroupAgentScheduledNodeDispatchAuthorizationCliOutput {
    pub fn verified(value: VerifiedGroupAgentScheduledNodeDispatchAuthorization) -> Self {
        Self {
            r#type: "group_agent_scheduled_node_dispatch_authorization_verified",
            v: value.v,
            metadata_only: true,
            authorization_validated_against_current_state: true,
            all_current_effect_facts_false: true,
            authorization_decisions: AuthorizationDecisionView::authorized(),
            required_future_preconditions: RequiredFuturePreconditionsView::required(),
            current_effect_facts: CurrentEffectFactsView::default(),
            authorization: AuthorizationMetadataView::from(value),
            authorization_bytes_included: false,
            private_release_control_included: false,
            request_or_prompt_included: false,
            endpoint_model_budget_lane_pricing_or_standalone_digest_included: false,
        }
    }
}

impl AuthorizationDecisionView {
    fn authorized() -> Self {
        Self {
            lifecycle_contract_admission_authorized: true,
            execution_authority_release_authorized: true,
            dispatch_authority_release_authorized: true,
        }
    }
}

impl RequiredFuturePreconditionsView {
    fn required() -> Self {
        Self {
            fresh_off_machine_consent: true,
            header_safe_credential_preflight: true,
            exact_registered_destination_preflight: true,
            exact_pricing_snapshot_within_max_cost: true,
            global_project_lane_claim_until_terminal: true,
            atomic_pristine_head_admission_release_lane_claim: true,
            verified_intermediate_terminal_receipt_before_successor: true,
            provider_health_check_forbidden: true,
        }
    }
}

impl From<VerifiedGroupAgentScheduledNodeDispatchAuthorization> for AuthorizationMetadataView {
    fn from(value: VerifiedGroupAgentScheduledNodeDispatchAuthorization) -> Self {
        Self {
            v: value.v,
            authorization_id: value.authorization_id,
            graph_run_id: value.graph_run_id,
            schedule_id: value.schedule_id,
            scheduled_contract_id: value.scheduled_contract_id,
            scheduled_provider_request_id: value.scheduled_provider_request_id,
            execution_ordinal: value.execution_ordinal,
            node_id: value.node_id,
            attempt: value.attempt,
        }
    }
}

pub fn write_output(
    output: &GroupAgentScheduledNodeDispatchAuthorizationCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    write_human(output, writer)
}

fn write_human(
    output: &GroupAgentScheduledNodeDispatchAuthorizationCliOutput,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let value = &output.authorization;
    writeln!(
        writer,
        "scheduled authorization {}",
        terminal_text(&value.authorization_id)
    )?;
    writeln!(
        writer,
        "graph_run={} · request={} · node={} · attempt={}",
        terminal_text(&value.graph_run_id),
        terminal_text(&value.scheduled_provider_request_id),
        terminal_text(&value.node_id),
        value.attempt,
    )?;
    writeln!(
        writer,
        "authorization decisions: lifecycle admission=true, execution release=true, dispatch release=true"
    )?;
    writeln!(
        writer,
        "current effect facts: all false; no authority was released or effect performed"
    )?;
    writeln!(
        writer,
        "future execution still requires fresh consent, credential/destination/pricing preflight, atomic lane claim, and terminal evidence"
    )?;
    writeln!(
        writer,
        "no database/workspace/tool/network/provider/result/Conversation/Prompt/memory/writeback mutation occurred"
    )?;
    writeln!(
        writer,
        "authorization bytes, private control/request/Prompt, endpoint/model/budget/lane/pricing, and standalone digests remain hidden; content-addressed IDs remain visible"
    )
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
