use std::collections::VecDeque;

use super::{Command, next_value, usage};

#[path = "device_args/client.rs"]
mod client;
#[path = "device_args/inventory.rs"]
mod inventory;
#[path = "device_args/placement.rs"]
mod placement;
#[path = "device_args/run.rs"]
mod run;
#[path = "device_args/runner.rs"]
mod runner;

use client::{
    parse_client_instance_resource_view_preview, parse_client_instance_scheduler_selection_preview,
    parse_client_session_view_preview, parse_credential_candidate_preview,
    parse_heartbeat_persistence_preview, parse_identity_proof_preview,
};
use inventory::parse_inventory;
use placement::parse_placement;
use run::{
    parse_attempt_request_preview, parse_pending_run_intent_preview,
    parse_run_attempt_lease_dispatch_preflight_preview, parse_run_execution_evidence_preview,
    parse_run_observed_preview, parse_session_runner_receipt_history_preview,
    parse_session_runner_receipt_preview,
};
use runner::{
    parse_execution_lease_checkpoint_preview, parse_runner_attempt_boundary_preview,
    parse_runner_dispatch_plan_preview, parse_runner_execution_intent_preview,
    parse_runner_lease_fencing_preview, parse_runner_receipt_preview,
};

#[derive(Debug, Eq, PartialEq)]
pub enum DeviceCommand {
    AttemptRequestPreview { input: String },
    PendingRunIntentPreview { input: String },
    Placement(DevicePlacementCommand),
    Inventory(DeviceInventoryCommand),
    RunnerReceiptPreview { input: String },
    RunnerLeaseFencingPreview { input: String },
    ExecutionLeaseCheckpointPreview { input: String },
    RunnerDispatchPlanPreview { input: String },
    RunnerAttemptBoundaryPreview { input: String },
    ClientSessionViewPreview { input: String },
    ClientInstanceResourceViewPreview { input: String },
    ClientInstanceSchedulerSelectionPreview { input: String },
    CredentialCandidatePreview { input: String },
    RunnerExecutionIntentPreview { input: String },
    SessionRunnerReceiptPreview { input: String },
    SessionRunnerReceiptHistoryPreview { input: String },
    RunObservedPreview { input: String },
    RunExecutionEvidencePreview { input: String },
    RunAttemptLeaseDispatchPreflightPreview { input: String },
    HeartbeatPersistencePreview { input: String },
    IdentityProofPreview { input: String },
}

#[derive(Debug, Eq, PartialEq)]
pub enum DevicePlacementCommand {
    DryRun {
        input: String,
    },
    RunIntentPreview {
        input: String,
        placement_input: String,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum DeviceInventoryCommand {
    Show { input: String },
    PersistencePreview { input: String },
    PersistedObservation { input: String },
    PersistedObservationV2 { input: String },
    PlacementEvaluation { input: String },
    PlacementEvaluationV2 { input: String },
    Status { input: String },
    SnapshotCanonical { input: String },
    ResourceSummary { input: String },
    SessionObservation { input: String },
    PlacementBatchEvaluation { input: String },
}

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("attempt-request-preview") => parse_attempt_request_preview(tokens),
        Some("pending-run-intent-preview") => parse_pending_run_intent_preview(tokens),
        Some("placement") => parse_placement(tokens),
        Some("inventory") => parse_inventory(tokens),
        Some("runner-receipt-preview") => parse_runner_receipt_preview(tokens),
        Some("runner-lease-fencing-preview") => parse_runner_lease_fencing_preview(tokens),
        Some("execution-lease-checkpoint-preview") => {
            parse_execution_lease_checkpoint_preview(tokens)
        }
        Some("runner-dispatch-plan-preview") => parse_runner_dispatch_plan_preview(tokens),
        Some("runner-attempt-boundary-preview") => parse_runner_attempt_boundary_preview(tokens),
        Some("client-session-view-preview") => parse_client_session_view_preview(tokens),
        Some("client-instance-resource-view-preview") => {
            parse_client_instance_resource_view_preview(tokens)
        }
        Some("client-instance-scheduler-selection-preview") => {
            parse_client_instance_scheduler_selection_preview(tokens)
        }
        Some("credential-candidate-preview") => parse_credential_candidate_preview(tokens),
        Some("runner-execution-intent-preview") => parse_runner_execution_intent_preview(tokens),
        Some("session-runner-receipt-preview") => parse_session_runner_receipt_preview(tokens),
        Some("session-runner-receipt-history-preview") => {
            parse_session_runner_receipt_history_preview(tokens)
        }
        Some("run-observed-preview") => parse_run_observed_preview(tokens),
        Some("run-execution-evidence-preview") => parse_run_execution_evidence_preview(tokens),
        Some("run-attempt-lease-dispatch-preflight-preview") => {
            parse_run_attempt_lease_dispatch_preflight_preview(tokens)
        }
        Some("heartbeat-persistence-preview") => parse_heartbeat_persistence_preview(tokens),
        Some("identity-proof-preview") => parse_identity_proof_preview(tokens),
        Some(value) => Err(format!("unknown device command '{value}'\n\n{}", usage())),
        None => Err(format!("device command is required\n\n{}", usage())),
    }
}
