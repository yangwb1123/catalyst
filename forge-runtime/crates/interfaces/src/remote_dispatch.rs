use std::process::ExitCode;

use crate::{
    args::{Args, RemoteCommand},
    remote_command,
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
    match remote_command::execute(command, args.idempotency_key.as_deref()).await {
        Ok(value) => match serde_json::to_string_pretty(&value) {
            Ok(output) => {
                println!("{output}");
                if matches!(command, RemoteCommand::PromptsAdd { .. }) {
                    eprintln!("Prompt stored. No Run was started.");
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
            ExitCode::FAILURE
        }
    }
}

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
    match remote_command::run_tui().await {
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
