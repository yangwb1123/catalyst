use std::{collections::VecDeque, env, path::PathBuf};

#[path = "group_analysis_args.rs"]
mod group_analysis_args;
#[path = "group_args.rs"]
mod group_args;
#[path = "group_commands.rs"]
mod group_commands;
#[path = "group_agent_graph/args.rs"]
mod group_graph_args;
#[path = "group_panel_args.rs"]
mod group_panel_args;
#[path = "group_synthesis_args.rs"]
mod group_synthesis_args;
#[path = "run_args.rs"]
mod run_args;

pub use group_commands::{
    GroupAnalysisCommand, GroupCommand, GroupExecutionCommand, GroupGraphCommand,
    GroupGraphRunCommand, GroupGraphRunContractCommand, GroupGraphRunControlCommand,
    GroupGraphRunDispatchCommand, GroupPanelCommand, GroupRunCommand, GroupSynthesisCommand,
};

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
    Session(SessionCommand),
    Prompt(PromptCommand),
    Group(GroupCommand),
    Run(RunCommand),
    Demo(DemoArgs),
    Help,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SessionCommand {
    List,
    New { title: String },
}

#[derive(Debug, Eq, PartialEq)]
pub enum PromptCommand {
    Add {
        conversation_id: String,
        text: String,
    },
    List {
        conversation_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum RunCommand {
    Start {
        conversation_id: String,
        prompt_id: String,
        read_path: String,
        allowed_read_paths: Vec<String>,
        live: bool,
        model: Option<String>,
        max_output_tokens: u32,
    },
    List {
        conversation_id: Option<String>,
        limit: usize,
    },
    Show {
        run_id: String,
    },
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

fn parse_tokens(tokens: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut tokens: VecDeque<_> = tokens.into_iter().collect();
    let mut options = parse_global_options(&mut tokens)?;
    let command = parse_command(&mut tokens, &mut options)?;
    validate_options(&options, &command)?;
    Ok(Args {
        state_dir: options.state_dir,
        project: options.project,
        group: options.group,
        idempotency_key: options.idempotency_key,
        json: options.json,
        command,
    })
}

fn validate_options(options: &GlobalOptions, command: &Command) -> Result<(), String> {
    if options.project.is_some() && options.group.is_some() {
        return Err(format!(
            "-C/--project and --group are mutually exclusive\n\n{}",
            usage()
        ));
    }
    if (options.project.is_some() || options.group.is_some())
        && matches!(
            command,
            Command::Prompt(_)
                | Command::Group(_)
                | Command::Run(RunCommand::List { .. } | RunCommand::Show { .. })
        )
    {
        return Err(format!(
            "project/group selectors are not valid for this management command\n\n{}",
            usage()
        ));
    }
    if options.group.is_some()
        && matches!(
            command,
            Command::Demo(_) | Command::Run(RunCommand::Start { .. })
        )
    {
        return Err(format!(
            "--group is not valid for local execution\n\n{}",
            usage()
        ));
    }
    if matches!(command, Command::Run(RunCommand::Start { .. })) && options.project.is_none() {
        return Err(format!(
            "run start requires -C/--project PATH\n\n{}",
            usage()
        ));
    }
    validate_execution_options(options, command)
}

fn validate_execution_options(options: &GlobalOptions, command: &Command) -> Result<(), String> {
    if matches!(command, Command::Run(RunCommand::Start { live: true, .. }))
        && options.idempotency_key.is_none()
    {
        return Err(format!(
            "--live requires an explicit --idempotency-key\n\n{}",
            usage()
        ));
    }
    if options.read_path.is_some()
        && !matches!(
            command,
            Command::Demo(_) | Command::Run(RunCommand::Start { .. })
        )
    {
        return Err(format!(
            "--read is only valid for demo or run start\n\n{}",
            usage()
        ));
    }
    if options.idempotency_key.is_some()
        && let Some(message) = dispatch_claim_key_error(command)
    {
        return Err(format!("{message}\n\n{}", usage()));
    }
    if options.idempotency_key.is_some() && !accepts_idempotency_key(command) {
        return Err(format!(
            "--idempotency-key is only valid for mutating commands\n\n{}",
            usage()
        ));
    }
    Ok(())
}

fn dispatch_claim_key_error(command: &Command) -> Option<&'static str> {
    match command {
        Command::Group(GroupCommand::Analysis(GroupAnalysisCommand::Send { .. })) => Some(
            "--idempotency-key is not valid for group analysis send; ANALYSIS_ID owns the single dispatch claim",
        ),
        Command::Group(GroupCommand::Synthesis(GroupSynthesisCommand::Send { .. })) => Some(
            "--idempotency-key is not valid for group synthesis send; SYNTHESIS_ID owns the single dispatch claim",
        ),
        _ => None,
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
        "session" | "prompt" | "group" | "run" | "demo" | "help"
    )
}

fn accepts_idempotency_key(command: &Command) -> bool {
    matches!(
        command,
        Command::Session(SessionCommand::New { .. })
            | Command::Prompt(PromptCommand::Add { .. })
            | Command::Group(
                GroupCommand::Create { .. }
                    | GroupCommand::Add { .. }
                    | GroupCommand::Analysis(GroupAnalysisCommand::Prepare { .. })
                    | GroupCommand::Execution(GroupExecutionCommand::Start { .. })
                    | GroupCommand::Graph(
                        GroupGraphCommand::Prepare { .. }
                            | GroupGraphCommand::Run(
                                GroupGraphRunCommand::Prepare { .. }
                                    | GroupGraphRunCommand::Contract(
                                        GroupGraphRunContractCommand::Admit { .. },
                                    )
                                    | GroupGraphRunCommand::Dispatch(
                                        GroupGraphRunDispatchCommand::Prepare { .. },
                                    ),
                            ),
                    )
                    | GroupCommand::Panel(GroupPanelCommand::Prepare { .. })
                    | GroupCommand::Run(GroupRunCommand::Prepare { .. })
                    | GroupCommand::Synthesis(GroupSynthesisCommand::Prepare { .. })
            )
            | Command::Run(RunCommand::Start { .. })
    )
}

fn parse_named_command(
    command: &str,
    tokens: &mut VecDeque<String>,
    options: &mut GlobalOptions,
) -> Result<Command, String> {
    match command {
        "session" => parse_session(tokens),
        "prompt" => parse_prompt(tokens),
        "group" => group_args::parse(tokens, &mut options.idempotency_key),
        "run" => run_args::parse(tokens, options),
        "demo" => parse_demo(tokens, options),
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

fn parse_session(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => {
            require_empty(tokens)?;
            Ok(Command::Session(SessionCommand::List))
        }
        Some("new") => {
            let title = parse_optional_title(tokens)?;
            Ok(Command::Session(SessionCommand::New { title }))
        }
        Some(value) => Err(format!("unknown session command '{value}'\n\n{}", usage())),
        None => Err(format!("session command is required\n\n{}", usage())),
    }
}

fn parse_optional_title(tokens: &mut VecDeque<String>) -> Result<String, String> {
    if tokens.front().is_some_and(|value| value == "--title") {
        tokens.pop_front();
        let title = next_value(tokens, "--title")?;
        require_empty(tokens)?;
        return Ok(title);
    }
    if tokens.is_empty() {
        return Ok("New conversation".into());
    }
    Ok(drain_text(tokens))
}

fn parse_prompt(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("add") => {
            let conversation_id = next_value(tokens, "prompt add")?;
            let text = required_text(tokens, "prompt text")?;
            Ok(Command::Prompt(PromptCommand::Add {
                conversation_id,
                text,
            }))
        }
        Some("list") => parse_prompt_list(tokens),
        Some(value) => Err(format!("unknown prompt command '{value}'\n\n{}", usage())),
        None => Err(format!("prompt command is required\n\n{}", usage())),
    }
}

fn parse_prompt_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let (conversation_id, limit) = parse_optional_id_and_limit(tokens)?;
    Ok(Command::Prompt(PromptCommand::List {
        conversation_id,
        limit,
    }))
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

fn next_value(tokens: &mut VecDeque<String>, option: &str) -> Result<String, String> {
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

fn require_empty(tokens: &VecDeque<String>) -> Result<(), String> {
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
#[path = "args_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "group_run_args_tests.rs"]
mod group_run_tests;

#[cfg(test)]
#[path = "group_execution_args_tests.rs"]
mod group_execution_tests;

#[cfg(test)]
#[path = "group_agent_graph/args_tests.rs"]
mod group_graph_tests;

#[cfg(test)]
#[path = "group_agent_graph/dispatch_args_tests.rs"]
mod group_graph_dispatch_tests;

#[cfg(test)]
#[path = "group_analysis_args_tests.rs"]
mod group_analysis_tests;

#[cfg(test)]
#[path = "group_panel_args_tests.rs"]
mod group_panel_tests;

#[cfg(test)]
#[path = "group_synthesis_args_tests.rs"]
mod group_synthesis_tests;
