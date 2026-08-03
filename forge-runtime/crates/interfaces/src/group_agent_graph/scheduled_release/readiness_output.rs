use std::io::{self, Write};

use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeDispatchReadinessCliOutput {
    r#type: &'static str,
    v: u16,
    metadata_only: bool,
    readiness_validated_against_current_state: bool,
    authorization_validated_against_current_state: bool,
    exact_registered_destination_validated: bool,
    exact_pricing_snapshot_validated: bool,
    pricing_upper_bound_within_frozen_budget: bool,
    pricing_provenance: &'static str,
    vendor_attestation_present: bool,
    authorization_decisions_are_future_only: bool,
    authorization_decisions: AuthorizationDecisionView,
    all_current_effect_facts_false: bool,
    #[serde(flatten)]
    effect_facts: ReadinessEffectFactsView,
    #[serde(flatten)]
    secrecy: ReadinessSecrecyView,
    readiness: ScheduledReadinessMetadataView,
}

#[derive(Serialize)]
struct AuthorizationDecisionView {
    lifecycle_contract_admission_authorized: bool,
    execution_authority_release_authorized: bool,
    dispatch_authority_release_authorized: bool,
}

#[derive(Default, Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct ReadinessEffectFactsView {
    final_effectful_preflight_performed: bool,
    lifecycle_contract_admitted: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    fresh_off_machine_consent_obtained: bool,
    credential_read: bool,
    credential_preflight_performed: bool,
    provider_constructed: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    project_lane_claimed: bool,
    provider_request_sent: bool,
    execution_performed: bool,
    progress_observed: bool,
    terminal_receipt_recorded: bool,
    successor_advance_authorized: bool,
    result_produced_or_persisted: bool,
    database_written: bool,
    conversation_prompt_or_memory_written: bool,
    writeback_performed: bool,
}

#[derive(Default, Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct ReadinessSecrecyView {
    authorization_bytes_included: bool,
    pricing_bytes_included: bool,
    pricing_values_included: bool,
    endpoint_model_budget_lane_or_standalone_digest_included: bool,
}

#[derive(Serialize)]
pub(crate) struct ScheduledReadinessMetadataView {
    pub(crate) authorization_id: String,
    pub(crate) graph_run_id: String,
    pub(crate) schedule_id: String,
    pub(crate) scheduled_contract_id: String,
    pub(crate) scheduled_provider_request_id: String,
    pub(crate) execution_ordinal: usize,
    pub(crate) node_id: String,
    pub(crate) attempt: u16,
}

impl GroupAgentScheduledNodeDispatchReadinessCliOutput {
    pub(crate) fn verified(v: u16, readiness: ScheduledReadinessMetadataView) -> Self {
        Self {
            r#type: "group_agent_scheduled_node_dispatch_readiness_verified",
            v,
            metadata_only: true,
            readiness_validated_against_current_state: true,
            authorization_validated_against_current_state: true,
            exact_registered_destination_validated: true,
            exact_pricing_snapshot_validated: true,
            pricing_upper_bound_within_frozen_budget: true,
            pricing_provenance: "operator_asserted",
            vendor_attestation_present: false,
            authorization_decisions_are_future_only: true,
            authorization_decisions: AuthorizationDecisionView::authorized(),
            all_current_effect_facts_false: true,
            effect_facts: ReadinessEffectFactsView::default(),
            secrecy: ReadinessSecrecyView::default(),
            readiness,
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

pub fn write_output(
    output: &GroupAgentScheduledNodeDispatchReadinessCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    write_human(output, writer)
}

fn write_human(
    output: &GroupAgentScheduledNodeDispatchReadinessCliOutput,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let value = &output.readiness;
    writeln!(
        writer,
        "scheduled dispatch readiness {} · graph_run={} · request={} · node={} · attempt={}",
        terminal_text(&value.authorization_id),
        terminal_text(&value.graph_run_id),
        terminal_text(&value.scheduled_provider_request_id),
        terminal_text(&value.node_id),
        value.attempt,
    )?;
    writeln!(
        writer,
        "current authorization, exact official registered destination, and exact pricing artifact validated"
    )?;
    writeln!(
        writer,
        "artifact-declared integer cost upper bound fits the frozen budget; pricing is operator-asserted, not vendor-attested"
    )?;
    writeln!(
        writer,
        "future-only authorization decisions: lifecycle admission=true, execution release=true, dispatch release=true"
    )?;
    writeln!(
        writer,
        "readiness only: no consent, credential/provider/network/workspace/tool, lifecycle admission, authority/lane claim, provider send, execution/progress/receipt/successor, result, database, Conversation/Prompt/memory, or writeback effect occurred"
    )?;
    writeln!(
        writer,
        "authorization/pricing bytes and values, endpoint, model, budget, lane, and standalone digests remain hidden"
    )
}

#[cfg(test)]
#[path = "readiness_output_tests.rs"]
mod tests;
