use std::{error::Error, io, path::PathBuf, sync::Arc};

use forge_runtime_application::{
    AuthorizedScheduledReadyNodeRelease, GroupAgentScheduledReadyNodeDispatchAuthorization,
    ScheduledReadyNodeReleaseService,
};
use forge_runtime_infrastructure::{PinnedScheduledReadyNodeReleaseBridge, SqliteHubStore};
use serde::Serialize;

use crate::{args::Args, group_context_output::terminal_text, state_path::hub_database_path};

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScheduledReadyNodeReleaseCliOutput {
    #[serde(rename = "type")]
    kind: &'static str,
    v: u16,
    metadata_only: bool,
    future_release_policy: bool,
    source_bundle_fresh_revalidated: bool,
    core_reconcile_rerun: bool,
    progress_observed: bool,
    private_source_sent_to_core: bool,
    effect_facts_scope: &'static str,
    sqlite_live_reader_coordination_possible: bool,
    core_trust_boundary: CoreTrustBoundaryView,
    runtime_effect_facts: RuntimeEffectFactsView,
    authorization: GroupAgentScheduledReadyNodeDispatchAuthorization,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct CoreTrustBoundaryView {
    same_user_code: bool,
    operator_trust_required: bool,
    binary_identity_validated: bool,
    reconcile_handshake_validated: bool,
    ready_release_handshake_validated: bool,
    empty_environment: bool,
    filesystem_isolation_enforced: bool,
    network_isolation_enforced: bool,
    effect_containment_enforced: bool,
    effect_attestation_present: bool,
}

#[derive(Default, Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct RuntimeEffectFactsView {
    logical_hub_mutated: bool,
    schema_migrated: bool,
    scheduled_candidate_materialized: bool,
    provider_request_prepared: bool,
    consent_collected_or_consumed: bool,
    predecessor_content_consent_collected_or_consumed: bool,
    credential_read: bool,
    provider_constructed: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    project_lane_claimed: bool,
    provider_request_sent: bool,
    lifecycle_contract_admitted: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    node_execution_performed: bool,
    terminal_receipt_recorded: bool,
    successor_authority_granted: bool,
    result_produced_or_persisted: bool,
    recovery_retry_or_resend_performed: bool,
    conversation_prompt_memory_or_writeback_written: bool,
}

impl ScheduledReadyNodeReleaseCliOutput {
    fn new(result: AuthorizedScheduledReadyNodeRelease) -> Self {
        Self {
            kind: "scheduled_ready_node_release_authorization",
            v: result.authorization.v,
            metadata_only: true,
            future_release_policy: true,
            source_bundle_fresh_revalidated: true,
            core_reconcile_rerun: true,
            progress_observed: true,
            private_source_sent_to_core: true,
            effect_facts_scope: "forge_runtime",
            sqlite_live_reader_coordination_possible: true,
            core_trust_boundary: CoreTrustBoundaryView::validated(),
            runtime_effect_facts: RuntimeEffectFactsView::default(),
            authorization: result.authorization,
        }
    }
}

impl CoreTrustBoundaryView {
    fn validated() -> Self {
        Self {
            same_user_code: true,
            operator_trust_required: true,
            binary_identity_validated: true,
            reconcile_handshake_validated: true,
            ready_release_handshake_validated: true,
            empty_environment: true,
            filesystem_isolation_enforced: false,
            network_isolation_enforced: false,
            effect_containment_enforced: false,
            effect_attestation_present: false,
        }
    }
}

pub fn execute(
    args: &Args,
    graph_run_id: &str,
    core_bin: &str,
    core_bin_sha256: &str,
) -> Result<ScheduledReadyNodeReleaseCliOutput, Box<dyn Error>> {
    let core = Arc::new(PinnedScheduledReadyNodeReleaseBridge::new(
        PathBuf::from(core_bin),
        core_bin_sha256.into(),
    )?);
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_live_read_only(
        database,
    )?);
    let service = ScheduledReadyNodeReleaseService::new(store.clone(), store, core.clone(), core);
    Ok(ScheduledReadyNodeReleaseCliOutput::new(
        service.authorize(graph_run_id)?,
    ))
}

pub fn write_output(
    output: &ScheduledReadyNodeReleaseCliOutput,
    json: bool,
    writer: &mut impl io::Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    let value = &output.authorization;
    writeln!(
        writer,
        "scheduled ready release {} — ordinal={} node={}",
        terminal_text(&value.graph_run_id),
        value.execution_ordinal,
        terminal_text(&value.node_id),
    )?;
    writeln!(
        writer,
        "authorization_sha256={}",
        value.authorization_sha256
    )?;
    writeln!(
        writer,
        "future max-one policy only — no current consent, lifecycle, execution, dispatch, lane, or send authority"
    )?;
    writeln!(
        writer,
        "the pinned Core received private source and is trusted same-user code; its byte pin and handshakes are not effect containment or attestation"
    )
}
