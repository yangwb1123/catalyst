use super::*;

pub(super) async fn execute(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let (input, instance_id, instance_view) = match command {
        RemoteCommand::SchedulerSelectionPreview {
            input,
            instance_id,
            instance_view,
        }
        | RemoteCommand::SchedulerSelectionLease {
            input,
            instance_id,
            instance_view,
        }
        | RemoteCommand::SchedulerSelectionLeaseRenew {
            input,
            instance_id,
            instance_view,
        }
        | RemoteCommand::SchedulerSelectionLeaseRelease {
            input,
            instance_id,
            instance_view,
        } => (
            input.as_str(),
            instance_id.as_deref(),
            instance_view.as_deref(),
        ),
        _ => unreachable!("scheduler command was classified before dispatch"),
    };
    if matches!(command, RemoteCommand::SchedulerSelectionPreview { .. }) {
        return execute_scheduler_selection_preview(client, input, instance_id, instance_view)
            .await;
    }
    let key = required_idempotency_key(idempotency_key)?;
    execute_lease(client, command, input, key, instance_id, instance_view).await
}

async fn execute_lease(
    client: &RemoteClient,
    command: &RemoteCommand,
    input: &str,
    key: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::SchedulerSelectionLease { .. } => {
            execute_scheduler_selection_lease(client, input, key, instance_id, instance_view).await
        }
        RemoteCommand::SchedulerSelectionLeaseRenew { .. } => {
            execute_scheduler_selection_lease_renewal(
                client,
                input,
                key,
                instance_id,
                instance_view,
            )
            .await
        }
        RemoteCommand::SchedulerSelectionLeaseRelease { .. } => {
            execute_scheduler_selection_lease_release(
                client,
                input,
                key,
                instance_id,
                instance_view,
            )
            .await
        }
        _ => unreachable!("scheduler lease command was classified before dispatch"),
    }
}
