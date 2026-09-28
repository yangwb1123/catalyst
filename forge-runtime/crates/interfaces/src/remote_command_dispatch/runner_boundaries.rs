use super::{
    Error, RemoteClient, RemoteCommand, RemoteError, Value,
    ensure_runner_admission_instance_projection,
};

enum Kind {
    DispatchAdmission,
    TransportAdmission,
    ExecutionBoundary,
    AttemptBoundary,
}

pub(super) async fn execute(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let kind = kind(command)?;
    let (input, instance_id, instance_view) = projection_options(command)?;
    match kind {
        Kind::DispatchAdmission => {
            dispatch_admission(client, input, instance_id, instance_view).await
        }
        Kind::TransportAdmission => {
            transport_admission(client, input, instance_id, instance_view).await
        }
        Kind::ExecutionBoundary => {
            execution_boundary(client, input, instance_id, instance_view).await
        }
        Kind::AttemptBoundary => attempt_boundary(client, input, instance_id, instance_view).await,
    }
}

fn kind(command: &RemoteCommand) -> Result<Kind, RemoteError> {
    match command {
        RemoteCommand::RunnerDispatchAdmissionPreview { .. } => Ok(Kind::DispatchAdmission),
        RemoteCommand::RunnerTransportAdmissionPreview { .. } => Ok(Kind::TransportAdmission),
        RemoteCommand::RunnerExecutionBoundaryPreview { .. } => Ok(Kind::ExecutionBoundary),
        RemoteCommand::RunnerAttemptBoundaryPreview { .. } => Ok(Kind::AttemptBoundary),
        _ => Err(RemoteError(
            "invalid Runner boundary preview command".into(),
        )),
    }
}

fn projection_options(
    command: &RemoteCommand,
) -> Result<(&str, Option<&str>, Option<&str>), RemoteError> {
    match command {
        RemoteCommand::RunnerDispatchAdmissionPreview {
            input,
            instance_id,
            instance_view,
        }
        | RemoteCommand::RunnerTransportAdmissionPreview {
            input,
            instance_id,
            instance_view,
        }
        | RemoteCommand::RunnerExecutionBoundaryPreview {
            input,
            instance_id,
            instance_view,
        }
        | RemoteCommand::RunnerAttemptBoundaryPreview {
            input,
            instance_id,
            instance_view,
        } => Ok((input, instance_id.as_deref(), instance_view.as_deref())),
        _ => Err(RemoteError(
            "invalid Runner boundary preview command".into(),
        )),
    }
}

async fn dispatch_admission(
    client: &RemoteClient,
    input: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::runner_dispatch_admission::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::runner_dispatch_admission::conversation_and_run(&request)?;
    let target_id = super::super::runner_dispatch_admission::target_id(&request)?;
    ensure_runner_admission_instance_projection(
        client,
        &conversation_id,
        target_id,
        instance_id,
        instance_view,
        "Runner dispatch admission",
    )
    .await?;
    let response = client
        .preview_runner_dispatch_admission(&conversation_id, &run_id, &request)
        .await?;
    super::super::runner_dispatch_admission::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}

async fn transport_admission(
    client: &RemoteClient,
    input: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::runner_transport_admission::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::runner_transport_admission::conversation_and_run(&request)?;
    let target_id = super::super::runner_transport_admission::target_id(&request)?;
    ensure_runner_admission_instance_projection(
        client,
        &conversation_id,
        target_id,
        instance_id,
        instance_view,
        "Runner transport admission",
    )
    .await?;
    let response = client
        .preview_runner_transport_admission(&conversation_id, &run_id, &request)
        .await?;
    super::super::runner_transport_admission::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}

async fn execution_boundary(
    client: &RemoteClient,
    input: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::runner_execution_boundary::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::runner_execution_boundary::conversation_and_run(&request)?;
    let target_id = super::super::runner_execution_boundary::target_id(&request)?;
    ensure_runner_admission_instance_projection(
        client,
        &conversation_id,
        target_id,
        instance_id,
        instance_view,
        "Runner execution boundary",
    )
    .await?;
    let response = client
        .preview_runner_execution_boundary(&conversation_id, &run_id, &request)
        .await?;
    super::super::runner_execution_boundary::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}

async fn attempt_boundary(
    client: &RemoteClient,
    input: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::runner_attempt_boundary::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::runner_attempt_boundary::conversation_and_run(&request)?;
    let target_id = super::super::runner_attempt_boundary::target_id(&request)?;
    ensure_runner_admission_instance_projection(
        client,
        &conversation_id,
        target_id,
        instance_id,
        instance_view,
        "Runner Attempt boundary",
    )
    .await?;
    let response = client
        .preview_runner_attempt_boundary(&conversation_id, &run_id, &request)
        .await?;
    super::super::runner_attempt_boundary::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}

#[cfg(test)]
#[path = "attempt_projection_tests.rs"]
mod tests;
