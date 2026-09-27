use super::*;

pub(super) async fn execute(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::SessionObservationPreview { input } => {
            session_observation(client, input).await
        }
        RemoteCommand::SessionRunnerReceiptPreview { input } => receipt(client, input).await,
        RemoteCommand::SessionRunnerReceiptHistoryPreview { input } => {
            receipt_history(client, input).await
        }
        RemoteCommand::SessionRunnerReconciliationRemotePreview { input } => {
            reconciliation(client, input).await
        }
        _ => unreachable!("session evidence command was classified before dispatch"),
    }
}

async fn session_observation(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::session_observation::read_request(input)?;
    let request_object = request
        .as_object()
        .ok_or_else(|| RemoteError("remote session observation input is invalid".into()))?;
    let conversation_id = request_object
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote session observation conversation is invalid".into()))?;
    let run_id = request_object
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote session observation Run is invalid".into()))?;
    let response = client
        .preview_session_device_observation(conversation_id, run_id, &request)
        .await?;
    super::super::session_observation::validate_response(&response, &request)?;
    Ok(response)
}

async fn receipt(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::session_runner_receipt::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::session_runner_receipt::conversation_and_run(&request)?;
    let response = client
        .preview_session_runner_receipt_observation(conversation_id, run_id, &request)
        .await?;
    super::super::session_runner_receipt::validate_response(
        &response,
        &request,
        conversation_id,
        run_id,
    )?;
    Ok(response)
}

async fn receipt_history(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::session_runner_receipt_history::read_request(input)?;
    let (conversation_id, run_id) =
        super::super::session_runner_receipt_history::conversation_and_run(&request)?;
    let response = client
        .preview_session_runner_receipt_history(conversation_id, run_id, &request)
        .await?;
    super::super::session_runner_receipt_history::validate_response(
        &response,
        &request,
        conversation_id,
        run_id,
    )?;
    Ok(response)
}

async fn reconciliation(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::session_runner_reconciliation::read_remote_request(input)?;
    let (conversation_id, run_id) =
        super::super::session_runner_reconciliation::remote_conversation_and_run(&request)?;
    // Canonicalize the caller-supplied history through the same
    // authenticated Core reduction used by the Console chain before
    // asking for the manual reconciliation projection. This keeps
    // every client on one owner/Conversation/Run-bound source image.
    let (canonical_history, response) = client
        .preview_session_runner_reconciliation_from_history(conversation_id, run_id, &request)
        .await?;
    super::super::session_runner_receipt_history::validate_response(
        &canonical_history,
        &request,
        conversation_id,
        run_id,
    )?;
    super::super::session_runner_reconciliation::validate_remote_response(
        &response,
        &canonical_history,
        conversation_id,
        run_id,
    )?;
    Ok(response)
}
