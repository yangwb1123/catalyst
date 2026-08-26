use std::{error::Error, io, path::PathBuf, sync::Arc};

use forge_runtime_application::ScheduledGraphReconcileService;
use forge_runtime_domain::{ScheduledGraphReconcileDecision, ScheduledGraphReconcileDisposition};
use forge_runtime_infrastructure::{PinnedScheduledGraphReconcileBridge, SqliteHubStore};
use serde::Serialize;

use crate::{args::Args, group_context_output::terminal_text, state_path::hub_database_path};

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScheduledGraphReconcileCliOutput {
    #[serde(rename = "type")]
    kind: &'static str,
    v: u16,
    metadata_only: bool,
    progress_snapshot_validated: bool,
    core_decision_validated: bool,
    progress_observed: bool,
    effect_facts_scope: &'static str,
    sqlite_live_reader_coordination_possible: bool,
    core_trust_boundary: CoreTrustBoundaryView,
    runtime_effect_facts: RuntimeEffectFactsView,
    content_included: bool,
    decision: ScheduledGraphReconcileDecision,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct CoreTrustBoundaryView {
    same_user_code: bool,
    operator_trust_required: bool,
    binary_identity_validated: bool,
    protocol_handshake_validated: bool,
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
    scheduled_candidate_materialized: bool,
    provider_request_prepared: bool,
    consent_consumed: bool,
    credential_read: bool,
    provider_constructed: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    project_lane_claimed: bool,
    provider_request_sent: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    node_execution_performed: bool,
    recovery_performed: bool,
    retry_performed: bool,
    resend_performed: bool,
    terminal_receipt_recorded: bool,
    successor_authority_granted: bool,
    result_produced_or_persisted: bool,
    conversation_prompt_memory_or_writeback_written: bool,
}

impl ScheduledGraphReconcileCliOutput {
    fn new(decision: ScheduledGraphReconcileDecision) -> Self {
        Self {
            kind: "scheduled_graph_reconcile",
            v: decision.v,
            metadata_only: true,
            progress_snapshot_validated: true,
            core_decision_validated: true,
            progress_observed: true,
            effect_facts_scope: "forge_runtime",
            sqlite_live_reader_coordination_possible: true,
            core_trust_boundary: CoreTrustBoundaryView::validated(),
            runtime_effect_facts: RuntimeEffectFactsView::default(),
            content_included: false,
            decision,
        }
    }
}

impl CoreTrustBoundaryView {
    fn validated() -> Self {
        Self {
            same_user_code: true,
            operator_trust_required: true,
            binary_identity_validated: true,
            protocol_handshake_validated: true,
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
) -> Result<ScheduledGraphReconcileCliOutput, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_live_read_only(
        database,
    )?);
    let core = Arc::new(PinnedScheduledGraphReconcileBridge::new(
        PathBuf::from(core_bin),
        core_bin_sha256.into(),
    )?);
    let service = ScheduledGraphReconcileService::new(store, core);
    Ok(ScheduledGraphReconcileCliOutput::new(
        service.reconcile(graph_run_id)?,
    ))
}

pub fn write_output(
    output: &ScheduledGraphReconcileCliOutput,
    json: bool,
    writer: &mut impl io::Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    let decision = &output.decision;
    writeln!(
        writer,
        "scheduled graph reconcile {} — {}",
        terminal_text(&decision.graph_run_id),
        disposition_label(decision.disposition),
    )?;
    if let (Some(ordinal), Some(node_id)) = (
        decision.next_execution_ordinal,
        decision.next_node_id.as_deref(),
    ) {
        writeln!(
            writer,
            "next ordinal={ordinal} node={}",
            terminal_text(node_id)
        )?;
    }
    writeln!(writer, "snapshot_sha256={}", decision.snapshot_sha256)?;
    writeln!(
        writer,
        "observation only — Forge Runtime granted no execution, dispatch, recovery, retry, or successor authority"
    )?;
    writeln!(
        writer,
        "the operator-pinned Core is trusted same-user code; its byte pin is not effect containment or attestation"
    )
}

fn disposition_label(disposition: ScheduledGraphReconcileDisposition) -> &'static str {
    match disposition {
        ScheduledGraphReconcileDisposition::Ready => "ready",
        ScheduledGraphReconcileDisposition::ClaimedUnknown => "claimed_unknown",
        ScheduledGraphReconcileDisposition::ManualRecoveryRequired => "manual_recovery_required",
        ScheduledGraphReconcileDisposition::Failed => "failed",
        ScheduledGraphReconcileDisposition::FailedUncertain => "failed_uncertain",
        ScheduledGraphReconcileDisposition::Completed => "completed",
        ScheduledGraphReconcileDisposition::IncompatibleProgress => "incompatible_progress",
    }
}
