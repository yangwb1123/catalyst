use super::{
    Command, GlobalOptions, GovernanceCommand, GovernanceJournalCommand, GroupAnalysisCommand,
    GroupCommand, GroupExecutionCommand, GroupGraphCommand, GroupGraphRunCommand,
    GroupGraphRunContractCommand, GroupGraphRunDispatchCommand, GroupGraphRunScheduleCommand,
    GroupGraphRunScheduledContractCommand, GroupGraphRunScheduledContractProviderRequestCommand,
    GroupGraphRunScheduledContractSuccessorCommand, GroupPanelCommand, GroupRunCommand,
    GroupSynthesisCommand, PromptCommand, RunCommand, SessionCommand, usage,
};

pub(super) fn validate_options(options: &GlobalOptions, command: &Command) -> Result<(), String> {
    validate_scope_options(options, command)?;
    validate_execution_options(options, command)
}

fn validate_scope_options(options: &GlobalOptions, command: &Command) -> Result<(), String> {
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
                | Command::Governance(_)
                | Command::Group(_)
                | Command::Run(
                    RunCommand::List { .. }
                        | RunCommand::Show { .. }
                        | RunCommand::Explain { .. }
                        | RunCommand::Lineage { .. },
                )
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
            Command::Demo(_)
                | Command::Run(
                    RunCommand::Start { .. }
                        | RunCommand::Resume { .. }
                        | RunCommand::Restart { .. }
                        | RunCommand::Branch { .. }
                )
        )
    {
        return Err(format!(
            "--group is not valid for local execution\n\n{}",
            usage()
        ));
    }
    require_project(options, command)
}

fn require_project(options: &GlobalOptions, command: &Command) -> Result<(), String> {
    if let Some(operation) = project_required_operation(command)
        && options.project.is_none()
    {
        return Err(format!(
            "run {operation} requires -C/--project PATH\n\n{}",
            usage()
        ));
    }
    Ok(())
}

fn project_required_operation(command: &Command) -> Option<&'static str> {
    match command {
        Command::Run(RunCommand::Start { .. }) => Some("start"),
        Command::Run(RunCommand::Resume { .. }) => Some("resume"),
        Command::Run(RunCommand::Restart { .. }) => Some("restart"),
        Command::Run(RunCommand::Branch { .. }) => Some("branch"),
        _ => None,
    }
}

fn dispatch_claim_key_error(command: &Command) -> Option<&'static str> {
    match command {
        Command::Group(GroupCommand::Analysis(GroupAnalysisCommand::Send { .. })) => Some(
            "--idempotency-key is not valid for group analysis send; ANALYSIS_ID owns the single dispatch claim",
        ),
        Command::Group(GroupCommand::Synthesis(GroupSynthesisCommand::Send { .. })) => Some(
            "--idempotency-key is not valid for group synthesis send; SYNTHESIS_ID owns the single dispatch claim",
        ),
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Dispatch(GroupGraphRunDispatchCommand::Execute { .. }),
        ))) => Some(
            "--idempotency-key is not valid for graph dispatch execute; GRAPH_RUN_ID owns the single dispatch claim",
        ),
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Dispatch(GroupGraphRunDispatchCommand::Adjudicate { .. }),
        ))) => Some(
            "--idempotency-key is not valid for graph dispatch adjudicate; GRAPH_RUN_ID owns the single dispatch claim",
        ),
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::ScheduledContract(
                GroupGraphRunScheduledContractCommand::ProviderRequest(
                    GroupGraphRunScheduledContractProviderRequestCommand::Execute { .. },
                ),
            ),
        ))) => Some(
            "--idempotency-key is not valid for scheduled dispatch execute; PROVIDER_REQUEST_ID owns the single dispatch claim",
        ),
        _ => None,
    }
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
                                    | GroupGraphRunCommand::Schedule(
                                        GroupGraphRunScheduleCommand::Admit { .. },
                                    )
                                    | GroupGraphRunCommand::ScheduledContract(
                                        GroupGraphRunScheduledContractCommand::Admit { .. }
                                            | GroupGraphRunScheduledContractCommand::Successor(
                                                GroupGraphRunScheduledContractSuccessorCommand::Admit { .. },
                                            )
                                            | GroupGraphRunScheduledContractCommand::ProviderRequest(
                                                GroupGraphRunScheduledContractProviderRequestCommand::Prepare { .. },
                                            ),
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
            | Command::Run(
                RunCommand::Start { .. }
                    | RunCommand::Restart { .. }
                    | RunCommand::Branch { .. },
            )
            | Command::Governance(GovernanceCommand::Journal(
                GovernanceJournalCommand::Append { .. },
            ))
    )
}

fn validate_execution_options(options: &GlobalOptions, command: &Command) -> Result<(), String> {
    if let Some(message) = explicit_key_requirement(command)
        && options.idempotency_key.is_none()
    {
        return Err(format!("{message}\n\n{}", usage()));
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

fn explicit_key_requirement(command: &Command) -> Option<&'static str> {
    match command {
        Command::Governance(GovernanceCommand::Journal(GovernanceJournalCommand::Append {
            ..
        })) => Some("governance journal append requires an explicit --idempotency-key"),
        Command::Run(RunCommand::Start { live: true, .. }) => {
            Some("--live requires an explicit --idempotency-key")
        }
        Command::Run(RunCommand::Restart { .. }) => {
            Some("run restart requires an explicit --idempotency-key")
        }
        Command::Run(RunCommand::Branch { .. }) => {
            Some("run branch requires an explicit --idempotency-key")
        }
        _ => None,
    }
}
