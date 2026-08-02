use std::io::{self, Write};

use forge_runtime_application::VerifiedGroupAgentNodeDispatchReadiness;
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentNodeDispatchReadinessCliOutput {
    r#type: &'static str,
    v: u16,
    metadata_only: bool,
    readiness_validated: bool,
    authorization_validated: bool,
    destination_registered: bool,
    pricing_snapshot_validated: bool,
    pricing_upper_bound_within_budget: bool,
    pricing_provenance: &'static str,
    vendor_attestation_present: bool,
    final_effectful_preflight_performed: bool,
    dispatch_authority_released: bool,
    consent_obtained: bool,
    credential_read: bool,
    credential_preflight_performed: bool,
    provider_constructed: bool,
    provider_used: bool,
    network_accessed: bool,
    project_lane_claimed: bool,
    execution_performed: bool,
    result_produced: bool,
    result_persisted: bool,
    graph_advanced: bool,
    database_written: bool,
    authorization_bytes_included: bool,
    pricing_bytes_included: bool,
    pricing_values_included: bool,
    destination_included: bool,
    model_included: bool,
    readiness: DispatchReadinessMetadataView,
}

#[derive(Serialize)]
struct DispatchReadinessMetadataView {
    authorization_id: String,
    graph_run_id: String,
    contract_id: String,
    dispatch_request_id: String,
    node_id: String,
    attempt: u16,
}

impl GroupAgentNodeDispatchReadinessCliOutput {
    pub fn verified(value: VerifiedGroupAgentNodeDispatchReadiness) -> Self {
        let authorization = value.authorization;
        Self {
            r#type: "group_agent_node_dispatch_readiness_verified",
            v: value.v,
            metadata_only: true,
            readiness_validated: true,
            authorization_validated: true,
            destination_registered: true,
            pricing_snapshot_validated: true,
            pricing_upper_bound_within_budget: true,
            pricing_provenance: "operator_asserted",
            vendor_attestation_present: false,
            final_effectful_preflight_performed: false,
            dispatch_authority_released: false,
            consent_obtained: false,
            credential_read: false,
            credential_preflight_performed: false,
            provider_constructed: false,
            provider_used: false,
            network_accessed: false,
            project_lane_claimed: false,
            execution_performed: false,
            result_produced: false,
            result_persisted: false,
            graph_advanced: false,
            database_written: false,
            authorization_bytes_included: false,
            pricing_bytes_included: false,
            pricing_values_included: false,
            destination_included: false,
            model_included: false,
            readiness: DispatchReadinessMetadataView {
                authorization_id: authorization.authorization_id,
                graph_run_id: authorization.graph_run_id,
                contract_id: authorization.contract_id,
                dispatch_request_id: authorization.dispatch_request_id,
                node_id: authorization.node_id,
                attempt: authorization.attempt,
            },
        }
    }
}

pub fn write_output(
    output: &GroupAgentNodeDispatchReadinessCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    let value = &output.readiness;
    writeln!(
        writer,
        "dispatch readiness {} · graph_run={} · request={} · node={} · attempt={}",
        terminal_text(&value.authorization_id),
        terminal_text(&value.graph_run_id),
        terminal_text(&value.dispatch_request_id),
        terminal_text(&value.node_id),
        value.attempt,
    )?;
    writeln!(
        writer,
        "current authorization, registered destination, and exact pricing artifact validated"
    )?;
    writeln!(
        writer,
        "artifact-declared cost upper bound fits the frozen budget; pricing is operator-asserted, not vendor-attested"
    )?;
    writeln!(
        writer,
        "readiness only: no final consent/credential preflight, provider, claim, network, execution, result, database write, or graph advance occurred"
    )?;
    writeln!(
        writer,
        "authorization, pricing values, destination, and model remain hidden"
    )
}

#[cfg(test)]
#[path = "dispatch_readiness_output_tests.rs"]
mod tests;
