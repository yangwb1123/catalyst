use std::collections::VecDeque;

use super::{Command, next_value, usage};

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

fn parse_pending_run_intent_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device pending-run-intent-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device pending-run-intent-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::PendingRunIntentPreview {
        input,
    }))
}

fn parse_attempt_request_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device attempt-request-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device attempt-request-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::AttemptRequestPreview {
        input,
    }))
}

fn parse_runner_receipt_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-receipt-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-receipt-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::RunnerReceiptPreview {
        input,
    }))
}

fn parse_runner_lease_fencing_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-lease-fencing-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-lease-fencing-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::RunnerLeaseFencingPreview {
        input,
    }))
}

fn parse_execution_lease_checkpoint_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device execution-lease-checkpoint-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device execution-lease-checkpoint-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::ExecutionLeaseCheckpointPreview { input },
    ))
}

fn parse_runner_dispatch_plan_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-dispatch-plan-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-dispatch-plan-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::RunnerDispatchPlanPreview {
        input,
    }))
}

fn parse_runner_attempt_boundary_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-attempt-boundary-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-attempt-boundary-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::RunnerAttemptBoundaryPreview { input },
    ))
}

fn parse_client_session_view_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device client-session-view-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device client-session-view-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::ClientSessionViewPreview {
        input,
    }))
}

fn parse_client_instance_resource_view_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device client-instance-resource-view-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device client-instance-resource-view-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::ClientInstanceResourceViewPreview { input },
    ))
}

fn parse_client_instance_scheduler_selection_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device client-instance-scheduler-selection-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device client-instance-scheduler-selection-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::ClientInstanceSchedulerSelectionPreview { input },
    ))
}

fn parse_credential_candidate_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device credential-candidate-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device credential-candidate-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::CredentialCandidatePreview {
        input,
    }))
}

fn parse_heartbeat_persistence_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device heartbeat-persistence-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device heartbeat-persistence-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::HeartbeatPersistencePreview { input },
    ))
}

fn parse_identity_proof_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device identity-proof-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device identity-proof-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::IdentityProofPreview {
        input,
    }))
}

fn parse_runner_execution_intent_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-execution-intent-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-execution-intent-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::RunnerExecutionIntentPreview { input },
    ))
}

fn parse_session_runner_receipt_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device session-runner-receipt-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device session-runner-receipt-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::SessionRunnerReceiptPreview { input },
    ))
}

fn parse_session_runner_receipt_history_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device session-runner-receipt-history-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device session-runner-receipt-history-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::SessionRunnerReceiptHistoryPreview { input },
    ))
}

fn parse_run_execution_evidence_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device run-execution-evidence-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device run-execution-evidence-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::RunExecutionEvidencePreview { input },
    ))
}

fn parse_run_attempt_lease_dispatch_preflight_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device run-attempt-lease-dispatch-preflight-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device run-attempt-lease-dispatch-preflight-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::RunAttemptLeaseDispatchPreflightPreview { input },
    ))
}

fn parse_run_observed_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device run-observed-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device run-observed-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::RunObservedPreview { input }))
}

fn parse_inventory(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("show") => parse_inventory_show(tokens),
        Some("persistence-preview") => parse_inventory_persistence_preview(tokens),
        Some("persisted-observation") => parse_inventory_persisted_observation_preview(tokens),
        Some("persisted-observation-v2") => {
            parse_inventory_persisted_observation_v2_preview(tokens)
        }
        Some("placement-evaluation") => parse_inventory_placement_evaluation(tokens),
        Some("status") => parse_inventory_status(tokens),
        Some("snapshot-canonical") => parse_inventory_snapshot_canonical(tokens),
        Some("resource-summary") => parse_inventory_resource_summary(tokens),
        Some("session-observation") => parse_inventory_session_observation(tokens),
        Some("placement-batch-evaluation") => parse_inventory_placement_batch_evaluation(tokens),
        Some("placement-evaluation-v2") => parse_inventory_placement_evaluation_v2(tokens),
        Some(value) => Err(format!(
            "unknown device inventory command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "device inventory command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_inventory_persistence_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory persistence-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory persistence-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PersistencePreview { input },
    )))
}

fn parse_inventory_persisted_observation_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory persisted-observation option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory persisted-observation requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PersistedObservation { input },
    )))
}

fn parse_inventory_persisted_observation_v2_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory persisted-observation-v2 option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory persisted-observation-v2 requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PersistedObservationV2 { input },
    )))
}

fn parse_inventory_placement_batch_evaluation(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory placement-batch-evaluation option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory placement-batch-evaluation requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PlacementBatchEvaluation { input },
    )))
}

fn parse_inventory_placement_evaluation_v2(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory placement-evaluation-v2 option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory placement-evaluation-v2 requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PlacementEvaluationV2 { input },
    )))
}

fn parse_inventory_placement_evaluation(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory placement-evaluation option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory placement-evaluation requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PlacementEvaluation { input },
    )))
}

fn parse_inventory_session_observation(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory session-observation option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory session-observation requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::SessionObservation { input },
    )))
}

fn parse_inventory_snapshot_canonical(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory snapshot-canonical option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory snapshot-canonical requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::SnapshotCanonical { input },
    )))
}

fn parse_inventory_status(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory status option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory status requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::Status { input },
    )))
}

fn parse_inventory_resource_summary(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory resource-summary option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory resource-summary requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::ResourceSummary { input },
    )))
}

fn parse_inventory_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory show option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory show requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::Show { input },
    )))
}

fn parse_placement(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("dry-run") => parse_dry_run(tokens),
        Some("run-intent-preview") => parse_run_intent_preview(tokens),
        Some(value) => Err(format!(
            "unknown device placement command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "device placement command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_run_intent_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    let mut placement_input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            "--placement-input" if placement_input.is_none() => {
                placement_input = Some(next_value(tokens, "--placement-input")?);
            }
            "--placement-input" => {
                return Err("--placement-input was specified more than once".into());
            }
            value => {
                return Err(format!(
                    "unknown device placement run-intent-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device placement run-intent-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    let placement_input = placement_input.ok_or_else(|| {
        format!(
            "device placement run-intent-preview requires --placement-input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() || placement_input.trim().is_empty() {
        return Err("input options require a non-empty FILE|- value".into());
    }
    if input == "-" && placement_input == "-" {
        return Err("--input and --placement-input cannot both read stdin".into());
    }
    Ok(Command::Device(DeviceCommand::Placement(
        DevicePlacementCommand::RunIntentPreview {
            input,
            placement_input,
        },
    )))
}

fn parse_dry_run(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device placement dry-run option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device placement dry-run requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Placement(
        DevicePlacementCommand::DryRun { input },
    )))
}
