use super::{
    Error, RemoteClient, RemoteCommand, Value, ensure_runner_metadata_instance_projection,
};

pub(super) async fn execute(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::RunnerExecutionIntentPreview {
            input,
            instance_id,
            instance_view,
        } => {
            execution_intent(
                client,
                input,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await
        }
        RemoteCommand::RunAttemptLeaseDispatchPreflightPreview {
            input,
            instance_id,
            instance_view,
        } => {
            attempt_preflight(
                client,
                input,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await
        }
        RemoteCommand::RunnerDispatchPlanPreview {
            input,
            instance_id,
            instance_view,
        } => {
            dispatch_plan(
                client,
                input,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await
        }
        _ => unreachable!("Runner metadata command was classified before dispatch"),
    }
}

async fn execution_intent(
    client: &RemoteClient,
    input: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::runner_execution_intent::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::runner_execution_intent::conversation_and_run(&request)?;
    ensure_runner_metadata_instance_projection(
        client,
        &request,
        instance_id,
        instance_view,
        "Runner execution-intent",
    )
    .await?;
    let response = client
        .preview_runner_execution_intent(&conversation_id, &run_id, &request)
        .await?;
    super::super::runner_execution_intent::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}

async fn attempt_preflight(
    client: &RemoteClient,
    input: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::run_attempt_lease_dispatch_preflight::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::run_attempt_lease_dispatch_preflight::conversation_and_run(&request)?;
    ensure_runner_metadata_instance_projection(
        client,
        &request,
        instance_id,
        instance_view,
        "Run/Attempt/lease preflight",
    )
    .await?;
    let response = client
        .preview_run_attempt_lease_dispatch_preflight(&conversation_id, &run_id, &request)
        .await?;
    super::super::run_attempt_lease_dispatch_preflight::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}

async fn dispatch_plan(
    client: &RemoteClient,
    input: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::runner_dispatch_plan_preview::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::runner_dispatch_plan_preview::conversation_and_run(&request)?;
    ensure_runner_metadata_instance_projection(
        client,
        &request,
        instance_id,
        instance_view,
        "Runner dispatch-plan",
    )
    .await?;
    let dispatch_plan = super::super::runner_dispatch_plan_preview::dispatch_plan(&request)?;
    let response = client
        .preview_runner_dispatch_plan(&conversation_id, &run_id, &dispatch_plan)
        .await?;
    super::super::runner_dispatch_plan_preview::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}
