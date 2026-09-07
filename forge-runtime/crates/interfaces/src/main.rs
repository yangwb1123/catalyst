mod agent_command;
mod agent_run_limits;
mod agent_workspace;
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
mod human_event_sink;
mod openai_prepared_dispatch;
mod run_branch_command;
mod run_branch_output;
mod run_command;
mod run_lineage_output;
mod run_provider;
mod run_restart_command;
mod run_restart_output;
mod run_selection;
mod runtime_application;
mod state_path;

use std::{
    io::{self, Write},
    process::ExitCode,
};

use args::{Args, Command, usage};
use hub_output::write_output;
use runtime_application::RuntimeError;

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
        Command::Agent(agent_args) => run_agent(args, agent_args).await,
        Command::Governance(command) => run_governance_journal(args, command),
        Command::Run(command) => run_project_command(args, command).await,
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

async fn run_project_command(args: &Args, command: &args::RunCommand) -> ExitCode {
    match command {
        args::RunCommand::Start {
            conversation_id,
            prompt_id,
            read_path,
            allowed_read_paths,
            live,
            model,
            max_output_tokens,
        } => {
            let mode = if *live {
                run_command::StartMode::Live
            } else {
                run_command::StartMode::Deterministic
            };
            let max_tool_calls = if *live && allowed_read_paths.is_empty() {
                0
            } else {
                4
            };
            run_persisted(
                args,
                run_command::StartOptions {
                    conversation_id,
                    prompt_id,
                    read_path,
                    allowed_read_paths,
                    mode,
                    model: model.as_deref(),
                    max_output_tokens: *max_output_tokens,
                    output: run_command::StartOutput::JsonLines,
                    max_turns: 4,
                    max_tool_calls,
                },
            )
            .await
        }
        args::RunCommand::Resume { run_id } => run_resumed(args, run_id).await,
        args::RunCommand::Restart { run_id } => run_restarted(args, run_id),
        args::RunCommand::Branch { run_id } => run_branched(args, run_id),
        _ => run_hub(args),
    }
}

fn run_branched(args: &Args, parent_run_id: &str) -> ExitCode {
    let output = match run_branch_command::prepare(args, parent_run_id) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Run branch failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = run_branch_output::write(&output, args.json, &mut io::stdout().lock()) {
        eprintln!("failed to write Run branch output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_restarted(args: &Args, source_run_id: &str) -> ExitCode {
    let output = match run_restart_command::prepare(args, source_run_id) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Run restart failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = run_restart_output::write(&output, args.json, &mut io::stdout().lock()) {
        eprintln!("failed to write Run restart output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
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

async fn run_agent(args: &Args, agent_args: &args::AgentArgs) -> ExitCode {
    match agent_command::run(args, agent_args).await {
        Ok(RunOutcome::Completed { .. }) => ExitCode::SUCCESS,
        Ok(outcome) => {
            write_agent_stopped(args, &outcome);
            ExitCode::from(2)
        }
        Err(error) => {
            if args.json {
                let code = error
                    .downcast_ref::<agent_command::AgentExecutionError>()
                    .map_or(
                        "agent_setup_failed",
                        agent_command::AgentExecutionError::code,
                    );
                eprintln!(
                    "{}",
                    serde_json::json!({"type": "agent_failed", "code": code})
                );
            } else {
                eprintln!(
                    "Agent failed: {}",
                    group_context_output::terminal_text(&error.to_string())
                );
            }
            ExitCode::FAILURE
        }
    }
}

fn write_agent_stopped(args: &Args, outcome: &RunOutcome) {
    let (status, detail) = match outcome {
        RunOutcome::Completed { .. } => ("completed", None),
        RunOutcome::Cancelled => ("cancelled", None),
        RunOutcome::LimitExceeded { kind } => ("limit_exceeded", Some(format!("{kind:?}"))),
        RunOutcome::Failed { code, .. } => ("failed", Some(code.clone())),
    };
    if args.json {
        eprintln!(
            "{}",
            serde_json::json!({"type": "agent_stopped", "status": status, "detail": detail})
        );
    } else if let Some(detail) = detail {
        eprintln!(
            "agent stopped without completion: {status} ({})",
            group_context_output::terminal_text(&detail)
        );
    } else {
        eprintln!("agent stopped without completion: {status}");
    }
}

async fn run_resumed(args: &Args, run_id: &str) -> ExitCode {
    match run_command::resume(args, run_id).await {
        Ok(RunOutcome::Completed { .. }) => ExitCode::SUCCESS,
        Ok(outcome) => {
            write_resume_stopped(&outcome);
            ExitCode::from(2)
        }
        Err(error) => {
            let detail = error.downcast_ref::<RuntimeError>().map_or_else(
                || group_context_output::terminal_text(&error.to_string()),
                |runtime| runtime.code().to_owned(),
            );
            eprintln!("Run resume failed: {detail}");
            ExitCode::FAILURE
        }
    }
}

fn write_resume_stopped(outcome: &RunOutcome) {
    let detail = match outcome {
        RunOutcome::Cancelled => "cancelled".into(),
        RunOutcome::LimitExceeded { kind } => format!("limit_exceeded ({kind:?})"),
        RunOutcome::Failed { code, .. } => {
            format!("failed ({})", group_context_output::terminal_text(code))
        }
        RunOutcome::Completed { .. } => "completed".into(),
    };
    eprintln!("runtime stopped without completion: {detail}");
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
fn wave_rejected_count(
    output: &group_agent_graph::run_command::GroupAgentGraphRunCommandCliOutput,
) -> usize {
    let group_agent_graph::run_command::GroupAgentGraphRunCommandCliOutput::ScheduledContract(
        boxed,
    ) = output
    else {
        return 0;
    };
    let group_agent_graph::scheduled_contract_output::GroupAgentScheduledNodeContractCliOutput::Wave { rejected, .. } =
        boxed.as_ref()
    else {
        return 0;
    };
    rejected.len()
}
