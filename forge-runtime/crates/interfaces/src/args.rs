use std::{collections::VecDeque, env, path::PathBuf};

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
pub enum GroupCommand {
    Create {
        name: String,
    },
    Add {
        group_id: String,
        project: PathBuf,
        role: String,
    },
    List,
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
    if options.project.is_some() && options.group.is_some() {
        return Err(format!(
            "-C/--project and --group are mutually exclusive\n\n{}",
            usage()
        ));
    }
    if (options.project.is_some() || options.group.is_some())
        && matches!(command, Command::Prompt(_) | Command::Group(_))
    {
        return Err(format!(
            "project/group selectors are not valid for prompt or group management commands\n\n{}",
            usage()
        ));
    }
    if options.group.is_some() && matches!(command, Command::Demo(_)) {
        return Err(format!("--group is not valid for demo\n\n{}", usage()));
    }
    if options.read_path.is_some() && !matches!(command, Command::Demo(_)) {
        return Err(format!("--read is only valid for demo\n\n{}", usage()));
    }
    if options.idempotency_key.is_some() && !accepts_idempotency_key(&command) {
        return Err(format!(
            "--idempotency-key is only valid for mutating Hub commands\n\n{}",
            usage()
        ));
    }
    Ok(Args {
        state_dir: options.state_dir,
        project: options.project,
        group: options.group,
        idempotency_key: options.idempotency_key,
        json: options.json,
        command,
    })
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
            Some("--idempotency-key") => {
                tokens.pop_front();
                options.idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
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
    matches!(value, "session" | "prompt" | "group" | "demo" | "help")
}

fn accepts_idempotency_key(command: &Command) -> bool {
    matches!(
        command,
        Command::Session(SessionCommand::New { .. })
            | Command::Prompt(PromptCommand::Add { .. })
            | Command::Group(GroupCommand::Create { .. } | GroupCommand::Add { .. })
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
        "group" => parse_group(tokens),
        "demo" => parse_demo(tokens, options),
        "help" => {
            require_empty(tokens)?;
            Ok(Command::Help)
        }
        _ => unreachable!("command was checked before dispatch"),
    }
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
    let conversation_id = match tokens.front().map(String::as_str) {
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
    Ok(Command::Prompt(PromptCommand::List {
        conversation_id,
        limit,
    }))
}

fn parse_group(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("create") => Ok(Command::Group(GroupCommand::Create {
            name: required_text(tokens, "group name")?,
        })),
        Some("add") => parse_group_add(tokens),
        Some("list") => {
            require_empty(tokens)?;
            Ok(Command::Group(GroupCommand::List))
        }
        Some(value) => Err(format!("unknown group command '{value}'\n\n{}", usage())),
        None => Err(format!("group command is required\n\n{}", usage())),
    }
}

fn parse_group_add(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let group_id = next_value(tokens, "group add")?;
    let project = PathBuf::from(next_value(tokens, "group add project")?);
    let mut role = "member".to_owned();
    if tokens.front().is_some_and(|value| value == "--role") {
        tokens.pop_front();
        role = next_value(tokens, "--role")?;
    }
    require_empty(tokens)?;
    Ok(Command::Group(GroupCommand::Add {
        group_id,
        project,
        role,
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
        Err(format!("unexpected argument '{value}'\n\n{}", usage()))
    })
}

pub fn usage() -> &'static str {
    "usage:
  forge-runtime [--state-dir PATH] [--json] [PATH|-C PATH|--group GROUP_ID]
  forge-runtime [OPTIONS] [PATH|-C PATH|--group GROUP_ID] session list
  forge-runtime [OPTIONS] [PATH|-C PATH|--group GROUP_ID] session new [--title TITLE]
  forge-runtime [OPTIONS] prompt add SESSION_ID PROMPT|-
  forge-runtime [OPTIONS] prompt list [SESSION_ID] [--limit N]
  forge-runtime [OPTIONS] group create NAME
  forge-runtime [OPTIONS] group add GROUP_ID PATH [--role ROLE]
  forge-runtime [OPTIONS] group list
  forge-runtime [OPTIONS] [PATH|-C PATH] demo [--read FILE] PROMPT

  Mutations accept --idempotency-key KEY before the command.
  For prompt add, '-' reads UTF-8 prompt content from standard input.
  A PATH named session/prompt/group/demo/help must use ./PATH or -C PATH."
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
