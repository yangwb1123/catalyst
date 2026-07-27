mod args;
mod demo;
mod hub_command;
mod hub_output;
mod state_path;

use std::{
    io::{self, Write},
    process::ExitCode,
};

use args::{Args, Command, usage};
use forge_runtime_domain::RunOutcome;
use hub_output::write_output;

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

fn argument_error(error: &str) -> ExitCode {
    let _ = writeln!(io::stderr().lock(), "{error}");
    ExitCode::from(2)
}
