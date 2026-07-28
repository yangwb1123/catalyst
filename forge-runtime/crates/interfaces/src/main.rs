mod args;
mod cli_usage;
mod demo;
mod group_context_output;
mod group_run_output;
mod hub_command;
mod hub_output;
mod run_command;
mod state_path;

use std::{
    io::{self, Write},
    process::ExitCode,
};

use args::{Args, Command, usage};
use hub_output::write_output;

pub(crate) use forge_runtime_domain as runtime_domain;
use runtime_domain::RunOutcome;

#[tokio::main]
async fn main() -> ExitCode {
    let args = match Args::parse() {
        Ok(args) => args,
        Err(error) => return argument_error(&error),
    };
    match &args.command {
        Command::Help => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
        Command::Demo(demo_args) => run_demo(demo_args, args.project.as_deref()).await,
        Command::Run(args::RunCommand::Start {
            conversation_id,
            prompt_id,
            read_path,
            allowed_read_paths,
            live,
            model,
            max_output_tokens,
        }) => {
            run_persisted(
                &args,
                run_command::StartOptions {
                    conversation_id,
                    prompt_id,
                    read_path,
                    allowed_read_paths,
                    live: *live,
                    model: model.as_deref(),
                    max_output_tokens: *max_output_tokens,
                },
            )
            .await
        }
        _ => run_hub(&args),
    }
}

fn run_hub(args: &Args) -> ExitCode {
    let output = match hub_command::execute(args) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Hub command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    if let Err(error) = write_output(&output, args.json, &mut writer) {
        eprintln!("failed to write CLI output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn run_demo(args: &args::DemoArgs, project: Option<&std::path::Path>) -> ExitCode {
    match demo::run(args, project).await {
        Ok(RunOutcome::Completed { .. }) => ExitCode::SUCCESS,
        Ok(outcome) => {
            eprintln!("runtime stopped without completion: {outcome:?}");
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("runtime failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run_persisted(args: &Args, options: run_command::StartOptions<'_>) -> ExitCode {
    match run_command::start(args, options).await {
        Ok(RunOutcome::Completed { .. }) => ExitCode::SUCCESS,
        Ok(outcome) => {
            eprintln!("runtime stopped without completion: {outcome:?}");
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("Run command failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn argument_error(error: &str) -> ExitCode {
    let _ = writeln!(io::stderr().lock(), "{error}");
    ExitCode::from(2)
}
