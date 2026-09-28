use std::{io, process::ExitCode};

use crate::{
    args::{DeviceCommand, DeviceInventoryCommand, DevicePlacementCommand},
    device_attempt_request_command, device_client_instance_resource_view_command,
    device_client_instance_scheduler_selection_preview_command, device_client_session_view_command,
    device_command, device_credential_candidate_command, device_execution_lease_checkpoint_command,
    device_heartbeat_persistence_command, device_identity_proof_command, device_inventory_command,
    device_inventory_observation_v2_command, device_inventory_persisted_observation_command,
    device_inventory_persistence_command, device_inventory_placement_batch_evaluation_command,
    device_inventory_placement_evaluation_command,
    device_inventory_placement_evaluation_v2_command, device_inventory_snapshot_command,
    device_inventory_status_command, device_pending_run_intent_command,
    device_resource_summary_command, device_run_attempt_lease_dispatch_preflight_command,
    device_run_execution_evidence_command, device_run_intent_command, device_run_observed_command,
    device_runner_attempt_boundary_command, device_runner_dispatch_plan_command,
    device_runner_execution_intent_command, device_runner_lease_fencing_command,
    device_runner_receipt_command, device_session_runner_receipt_command,
    device_session_runner_receipt_history_command,
};

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let execute: fn(&DeviceCommand, bool) -> ExitCode = match command {
        DeviceCommand::AttemptRequestPreview { .. } => run_attempt_request_preview,
        DeviceCommand::PendingRunIntentPreview { .. } => run_pending_run_intent_preview,
        DeviceCommand::Placement(DevicePlacementCommand::DryRun { .. }) => run_placement_dry_run,
        DeviceCommand::Placement(DevicePlacementCommand::RunIntentPreview { .. }) => {
            run_run_intent_preview
        }
        DeviceCommand::RunnerReceiptPreview { .. } => run_runner_receipt_preview,
        DeviceCommand::RunnerLeaseFencingPreview { .. } => run_runner_lease_fencing_preview,
        DeviceCommand::ExecutionLeaseCheckpointPreview { .. } => {
            run_execution_lease_checkpoint_preview
        }
        DeviceCommand::RunnerDispatchPlanPreview { .. } => run_runner_dispatch_plan_preview,
        DeviceCommand::RunnerAttemptBoundaryPreview { .. } => run_runner_attempt_boundary_preview,
        DeviceCommand::ClientSessionViewPreview { .. } => run_client_session_view_preview,
        DeviceCommand::ClientInstanceResourceViewPreview { .. } => {
            run_client_instance_resource_view_preview
        }
        DeviceCommand::ClientInstanceSchedulerSelectionPreview { .. } => {
            device_client_instance_scheduler_selection_preview_command::run
        }
        DeviceCommand::CredentialCandidatePreview { .. } => run_credential_candidate_preview,
        DeviceCommand::RunnerExecutionIntentPreview { .. } => run_runner_execution_intent_preview,
        DeviceCommand::SessionRunnerReceiptPreview { .. } => run_session_runner_receipt_preview,
        DeviceCommand::SessionRunnerReceiptHistoryPreview { .. } => {
            run_session_runner_receipt_history_preview
        }
        DeviceCommand::RunObservedPreview { .. } => run_run_observed_preview,
        DeviceCommand::RunExecutionEvidencePreview { .. } => run_run_execution_evidence_preview,
        DeviceCommand::RunAttemptLeaseDispatchPreflightPreview { .. } => {
            run_run_attempt_lease_dispatch_preflight_preview
        }
        DeviceCommand::HeartbeatPersistencePreview { .. } => run_heartbeat_persistence_preview,
        DeviceCommand::IdentityProofPreview { .. } => run_identity_proof_preview,
        DeviceCommand::Inventory(inventory) => inventory_executor(inventory),
    };
    execute(command, json)
}

