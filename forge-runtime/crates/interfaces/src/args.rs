use std::{collections::VecDeque, env, path::PathBuf};
#[path = "args_validation.rs"]
mod args_validation;
#[path = "args/basic.rs"]
mod basic_args;
#[path = "governance_journal/args.rs"]
mod governance_journal_args;
#[path = "group_analysis_args.rs"]
mod group_analysis_args;
#[path = "group_args.rs"]
mod group_args;
#[path = "group_commands.rs"]
mod group_commands;
#[path = "group_agent_graph/args.rs"]
pub(crate) mod group_graph_args;
#[path = "group_panel_args.rs"]
mod group_panel_args;
#[path = "group_synthesis_args.rs"]
mod group_synthesis_args;
#[path = "run_args.rs"]
mod run_args;
pub use basic_args::{PromptCommand, SessionCommand};
pub use governance_journal_args::{GovernanceCommand, GovernanceJournalCommand};
pub use group_commands::{
    GroupAnalysisCommand, GroupCommand, GroupExecutionCommand, GroupGraphCommand,
    GroupGraphRunCommand, GroupGraphRunContractCommand, GroupGraphRunControlCommand,
    GroupGraphRunControllerCommand, GroupGraphRunControllerStartOptions,
    GroupGraphRunControllerStepOptions, GroupGraphRunDispatchCommand,
    GroupGraphRunReadyStepOptions, GroupGraphRunScheduleCommand,
    GroupGraphRunScheduledContractCommand, GroupGraphRunScheduledContractProviderRequestCommand,
    GroupGraphRunScheduledContractSuccessorCommand, GroupPanelCommand, GroupRunCommand,
    GroupSynthesisCommand, WaveAdmitExecutionOptions,
};
pub use run_args::RunCommand;
#[derive(Debug, Eq, PartialEq)]
pub struct Args {
    pub state_dir: Option<PathBuf>,
    pub project: Option<PathBuf>,
    pub group: Option<String>,
    pub idempotency_key: Option<String>,
    pub json: bool,
    pub command: Command,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Hub,
    HubStatus, // readiness probe (no migration)
    Session(SessionCommand),
    Prompt(PromptCommand),
    Governance(GovernanceCommand),
    Group(GroupCommand),
    Run(RunCommand),
    Demo(DemoArgs),
    Help,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DemoArgs {
    pub read_path: String,
    pub prompt: String,
}

#[derive(Default)]
struct GlobalOptions {
    state_dir: Option<PathBuf>,
    project: Option<PathBuf>,
    group: Option<String>,
    idempotency_key: Option<String>,
    read_path: Option<String>,
    json: bool,
    legacy_demo: bool,
}

impl Args {
    pub fn parse() -> Result<Self, String> {
        parse_tokens(env::args().skip(1))
    }
}

fn parse_global_options(tokens: &mut VecDeque<String>) -> Result<GlobalOptions, String> {
    let mut options = GlobalOptions::default();
    loop {
        match tokens.front().map(String::as_str) {
            Some("--state-dir") => {
                tokens.pop_front();
                options.state_dir = Some(PathBuf::from(next_value(tokens, "--state-dir")?));
            }
            Some("-C" | "--project" | "--workspace") => {
                let option = tokens.pop_front().expect("matched token exists");
                options.project = Some(PathBuf::from(next_value(tokens, &option)?));
                options.legacy_demo |= option == "--workspace";
            }
            Some("--group") => {
                tokens.pop_front();
                options.group = Some(next_value(tokens, "--group")?);
            }
            Some("--idempotency-key") if options.idempotency_key.is_none() => {
                tokens.pop_front();
                options.idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            Some("--idempotency-key") => {
                return Err(format!(
                    "--idempotency-key was specified more than once\n\n{}",
                    usage()
                ));
            }
            Some("--read") => {
                tokens.pop_front();
                options.read_path = Some(next_value(tokens, "--read")?);
                options.legacy_demo = true;
            }
            Some("--json") => {
                tokens.pop_front();
                options.json = true;
            }
            Some("--help" | "-h") => {
                tokens.clear();
                tokens.push_back("help".into());
                return Ok(options);
            }
            Some(value) if value.starts_with('-') => {
                return Err(format!("unknown option '{value}'\n\n{}", usage()));
            }
            _ => return Ok(options),
        }
    }
}

fn parse_command(
    tokens: &mut VecDeque<String>,
    options: &mut GlobalOptions,
) -> Result<Command, String> {
    let Some(first) = tokens.pop_front() else {
        return Ok(Command::Hub);
    };
    if is_command(&first) {
        return parse_named_command(&first, tokens, options);
    }
    if options.legacy_demo {
        tokens.push_front(first);
        return parse_demo(tokens, options);
    }
    if options.project.is_some() || options.group.is_some() {
        return Err(format!("unexpected argument '{first}'\n\n{}", usage()));
    }
    options.project = Some(PathBuf::from(first));
    let Some(command) = tokens.pop_front() else {
        return Ok(Command::Hub);
    };
    if !is_command(&command) {
        return Err(format!(
            "expected a command after the project path, got '{command}'\n\n{}",
            usage()
        ));
    }
    parse_named_command(&command, tokens, options)
}

fn is_command(value: &str) -> bool {
    matches!(
        value,
        "session" | "prompt" | "governance" | "group" | "run" | "demo" | "help" | "status"
    )
}

pub(crate) fn parse_tokens(tokens: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut tokens: VecDeque<_> = tokens.into_iter().collect();
    let mut options = parse_global_options(&mut tokens)?;
    let command = parse_command(&mut tokens, &mut options)?;
    args_validation::validate_options(&options, &command)?;
    Ok(Args {
        state_dir: options.state_dir,
        project: options.project,
        group: options.group,
        idempotency_key: options.idempotency_key,
        json: options.json,
        command,
    })
}

fn parse_named_command(
    command: &str,
    tokens: &mut VecDeque<String>,
    options: &mut GlobalOptions,
) -> Result<Command, String> {
    match command {
        "session" => basic_args::parse_session(tokens),
        "prompt" => basic_args::parse_prompt(tokens),
        "governance" => governance_journal_args::parse(tokens),
        "group" => group_args::parse(tokens, &mut options.idempotency_key),
        "run" => run_args::parse(tokens, options),
        "demo" => parse_demo(tokens, options),
        "status" => {
            require_empty(tokens)?;
            Ok(Command::HubStatus)
        }
        "help" => {
            require_empty(tokens)?;
            Ok(Command::Help)
        }
        _ => unreachable!("command was checked before dispatch"),
    }
}

fn parse_optional_id_and_limit(
    tokens: &mut VecDeque<String>,
) -> Result<(Option<String>, usize), String> {
    let id = match tokens.front().map(String::as_str) {
        Some("--limit") | None => None,
        Some(_) => tokens.pop_front(),
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        let value = next_value(tokens, "--limit")?;
        limit = value
            .parse()
            .map_err(|_| format!("invalid --limit '{value}'"))?;
    }
    require_empty(tokens)?;
    Ok((id, limit))
}

fn parse_demo(
    tokens: &mut VecDeque<String>,
    options: &mut GlobalOptions,
) -> Result<Command, String> {
    if tokens.front().is_some_and(|value| value == "--read") {
        tokens.pop_front();
        options.read_path = Some(next_value(tokens, "--read")?);
    }
    let prompt = required_text(tokens, "demo prompt")?;
    Ok(Command::Demo(DemoArgs {
        read_path: options
            .read_path
            .take()
            .unwrap_or_else(|| "README.md".into()),
        prompt,
    }))
}

pub(crate) fn next_value(tokens: &mut VecDeque<String>, option: &str) -> Result<String, String> {
    tokens
        .pop_front()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn required_text(tokens: &mut VecDeque<String>, field: &str) -> Result<String, String> {
    if tokens.is_empty() {
        return Err(format!("{field} is required\n\n{}", usage()));
    }
    Ok(drain_text(tokens))
}

fn drain_text(tokens: &mut VecDeque<String>) -> String {
    tokens.drain(..).collect::<Vec<_>>().join(" ")
}

pub(crate) fn require_empty(tokens: &VecDeque<String>) -> Result<(), String> {
    tokens.front().map_or(Ok(()), |value| {
        Err(format!(
            "unexpected argument '{}'\n\n{}",
            crate::group_context_output::terminal_text(value),
            usage()
        ))
    })
}

pub fn usage() -> &'static str {
    crate::cli_usage::TEXT
}

#[cfg(test)]
#[path = "group_analysis_args_tests.rs"]
mod group_analysis_tests;
#[cfg(test)]
#[path = "group_execution_args_tests.rs"]
mod group_execution_tests;
#[cfg(test)]
#[path = "group_agent_graph/dispatch_args_tests.rs"]
mod group_graph_dispatch_tests;
#[cfg(test)]
#[path = "group_agent_graph/args_tests.rs"]
mod group_graph_tests;
#[cfg(test)]
#[path = "group_run_args_tests.rs"]
mod group_run_tests;
#[cfg(test)]
#[path = "run_branch_args_tests.rs"]
mod run_branch_tests;
#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "group_panel_args_tests.rs"]
mod group_panel_tests;

#[cfg(test)]
#[path = "governance_journal/args_tests.rs"]
mod governance_journal_tests;
#[cfg(test)]
#[path = "group_synthesis_args_tests.rs"]
mod group_synthesis_tests;
