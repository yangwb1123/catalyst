use std::error::Error;

use serde_json::Value;

use crate::args::RemoteCommand;

use super::{RemoteClient, RemoteError, required_idempotency_key};

pub(crate) async fn execute(
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    if matches!(command, RemoteCommand::Login) {
        return Err(RemoteError("use the remote login entry point".into()).into());
    }
    let client = RemoteClient::from_env().await?;
    match command {
        RemoteCommand::Login => unreachable!("remote login is handled before API client setup"),
        RemoteCommand::Tui => {
            Err(RemoteError("use the interactive remote TUI entry point".into()).into())
        }
        RemoteCommand::SessionsList { .. }
        | RemoteCommand::SessionsCreate { .. }
        | RemoteCommand::SessionsImport { .. } => {
            execute_session_command(&client, command, idempotency_key).await
        }
        RemoteCommand::RunsList { .. } | RemoteCommand::RunTimeline { .. } => {
            execute_run_command(&client, command).await
        }
        RemoteCommand::PromptsList { .. } | RemoteCommand::PromptsAdd { .. } => {
            execute_prompt_command(&client, command, idempotency_key).await
        }
        RemoteCommand::ChangesList { after_cursor } => Ok(serde_json::to_value(
            client.resumed_conversation_changes(*after_cursor).await?,
        )?),
    }
}

async fn execute_session_command(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::SessionsList {
            after_id,
            scope,
            all_pages,
        } => Ok(client
            .list_conversations_json(after_id.as_deref(), scope.as_ref(), *all_pages)
            .await?),
        RemoteCommand::SessionsCreate { title, scope } => {
            let key = required_idempotency_key(idempotency_key)?;
            Ok(client.create_conversation(title, scope, key).await?)
        }
        RemoteCommand::SessionsImport { .. } => {
            Err(RemoteError("use the local import preview entry point".into()).into())
        }
        _ => unreachable!("only session commands are routed here"),
    }
}

async fn execute_run_command(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::RunsList {
            conversation_id,
            limit,
            before_created_at_ms,
            before_run_id,
        } => Ok(client
            .list_runs(
                conversation_id,
                *limit,
                *before_created_at_ms,
                before_run_id.as_deref(),
            )
            .await?),
        RemoteCommand::RunTimeline {
            conversation_id,
            run_id,
            after_sequence,
            limit,
        } => Ok(client
            .run_timeline(conversation_id, run_id, *after_sequence, *limit)
            .await?),
        _ => unreachable!("only Run commands are routed here"),
    }
}

async fn execute_prompt_command(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::PromptsList {
            conversation_id,
            before_created_at_ms,
            before_prompt_id,
        } => {
            let before = before_created_at_ms.zip(before_prompt_id.clone()).map(
                |(created_at_ms, prompt_id)| crate::args::PromptPageCursor {
                    created_at_ms,
                    prompt_id,
                },
            );
            Ok(client
                .list_prompts(conversation_id, before.as_ref())
                .await?)
        }
        RemoteCommand::PromptsAdd {
            conversation_id,
            expected_version,
            content,
        } => {
            let key = required_idempotency_key(idempotency_key)?;
            Ok(client
                .append_prompt(conversation_id, *expected_version, content, key)
                .await?)
        }
        _ => unreachable!("only Prompt commands are routed here"),
    }
}
