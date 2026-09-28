use super::{
    Error, RemoteClient, RemoteCommand, RemoteError, Value, execution_previews, placement_previews,
    runner_boundaries, runner_metadata, scheduler_routes, session_evidence,
};

pub(super) async fn execute(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::Login => unreachable!("remote login is handled before API client setup"),
        RemoteCommand::Tui => {
            Err(RemoteError("use the interactive remote TUI entry point".into()).into())
        }
        RemoteCommand::CredentialStorageStatus => {
            Err(RemoteError("use the local credential storage status entry point".into()).into())
        }
        RemoteCommand::InventoryShow => Ok(client.read_device_inventory().await?),
        RemoteCommand::InventoryShowV2 => Ok(client.read_device_inventory_v2().await?),
        RemoteCommand::InventoryShowConverged => {
            Ok(client.read_converged_inventory_resource_view().await?)
        }
        RemoteCommand::LifecycleRegistryShow => Ok(client.read_lifecycle_registry().await?),
        RemoteCommand::ClientInstanceSessionView => {
            Ok(client.read_client_instance_session_view().await?)
        }
        RemoteCommand::ClientInstanceResourceView => {
            Ok(client.read_client_instance_resource_view().await?)
        }
        RemoteCommand::ClientInstancesShowConverged => {
            Ok(client.read_converged_client_instance_views().await?)
        }
        RemoteCommand::SessionRunnerReconciliationPreview { .. } => {
            unreachable!("local reconciliation projection is handled before remote client setup")
        }
        _ => execute_candidate(client, command, idempotency_key).await,
    }
}

async fn execute_candidate(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::PlacementPreview { .. }
        | RemoteCommand::PlacementRegistryPreview { .. }
        | RemoteCommand::CredentialCandidatePreview { .. } => {
            placement_previews::execute(client, command).await
        }
        RemoteCommand::SchedulerSelectionPreview { .. }
        | RemoteCommand::SchedulerSelectionLease { .. }
        | RemoteCommand::SchedulerSelectionLeaseRenew { .. }
        | RemoteCommand::SchedulerSelectionLeaseRelease { .. } => {
            scheduler_routes::execute(client, command, idempotency_key).await
        }
        RemoteCommand::RunnerDispatchAdmissionPreview { .. }
        | RemoteCommand::RunnerTransportAdmissionPreview { .. }
        | RemoteCommand::RunnerExecutionBoundaryPreview { .. }
        | RemoteCommand::RunnerAttemptBoundaryPreview { .. } => {
            runner_boundaries::execute(client, command).await
        }
        RemoteCommand::RunnerExecutionIntentPreview { .. }
        | RemoteCommand::RunAttemptLeaseDispatchPreflightPreview { .. }
        | RemoteCommand::RunnerDispatchPlanPreview { .. } => {
            runner_metadata::execute(client, command).await
        }
        RemoteCommand::SessionObservationPreview { .. }
        | RemoteCommand::SessionRunnerReceiptPreview { .. }
        | RemoteCommand::SessionRunnerReceiptHistoryPreview { .. }
        | RemoteCommand::SessionRunnerReconciliationRemotePreview { .. } => {
            session_evidence::execute(client, command).await
        }
        RemoteCommand::RunExecutionEvidencePreview { .. }
        | RemoteCommand::LocalRunnerPreview { .. }
        | RemoteCommand::ExecutionConsentPreview { .. }
        | RemoteCommand::ExecutionReconciliationPreview { .. } => {
            execution_previews::execute(client, command).await
        }
        _ => unreachable!("preview command was classified before dispatch"),
    }
}
