use super::{Error, RemoteClient, RemoteCommand, Value, execute_execution_consent_preview};

pub(super) async fn execute(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::ExecutionConsentPreview {
            conversation_id,
            instance_id,
            instance_view,
        } => {
            execute_execution_consent_preview(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await
        }
        RemoteCommand::RunExecutionEvidencePreview { input } => run_evidence(client, input).await,
        RemoteCommand::LocalRunnerPreview { input } => local_runner(client, input).await,
        RemoteCommand::ExecutionReconciliationPreview { input } => {
            reconciliation(client, input).await
        }
        _ => unreachable!("execution preview command was classified before dispatch"),
    }
}

async fn run_evidence(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::run_execution_evidence::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::run_execution_evidence::conversation_and_run(&request)?;
    let response = client
        .preview_run_execution_evidence(&conversation_id, &run_id, &request)
        .await?;
    super::super::run_execution_evidence::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}

async fn local_runner(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::local_runner_preview::read_request(input)?;
    let (conversation_id, intent_id) =
        super::super::local_runner_preview::conversation_and_intent(&request)?;
    let response = client
        .preview_local_runner_execution_readiness(conversation_id, intent_id, &request)
        .await?;
    super::super::local_runner_preview::validate_response(
        &response,
        &request,
        conversation_id,
        intent_id,
    )?;
    Ok(response)
}

async fn reconciliation(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::execution_reconciliation::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::execution_reconciliation::conversation_and_run(&request)?;
    let response = client
        .preview_execution_reconciliation(&conversation_id, &run_id, &request)
        .await?;
    super::super::execution_reconciliation::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    )?;
    Ok(response)
}
