use std::io::{self, Write};

use forge_runtime_application::VerifiedGroupAgentNodeDispatchAuthorization;
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentNodeDispatchAuthorizationCliOutput {
    r#type: &'static str,
    v: u16,
    metadata_only: bool,
    authorization_validated: bool,
    dispatch_authority_release_authorized: bool,
    dispatch_authority_released: bool,
    consent_obtained: bool,
    fresh_off_machine_consent_obtained: bool,
    credential_read: bool,
    credential_preflight_performed: bool,
    execution_performed: bool,
    model_used: bool,
    provider_constructed: bool,
    provider_used: bool,
    network_invoked: bool,
    network_accessed: bool,
    project_lane_claimed: bool,
    graph_advanced: bool,
    workspace_accessed: bool,
    tools_used: bool,
    result_produced: bool,
    result_persisted: bool,
    conversation_or_prompt_written: bool,
    memory_written: bool,
    writeback_performed: bool,
    authorization_bytes_included: bool,
    release_control_included: bool,
    request_body_included: bool,
    destination_included: bool,
    model_included: bool,
    pricing_included: bool,
    authorization: DispatchAuthorizationMetadataView,
}

#[derive(Serialize)]
struct DispatchAuthorizationMetadataView {
    v: u16,
    authorization_id: String,
    authorization_sha256: String,
    graph_run_id: String,
    contract_id: String,
    dispatch_request_id: String,
    node_id: String,
    attempt: u16,
}

impl GroupAgentNodeDispatchAuthorizationCliOutput {
    pub fn verified(verified: VerifiedGroupAgentNodeDispatchAuthorization) -> Self {
        Self {
            r#type: "group_agent_node_dispatch_authorization_verified",
            v: verified.v,
            metadata_only: true,
            authorization_validated: true,
            dispatch_authority_release_authorized: true,
            dispatch_authority_released: false,
            consent_obtained: false,
            fresh_off_machine_consent_obtained: false,
            credential_read: false,
            credential_preflight_performed: false,
            execution_performed: false,
            model_used: false,
            provider_constructed: false,
            provider_used: false,
            network_invoked: false,
            network_accessed: false,
            project_lane_claimed: false,
            graph_advanced: false,
            workspace_accessed: false,
            tools_used: false,
            result_produced: false,
            result_persisted: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            authorization_bytes_included: false,
            release_control_included: false,
            request_body_included: false,
            destination_included: false,
            model_included: false,
            pricing_included: false,
            authorization: DispatchAuthorizationMetadataView::from(verified),
        }
    }
}

impl From<VerifiedGroupAgentNodeDispatchAuthorization> for DispatchAuthorizationMetadataView {
    fn from(verified: VerifiedGroupAgentNodeDispatchAuthorization) -> Self {
        Self {
            v: verified.v,
            authorization_id: verified.authorization_id,
            authorization_sha256: verified.authorization_sha256,
            graph_run_id: verified.graph_run_id,
            contract_id: verified.contract_id,
            dispatch_request_id: verified.dispatch_request_id,
            node_id: verified.node_id,
            attempt: verified.attempt,
        }
    }
}

pub fn write_output(
    output: &GroupAgentNodeDispatchAuthorizationCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    let authorization = &output.authorization;
    writeln!(
        writer,
        "dispatch authorization {} · graph_run={} · request={} · node={} · attempt={}",
        terminal_text(&authorization.authorization_id),
        terminal_text(&authorization.graph_run_id),
        terminal_text(&authorization.dispatch_request_id),
        terminal_text(&authorization.node_id),
        authorization.attempt,
    )?;
    writeln!(
        writer,
        "canonical authorization validated; artifact authorizes a future authority release"
    )?;
    writeln!(
        writer,
        "dispatch authority not released; fresh off-machine consent absent; credential not read"
    )?;
    writeln!(
        writer,
        "no credential preflight, project-lane claim, provider construction/model, or network effect occurred"
    )?;
    writeln!(
        writer,
        "no execution/graph advance/workspace/tools/result or writeback effect occurred"
    )?;
    writeln!(
        writer,
        "no Conversation/Prompt/memory write operation occurred"
    )?;
    writeln!(
        writer,
        "authorization bytes, release control, request body, destination, model, and pricing remain hidden"
    )
}

#[cfg(test)]
#[path = "dispatch_authorization_output_tests.rs"]
mod tests;
