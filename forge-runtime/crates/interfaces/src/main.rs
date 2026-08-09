mod args;
mod cli_usage;
mod demo;
mod governance_journal;
mod group_agent_graph;
mod group_analysis_panel_command;
mod group_analysis_panel_output;
mod group_context_output;
mod group_execution_output;
mod group_model_analysis_command;
mod group_model_analysis_output;
mod group_panel_synthesis_command;
mod group_panel_synthesis_output;
mod group_run_output;
mod hub_command;
mod hub_output;
mod openai_prepared_dispatch;
mod run_command;
mod runtime_application;
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
    dispatch(&args).await
}

async fn dispatch(args: &Args) -> ExitCode {
    match &args.command {
        Command::Help => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
        Command::Demo(demo_args) => run_demo(demo_args, args.project.as_deref()).await,
        Command::Governance(command) => run_governance_journal(args, command),
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
                args,
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
        Command::Group(args::GroupCommand::Analysis(command)) => {
            run_group_model_analysis(args, command).await
        }
        Command::Group(args::GroupCommand::Graph(args::GroupGraphCommand::Run(command))) => {
            Box::pin(run_group_agent_graph_run(args, command)).await
        }
        Command::Group(args::GroupCommand::Graph(command)) => run_group_agent_graph(args, command),
        Command::Group(args::GroupCommand::Panel(command)) => {
            run_group_analysis_panel(args, command)
        }
        Command::Group(args::GroupCommand::Synthesis(command)) => {
            run_group_panel_synthesis(args, command).await
        }
        _ => run_hub(args),
    }
}

async fn run_group_agent_graph_run(args: &Args, command: &args::GroupGraphRunCommand) -> ExitCode {
    let output = match Box::pin(group_agent_graph::run_command::execute(args, command)).await {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "Group Agent Graph Run command failed: {}",
                group_context_output::terminal_text(&error.to_string())
            );
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        group_agent_graph::run_command::write_output(&output, args.json, &mut io::stdout().lock())
    {
        eprintln!("failed to write Group Agent Graph Run output: {error}");
        return ExitCode::FAILURE;
    }
    // wave-admit partial failure: rejected nodes must be visible to
    // automation through the exit code, not only the JSON (Finding 4).
    if wave_rejected_count(&output) > 0 {
        eprintln!(
            "wave-admit: {} node(s) rejected; exit non-zero",
            wave_rejected_count(&output)
        );
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_group_agent_graph(args: &Args, command: &args::GroupGraphCommand) -> ExitCode {
    let output = match group_agent_graph::command::execute(args, command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "Group Agent Graph command failed: {}",
                group_context_output::terminal_text(&error.to_string())
            );
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        group_agent_graph::output::write_output(&output, args.json, &mut io::stdout().lock())
    {
        eprintln!("failed to write Group Agent Graph output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn run_group_panel_synthesis(args: &Args, command: &args::GroupSynthesisCommand) -> ExitCode {
    let output = match group_panel_synthesis_command::execute(args, command).await {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Group Panel Synthesis command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    write_cli_output(args, &output)
}

fn run_group_analysis_panel(args: &Args, command: &args::GroupPanelCommand) -> ExitCode {
    let output = match group_analysis_panel_command::execute(args, command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Group Analysis Panel command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    write_cli_output(args, &output)
}

async fn run_group_model_analysis(args: &Args, command: &args::GroupAnalysisCommand) -> ExitCode {
    let output = match group_model_analysis_command::execute(args, command).await {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Group Analysis command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    write_cli_output(args, &output)
}

fn run_hub(args: &Args) -> ExitCode {
    let output = match hub_command::execute(args) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Hub command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    write_cli_output(args, &output)
}

fn run_governance_journal(args: &Args, command: &args::GovernanceCommand) -> ExitCode {
    let output = match governance_journal::execute(args, command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Governance record journal command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) =
        governance_journal::write_output(&output, args.json, &mut io::stdout().lock())
    {
        eprintln!("failed to write governance record journal output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn write_cli_output(args: &Args, output: &hub_output::CliOutput) -> ExitCode {
    if let Err(error) = write_output(output, args.json, &mut io::stdout().lock()) {
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
    let usage_suffix = format!("\n\n{}", usage());
    let mut stderr = io::stderr().lock();
    if let Some(summary) = error.strip_suffix(&usage_suffix) {
        let _ = writeln!(
            stderr,
            "{}\n\n{}",
            group_context_output::terminal_text(summary),
            usage()
        );
    } else {
        let _ = writeln!(stderr, "{}", group_context_output::terminal_text(error));
    }
    ExitCode::from(2)
}


///  returns the number of rejected wave nodes in the
/// output, or zero for any other output shape.
fn wave_rejected_count(output: &group_agent_graph::run_command::GroupAgentGraphRunCommandCliOutput) -> usize {
    let group_agent_graph::run_command::GroupAgentGraphRunCommandCliOutput::ScheduledContract(boxed) = output else {
        return 0;
    };
    let group_agent_graph::scheduled_contract_output::GroupAgentScheduledNodeContractCliOutput::Wave { rejected, .. } =
        boxed.as_ref()
    else {
        return 0;
    };
    rejected.len()
}