fn inventory_executor(command: &DeviceInventoryCommand) -> fn(&DeviceCommand, bool) -> ExitCode {
    match command {
        DeviceInventoryCommand::Show { .. } => run_inventory_show,
        DeviceInventoryCommand::PersistencePreview { .. } => {
            device_inventory_persistence_command::run
        }
        DeviceInventoryCommand::PersistedObservation { .. } => {
            device_inventory_persisted_observation_command::run
        }
        DeviceInventoryCommand::PersistedObservationV2 { .. } => {
            device_inventory_observation_v2_command::run
        }
        DeviceInventoryCommand::Status { .. } => device_inventory_status_command::run,
        DeviceInventoryCommand::SnapshotCanonical { .. } => device_inventory_snapshot_command::run,
        DeviceInventoryCommand::ResourceSummary { .. } => device_resource_summary_command::run,
        DeviceInventoryCommand::SessionObservation { .. } => {
            device_resource_summary_command::session_observation::run
        }
        DeviceInventoryCommand::PlacementBatchEvaluation { .. } => {
            device_inventory_placement_batch_evaluation_command::run
        }
        DeviceInventoryCommand::PlacementEvaluation { .. } => {
            device_inventory_placement_evaluation_command::run
        }
        DeviceInventoryCommand::PlacementEvaluationV2 { .. } => {
            device_inventory_placement_evaluation_v2_command::run
        }
    }
}

fn run_pending_run_intent_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_pending_run_intent_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device pending Run-intent preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_pending_run_intent_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device pending Run-intent preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_attempt_request_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_attempt_request_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Attempt request preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_attempt_request_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device Attempt request preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_heartbeat_persistence_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_heartbeat_persistence_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device heartbeat persistence preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_heartbeat_persistence_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device heartbeat persistence preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_identity_proof_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_identity_proof_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device identity proof preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_identity_proof_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device identity proof preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_runner_receipt_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_runner_receipt_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Runner terminal receipt preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_runner_receipt_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device Runner terminal receipt preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_runner_lease_fencing_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_runner_lease_fencing_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Runner lease fencing preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_runner_lease_fencing_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device Runner lease fencing preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_execution_lease_checkpoint_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_execution_lease_checkpoint_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device execution lease checkpoint preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = device_execution_lease_checkpoint_command::write_output(
        &output,
        json,
        &mut io::stdout().lock(),
    ) {
        eprintln!("failed to write device execution lease checkpoint preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_runner_dispatch_plan_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_runner_dispatch_plan_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Runner dispatch-plan preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_runner_dispatch_plan_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device Runner dispatch-plan preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_client_session_view_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_client_session_view_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device client-instance/session-view preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_client_session_view_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device client-instance/session-view preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_client_instance_resource_view_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_client_instance_resource_view_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device client-instance/resource-view preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = device_client_instance_resource_view_command::write_output(
        &output,
        json,
        &mut io::stdout().lock(),
    ) {
        eprintln!("failed to write device client-instance/resource-view preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_credential_candidate_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_credential_candidate_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device credential-candidate preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_credential_candidate_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device credential-candidate preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_runner_execution_intent_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_runner_execution_intent_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Runner execution intent preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = device_runner_execution_intent_command::write_output(
        &output,
        json,
        &mut io::stdout().lock(),
    ) {
        eprintln!("failed to write device Runner execution intent preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_runner_attempt_boundary_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_runner_attempt_boundary_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Runner Attempt boundary preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = device_runner_attempt_boundary_command::write_output(
        &output,
        json,
        &mut io::stdout().lock(),
    ) {
        eprintln!("failed to write device Runner Attempt boundary preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_session_runner_receipt_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_session_runner_receipt_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device session Runner receipt preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_session_runner_receipt_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device session Runner receipt preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_session_runner_receipt_history_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_session_runner_receipt_history_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device session Runner receipt history preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = device_session_runner_receipt_history_command::write_output(
        &output,
        json,
        &mut io::stdout().lock(),
    ) {
        eprintln!("failed to write device session Runner receipt history output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_run_execution_evidence_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_run_execution_evidence_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Run execution-evidence preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_run_execution_evidence_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device Run execution-evidence preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_run_attempt_lease_dispatch_preflight_preview(
    command: &DeviceCommand,
    json: bool,
) -> ExitCode {
    let output = match device_run_attempt_lease_dispatch_preflight_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Run/Attempt/lease preflight failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = device_run_attempt_lease_dispatch_preflight_command::write_output(
        &output,
        json,
        &mut io::stdout().lock(),
    ) {
        eprintln!("failed to write device Run/Attempt/lease preflight output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_run_observed_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_run_observed_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Run observed preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_run_observed_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device Run observed preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_placement_dry_run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = device_command::write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device command output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_run_intent_preview(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_run_intent_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device Run-intent preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_run_intent_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device Run-intent preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_inventory_show(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match device_inventory_command::execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        device_inventory_command::write_output(&output, json, &mut io::stdout().lock())
    {
        eprintln!("failed to write device inventory output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
