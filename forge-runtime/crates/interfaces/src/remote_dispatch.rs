use std::{
    io::{self, Read},
    process::ExitCode,
};

use crate::{
    args::{Args, MAX_PROMPT_BYTES, RemoteCommand},
    remote_command,
    runtime_domain::{
        PENDING_WRITE_RECOVERY_EVALUATION_MODE, PENDING_WRITE_RECOVERY_SCHEMA_VERSION,
        PendingWriteMetadata, project_pending_write,
    },
};

pub(crate) async fn run(args: &Args, command: &RemoteCommand) -> ExitCode {
    if let RemoteCommand::SessionsImport {
        conversation_id,
        confirm,
    } = command
    {
        return run_import(args, conversation_id, confirm.as_deref()).await;
    }
    if matches!(command, RemoteCommand::Login) {
        return run_login(args).await;
    }
    if matches!(command, RemoteCommand::Tui) {
        return run_tui(args).await;
    }
    if matches!(command, RemoteCommand::CredentialStorageStatus) {
        return run_credential_storage_status();
    }
    let result = match resolve_stdin_prompt(command) {
        Ok(Some(content)) => {
            remote_command::execute_with_resolved_prompt(
                command,
                args.idempotency_key.as_deref(),
                &content,
            )
            .await
        }
        Ok(None) => remote_command::execute(command, args.idempotency_key.as_deref()).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(value) => match serde_json::to_string_pretty(&value) {
            Ok(output) => {
                println!("{output}");
                if matches!(command, RemoteCommand::PromptsAdd { .. }) {
                    eprintln!("{}", prompt_acknowledgement(&value));
                } else if matches!(command, RemoteCommand::PromptsReceipt { .. }) {
                    eprintln!("Prompt append receipt observed. No Run was started.");
                } else if matches!(command, RemoteCommand::PendingRunIntentSubmit { .. }) {
                    eprintln!("Pending Run-intent stored. No Run was started.");
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to encode remote API response: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("Remote Forge command failed: {error}");
            if let Some(recovery) = pending_write_recovery(command, args.idempotency_key.as_deref())
            {
                eprintln!("Pending write recovery: {recovery}");
            }
            ExitCode::FAILURE
        }
    }
}

fn prompt_acknowledgement(value: &serde_json::Value) -> &'static str {
    match value.get("replayed").and_then(serde_json::Value::as_bool) {
        Some(true) => "Prompt retry replayed the existing message. No Run was started.",
        Some(false) => "Prompt stored. No Run was started.",
        None => "Prompt receipt did not identify replay status. No Run was started.",
    }
}

fn run_credential_storage_status() -> ExitCode {
    match remote_command::credential_storage_status() {
        Ok(value) => match serde_json::to_string_pretty(&value) {
            Ok(output) => {
                println!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to encode credential storage status: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("Credential storage status failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn pending_write_recovery(command: &RemoteCommand, key: Option<&str>) -> Option<String> {
    let key = key.filter(|value| !value.is_empty())?;
    let metadata = match command {
        RemoteCommand::PromptsAdd {
            conversation_id,
            expected_version,
            ..
        } => PendingWriteMetadata {
            operation: "append_prompt".into(),
            conversation_id: Some(conversation_id.clone()),
            expected_version: Some(*expected_version),
            idempotency_key: key.into(),
            state: "unconfirmed".into(),
            attempted_at_ms: None,
            last_observed_at_ms: None,
        },
        RemoteCommand::PromptsReceipt {
            conversation_id,
            expected_version,
            ..
        } => PendingWriteMetadata {
            operation: "append_prompt_receipt".into(),
            conversation_id: Some(conversation_id.clone()),
            expected_version: Some(*expected_version),
            idempotency_key: key.into(),
            state: "unconfirmed".into(),
            attempted_at_ms: None,
            last_observed_at_ms: None,
        },
        RemoteCommand::PendingRunIntentSubmit {
            conversation_id,
            expected_version,
            ..
        } => PendingWriteMetadata {
            operation: "submit_pending_run_intent".into(),
            conversation_id: Some(conversation_id.clone()),
            expected_version: Some(*expected_version),
            idempotency_key: key.into(),
            state: "unconfirmed".into(),
            attempted_at_ms: None,
            last_observed_at_ms: None,
        },
        RemoteCommand::SessionsCreate { .. } => PendingWriteMetadata {
            operation: "create_conversation".into(),
            conversation_id: None,
            expected_version: None,
            idempotency_key: key.into(),
            state: "unconfirmed".into(),
            attempted_at_ms: None,
            last_observed_at_ms: None,
        },
        _ => return None,
    };
    project_pending_write(metadata.clone()).ok()?;
    let mut value = serde_json::to_value(metadata).ok()?;
    let object = value.as_object_mut()?;
    object.insert(
        "schema_version".into(),
        serde_json::Value::String(PENDING_WRITE_RECOVERY_SCHEMA_VERSION.into()),
    );
    object.insert(
        "evaluation_mode".into(),
        serde_json::Value::String(PENDING_WRITE_RECOVERY_EVALUATION_MODE.into()),
    );
    serde_json::to_string(&value).ok()
}

fn resolve_stdin_prompt(
    command: &RemoteCommand,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let content = match command {
        RemoteCommand::PromptsAdd { content, .. }
        | RemoteCommand::PromptsReceipt { content, .. }
        | RemoteCommand::PendingRunIntentSubmit { content, .. } => content,
        _ => return Ok(None),
    };
    if content != "-" {
        return Ok(None);
    }
    let mut input = io::stdin().lock();
    read_prompt_from_stdin(&mut input).map(Some)
}

fn read_prompt_from_stdin(input: &mut impl Read) -> Result<String, Box<dyn std::error::Error>> {
    let limit = u64::try_from(MAX_PROMPT_BYTES.saturating_add(1)).unwrap_or(u64::MAX);
    let mut bytes = Vec::with_capacity(MAX_PROMPT_BYTES.min(8 * 1024));
    input.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PROMPT_BYTES {
        return Err(format!("remote prompt may contain at most {MAX_PROMPT_BYTES} bytes").into());
    }
    let prompt = String::from_utf8(bytes).map_err(|_| "remote stdin prompt must be valid UTF-8")?;
    if prompt.trim().is_empty() {
        return Err("remote prompt must not be empty".into());
    }
    Ok(prompt)
}

#[cfg(test)]
#[path = "remote_dispatch_tests.rs"]
mod tests;

async fn run_login(args: &Args) -> ExitCode {
    if args.json {
        eprintln!("remote login cannot be combined with --json");
        return ExitCode::FAILURE;
    }
    match remote_command::run_login().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Remote Forge login failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run_tui(args: &Args) -> ExitCode {
    if args.json {
        eprintln!("remote tui cannot be combined with --json");
        return ExitCode::FAILURE;
    }
    match remote_command::run_tui(args.state_dir.as_deref()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Remote Forge TUI failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run_import(args: &Args, conversation_id: &str, confirm: Option<&str>) -> ExitCode {
    match remote_command::run_import(
        args.state_dir.as_deref(),
        conversation_id,
        confirm,
        args.json,
    )
    .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Remote session import failed: {error}");
            ExitCode::FAILURE
        }
    }
}
