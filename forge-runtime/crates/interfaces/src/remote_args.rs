use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

#[path = "remote_args/execution_consent.rs"]
mod execution_consent;
#[path = "remote_args/execution_reconciliation.rs"]
mod execution_reconciliation;
#[path = "remote_args/local_runner_preview.rs"]
mod local_runner_preview;
#[path = "remote_args/prompts.rs"]
mod prompts;
#[path = "remote_args/run_attempt_lease_dispatch_preflight.rs"]
mod run_attempt_lease_dispatch_preflight;
#[path = "remote_args/runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_args/session_runner_receipt.rs"]
mod session_runner_receipt;

#[derive(Debug, Eq, PartialEq)]
pub enum RemoteCommand {
    Login,
    Tui,
    CredentialStorageStatus,
    PlacementPreview {
        input: String,
    },
    PlacementRegistryPreview {
        input: String,
    },
    CredentialCandidatePreview {
        input: String,
    },
    SessionObservationPreview {
        input: String,
    },
    SessionRunnerReceiptPreview {
        input: String,
    },
    RunAttemptLeaseDispatchPreflightPreview {
        input: String,
    },
    RunnerDispatchPlanPreview {
        input: String,
    },
    LocalRunnerPreview {
        input: String,
    },
    ExecutionConsentPreview {
        conversation_id: String,
    },
    ExecutionReconciliationPreview {
        input: String,
    },
    InventoryShow,
    InventoryShowV2,
    LifecycleRegistryShow,
    ClientInstanceSessionView,
    ClientInstanceResourceView,
    SessionsList {
        after_id: Option<String>,
        scope: Option<RemoteConversationScope>,
        instance_id: Option<String>,
        instance_view: Option<String>,
        all_pages: bool,
    },
    SessionsShow {
        conversation_id: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    SessionsCreate {
        title: String,
        scope: RemoteConversationScope,
    },
    SessionsImport {
        conversation_id: String,
        confirm: Option<String>,
    },
    RunsList {
        conversation_id: String,
        limit: usize,
        before_created_at_ms: Option<u64>,
        before_run_id: Option<String>,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunObserved {
        conversation_id: String,
        run_id: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunTimeline {
        conversation_id: String,
        run_id: String,
        after_sequence: u64,
        limit: usize,
        resume: bool,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PendingRunIntentsList {
        conversation_id: String,
        limit: usize,
        before_submitted_at_ms: Option<u64>,
        before_intent_id: Option<String>,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PendingRunIntentSubmit {
        conversation_id: String,
        expected_version: u64,
        content: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PendingRunIntentTimeline {
        conversation_id: String,
        intent_id: String,
        after_sequence: u64,
        limit: usize,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PromptsList {
        conversation_id: String,
        before_created_at_ms: Option<u64>,
        before_prompt_id: Option<String>,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PromptsAdd {
        conversation_id: String,
        expected_version: u64,
        content: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    ChangesList {
        after_cursor: Option<u64>,
    },
    ChangesWatch {
        after_cursor: Option<u64>,
        polls: usize,
        min_delay_ms: u64,
        max_delay_ms: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteConversationScope {
    Global,
    Project(String),
    Group(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptPageCursor {
    pub created_at_ms: u64,
    pub prompt_id: String,
}

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("login") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::Login))
        }
        Some("tui") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::Tui))
        }
        Some("credentials") => parse_credentials(tokens),
        Some("placement") => parse_placement(tokens),
        Some("credential-candidate") => parse_credential_candidate(tokens),
        Some("session-observation") => parse_session_observation(tokens),
        Some("session-runner-receipt") => session_runner_receipt::parse(tokens),
        Some("runner") => local_runner_preview::parse(tokens),
        Some("run-attempt-lease-dispatch-preflight-preview") => {
            run_attempt_lease_dispatch_preflight::parse(tokens)
        }
        Some("runner-dispatch-plan-preview") => runner_dispatch_plan_preview::parse(tokens),
        Some("execution-consent") => execution_consent::parse(tokens),
        Some("execution-reconciliation") => execution_reconciliation::parse(tokens),
        Some("inventory") => parse_inventory(tokens),
        Some("lifecycle-registry") => parse_lifecycle_registry(tokens),
        Some("client-instances") => parse_client_instances(tokens),
        Some("sessions") => parse_sessions(tokens),
        Some("runs") => parse_runs(tokens),
        Some("run-intents") => parse_pending_run_intents(tokens),
        Some("prompts") => prompts::parse(tokens),
        Some("changes") => parse_changes(tokens),
        Some(value) => Err(format!("unknown remote command '{value}'\n\n{}", usage())),
        None => Err(format!("remote command is required\n\n{}", usage())),
    }
}

fn parse_lifecycle_registry(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("show" | "read") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::LifecycleRegistryShow))
        }
        Some(value) => Err(format!(
            "unknown remote lifecycle-registry command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote lifecycle-registry command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_credentials(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("status") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::CredentialStorageStatus))
        }
        Some(value) => Err(format!(
            "unknown remote credentials command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote credentials command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_inventory(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("show") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::InventoryShow))
        }
        Some("show-v2") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::InventoryShowV2))
        }
        Some(value) => Err(format!(
            "unknown remote inventory command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote inventory command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_client_instances(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("session-view") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::ClientInstanceSessionView))
        }
        Some("resource-view") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::ClientInstanceResourceView))
        }
        Some(value) => Err(format!(
            "unknown remote client-instances command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote client-instances command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_pending_run_intents(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_pending_run_intent_list(tokens),
        Some("submit") => parse_pending_run_intent_submit(tokens),
        Some("timeline") => parse_pending_run_intent_timeline(tokens),
        Some(value) => Err(format!(
            "unknown remote run-intents command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote run-intents command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_pending_run_intent_submit(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote run-intents submit")?;
    if tokens
        .front()
        .is_none_or(|value| value != "--expected-version")
    {
        return Err(format!(
            "remote run-intent submit requires --expected-version from the session list\n\n{}",
            usage()
        ));
    }
    tokens.pop_front();
    let version = next_value(tokens, "--expected-version")?;
    let expected_version = version
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0 && *value <= 9_007_199_254_740_991)
        .ok_or_else(|| format!("invalid --expected-version '{version}'\n\n{}", usage()))?;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.front().map(String::as_str) {
        match option {
            "--instance" => {
                if instance_id.is_some() {
                    return Err(format!(
                        "invalid remote run-intents submit option '--instance'\n\n{}",
                        usage()
                    ));
                }
                tokens.pop_front();
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" => {
                if instance_view.is_some() {
                    return Err(format!(
                        "invalid remote run-intents submit option '--instance-view'\n\n{}",
                        usage()
                    ));
                }
                tokens.pop_front();
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => break,
        }
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote run-intents submit --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    if tokens.is_empty() {
        return Err(format!(
            "pending Run-intent content is required\n\n{}",
            usage()
        ));
    }
    let content_tokens = tokens.drain(..).collect::<Vec<_>>();
    if content_tokens.first().is_some_and(|token| token == "-") && content_tokens.len() != 1 {
        return Err(format!(
            "remote stdin pending Run-intent marker '-' must be the only content token\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::PendingRunIntentSubmit {
        conversation_id,
        expected_version,
        content: content_tokens.join(" "),
        instance_id,
        instance_view,
    }))
}

fn parse_pending_run_intent_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote run-intents list")?;
    let mut limit = 25;
    let mut limit_seen = false;
    let mut before_submitted_at_ms = None;
    let mut before_intent_id = None;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--limit" if !limit_seen => {
                limit = parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 25, "--limit")?;
                limit_seen = true;
            }
            "--before-submitted-at-ms" if before_submitted_at_ms.is_none() => {
                before_submitted_at_ms = Some(parse_safe_integer_u64(
                    &next_value(tokens, "--before-submitted-at-ms")?,
                    "--before-submitted-at-ms",
                )?);
            }
            "--before-intent-id" if before_intent_id.is_none() => {
                before_intent_id = Some(next_value(tokens, "--before-intent-id")?);
            }
            "--instance" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote run-intents list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if before_submitted_at_ms.is_some() != before_intent_id.is_some() {
        return Err(format!(
            "remote run-intents list cursor requires both --before-submitted-at-ms and --before-intent-id\n\n{}",
            usage()
        ));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote run-intents list --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::PendingRunIntentsList {
        conversation_id,
        limit,
        before_submitted_at_ms,
        before_intent_id,
        instance_id,
        instance_view,
    }))
}

fn parse_pending_run_intent_timeline(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote run-intents timeline")?;
    let intent_id = next_value(tokens, "remote run-intents timeline")?;
    let mut after_sequence = 0;
    let mut limit = 25;
    let mut after_sequence_seen = false;
    let mut limit_seen = false;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--after-sequence" if !after_sequence_seen => {
                after_sequence = parse_safe_integer_u64(
                    &next_value(tokens, "--after-sequence")?,
                    "--after-sequence",
                )?;
                after_sequence_seen = true;
            }
            "--limit" if !limit_seen => {
                limit = parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 25, "--limit")?;
                limit_seen = true;
            }
            "--instance" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote run-intents timeline option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote run-intents timeline --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::PendingRunIntentTimeline {
        conversation_id,
        intent_id,
        after_sequence,
        limit,
        instance_id,
        instance_view,
    }))
}

fn parse_session_observation(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("preview") => {
            let mut input = None;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--input" if input.is_none() => {
                        input = Some(next_value(tokens, "--input")?);
                    }
                    "--input" => return Err("--input was specified more than once".into()),
                    value => {
                        return Err(format!(
                            "invalid remote session observation preview option '{value}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            let input = input.ok_or_else(|| {
                format!(
                    "remote session observation preview requires --input FILE|-\n\n{}",
                    usage()
                )
            })?;
            if input.trim().is_empty() {
                return Err("--input requires a non-empty FILE|- value".into());
            }
            Ok(Command::Remote(RemoteCommand::SessionObservationPreview {
                input,
            }))
        }
        Some(value) => Err(format!(
            "unknown remote session observation command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote session observation command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_placement(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("preview") => {
            let mut input = None;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--input" if input.is_none() => {
                        input = Some(next_value(tokens, "--input")?);
                    }
                    "--input" => return Err("--input was specified more than once".into()),
                    value => {
                        return Err(format!(
                            "invalid remote placement preview option '{value}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            let input = input.ok_or_else(|| {
                format!(
                    "remote placement preview requires --input FILE|-\n\n{}",
                    usage()
                )
            })?;
            if input.trim().is_empty() {
                return Err("--input requires a non-empty FILE|- value".into());
            }
            Ok(Command::Remote(RemoteCommand::PlacementPreview { input }))
        }
        Some("registry-preview") => {
            let mut input = None;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--input" if input.is_none() => {
                        input = Some(next_value(tokens, "--input")?);
                    }
                    "--input" => return Err("--input was specified more than once".into()),
                    value => {
                        return Err(format!(
                            "invalid remote registry placement preview option '{value}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            let input = input.ok_or_else(|| {
                format!(
                    "remote registry placement preview requires --input FILE|-\n\n{}",
                    usage()
                )
            })?;
            if input.trim().is_empty() {
                return Err("--input requires a non-empty FILE|- value".into());
            }
            Ok(Command::Remote(RemoteCommand::PlacementRegistryPreview {
                input,
            }))
        }
        Some(value) => Err(format!(
            "unknown remote placement command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote placement command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_credential_candidate(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("preview") => {
            let mut input = None;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--input" if input.is_none() => {
                        input = Some(next_value(tokens, "--input")?);
                    }
                    "--input" => return Err("--input was specified more than once".into()),
                    value => {
                        return Err(format!(
                            "invalid remote credential-candidate preview option '{value}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            let input = input.ok_or_else(|| {
                format!(
                    "remote credential-candidate preview requires --input FILE|-\n\n{}",
                    usage()
                )
            })?;
            if input.trim().is_empty() {
                return Err("--input requires a non-empty FILE|- value".into());
            }
            Ok(Command::Remote(RemoteCommand::CredentialCandidatePreview {
                input,
            }))
        }
        Some(value) => Err(format!(
            "unknown remote credential-candidate command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote credential-candidate command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_runs(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_run_list(tokens),
        Some("observed") => parse_run_observed(tokens),
        Some("timeline") => parse_run_timeline(tokens),
        Some(value) => Err(format!(
            "unknown remote runs command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("remote runs command is required\n\n{}", usage())),
    }
}

fn parse_run_observed(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote runs observed")?;
    let run_id = next_value(tokens, "remote runs observed")?;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--instance" | "--instance-id" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote runs observed option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote runs observed --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::RunObserved {
        conversation_id,
        run_id,
        instance_id,
        instance_view,
    }))
}

fn parse_run_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote runs list")?;
    let mut limit = 25;
    let mut limit_seen = false;
    let mut before_created_at_ms = None;
    let mut before_run_id = None;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--limit" if !limit_seen => {
                limit = parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 25, "--limit")?;
                limit_seen = true;
            }
            "--before-created-at-ms" if before_created_at_ms.is_none() => {
                before_created_at_ms = Some(parse_safe_integer_u64(
                    &next_value(tokens, "--before-created-at-ms")?,
                    "--before-created-at-ms",
                )?);
            }
            "--before-run-id" if before_run_id.is_none() => {
                before_run_id = Some(next_value(tokens, "--before-run-id")?);
            }
            "--instance" | "--instance-id" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote runs list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if before_created_at_ms.is_some() != before_run_id.is_some() {
        return Err(format!(
            "remote runs list cursor requires both --before-created-at-ms and --before-run-id\n\n{}",
            usage()
        ));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote runs list --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::RunsList {
        conversation_id,
        limit,
        before_created_at_ms,
        before_run_id,
        instance_id,
        instance_view,
    }))
}

fn parse_run_timeline(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote runs timeline")?;
    let run_id = next_value(tokens, "remote runs timeline")?;
    let mut after_sequence = 0;
    let mut limit = 128;
    let mut after_sequence_seen = false;
    let mut limit_seen = false;
    let mut resume = false;
    let mut resume_seen = false;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--after-sequence" if !after_sequence_seen => {
                after_sequence = parse_safe_integer_u64(
                    &next_value(tokens, "--after-sequence")?,
                    "--after-sequence",
                )?;
                after_sequence_seen = true;
            }
            "--limit" if !limit_seen => {
                limit = parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 128, "--limit")?;
                limit_seen = true;
            }
            "--resume" if !resume_seen => {
                resume = true;
                resume_seen = true;
            }
            "--instance" | "--instance-id" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote runs timeline option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if resume && after_sequence_seen {
        return Err(format!(
            "remote runs timeline cannot combine --resume with --after-sequence\n\n{}",
            usage()
        ));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote runs timeline --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::RunTimeline {
        conversation_id,
        run_id,
        after_sequence,
        limit,
        resume,
        instance_id,
        instance_view,
    }))
}

fn parse_safe_integer_u64(value: &str, option: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|parsed| *parsed <= 9_007_199_254_740_991)
        .ok_or_else(|| format!("invalid {option} '{value}'\n\n{}", usage()))
}

fn parse_bounded_u64(value: &str, min: u64, max: u64, option: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|parsed| (*parsed >= min) && (*parsed <= max))
        .ok_or_else(|| format!("{option} must be between {min} and {max}\n\n{}", usage()))
}

fn parse_bounded_usize(value: &str, min: u64, max: u64, option: &str) -> Result<usize, String> {
    usize::try_from(parse_bounded_u64(value, min, max, option)?).map_err(|_| {
        format!(
            "{option} exceeds this platform's integer range\n\n{}",
            usage()
        )
    })
}

fn parse_changes(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => {
            let after_cursor = if tokens
                .front()
                .is_some_and(|value| value == "--after-cursor")
            {
                tokens.pop_front();
                let raw = next_value(tokens, "--after-cursor")?;
                Some(
                    raw.parse::<u64>()
                        .ok()
                        .filter(|value| i64::try_from(*value).is_ok())
                        .ok_or_else(|| format!("invalid --after-cursor '{raw}'\n\n{}", usage()))?,
                )
            } else {
                None
            };
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::ChangesList { after_cursor }))
        }
        Some("watch") => {
            let mut after_cursor = None;
            let mut polls = 8;
            let mut min_delay_ms = 250;
            let mut max_delay_ms = 5_000;
            let mut after_cursor_seen = false;
            let mut polls_seen = false;
            let mut min_delay_seen = false;
            let mut max_delay_seen = false;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--after-cursor" if !after_cursor_seen => {
                        after_cursor = Some(parse_safe_integer_u64(
                            &next_value(tokens, "--after-cursor")?,
                            "--after-cursor",
                        )?);
                        after_cursor_seen = true;
                    }
                    "--polls" if !polls_seen => {
                        polls =
                            parse_bounded_usize(&next_value(tokens, "--polls")?, 1, 64, "--polls")?;
                        polls_seen = true;
                    }
                    "--min-delay-ms" if !min_delay_seen => {
                        min_delay_ms = parse_bounded_u64(
                            &next_value(tokens, "--min-delay-ms")?,
                            0,
                            10_000,
                            "--min-delay-ms",
                        )?;
                        min_delay_seen = true;
                    }
                    "--max-delay-ms" if !max_delay_seen => {
                        max_delay_ms = parse_bounded_u64(
                            &next_value(tokens, "--max-delay-ms")?,
                            0,
                            60_000,
                            "--max-delay-ms",
                        )?;
                        max_delay_seen = true;
                    }
                    _ => {
                        return Err(format!(
                            "invalid remote changes watch option '{option}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            require_empty(tokens)?;
            if max_delay_ms < min_delay_ms {
                return Err(format!(
                    "--max-delay-ms must be greater than or equal to --min-delay-ms\n\n{}",
                    usage()
                ));
            }
            Ok(Command::Remote(RemoteCommand::ChangesWatch {
                after_cursor,
                polls,
                min_delay_ms,
                max_delay_ms,
            }))
        }
        Some(value) => Err(format!(
            "unknown remote changes command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("remote changes command is required\n\n{}", usage())),
    }
}

fn parse_sessions(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_session_list(tokens),
        Some("show") => parse_session_show(tokens),
        Some("create") => parse_session_create(tokens),
        Some("import") => parse_session_import(tokens),
        Some(value) => Err(format!(
            "unknown remote sessions command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote sessions command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_session_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote sessions show")?;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--instance" | "--instance-id" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote sessions show option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote sessions show --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::SessionsShow {
        conversation_id,
        instance_id,
        instance_view,
    }))
}

fn parse_session_import(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote sessions import")?;
    let mut confirm = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--confirm" if confirm.is_none() => {
                let digest = next_value(tokens, "--confirm")?;
                if digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(format!(
                        "--confirm must be a 64-character lowercase SHA-256 digest\n\n{}",
                        usage()
                    ));
                }
                confirm = Some(digest);
            }
            _ => {
                return Err(format!(
                    "invalid remote sessions import option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    Ok(Command::Remote(RemoteCommand::SessionsImport {
        conversation_id,
        confirm,
    }))
}

fn parse_session_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut after_id = None;
    let mut scope = None;
    let mut instance_id = None;
    let mut instance_view = None;
    let mut all_pages = false;
    let mut all_pages_seen = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--after" if after_id.is_none() => after_id = Some(next_value(tokens, "--after")?),
            "--scope" if scope.is_none() => {
                scope = Some(parse_scope(&next_value(tokens, "--scope")?)?);
            }
            "--instance" | "--instance-id" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            "--all" if !all_pages_seen => {
                all_pages = true;
                all_pages_seen = true;
            }
            _ => {
                return Err(format!(
                    "invalid remote sessions list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote sessions list --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::SessionsList {
        after_id,
        scope,
        instance_id,
        instance_view,
        all_pages,
    }))
}

fn parse_session_create(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut title = None;
    let mut scope = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--title" if title.is_none() => title = Some(next_value(tokens, "--title")?),
            "--scope" if scope.is_none() => {
                scope = Some(parse_scope(&next_value(tokens, "--scope")?)?);
            }
            _ => {
                return Err(format!(
                    "invalid remote sessions create option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    Ok(Command::Remote(RemoteCommand::SessionsCreate {
        title: title.unwrap_or_else(|| "New conversation".to_owned()),
        scope: scope.unwrap_or(RemoteConversationScope::Global),
    }))
}

pub(crate) fn parse_scope(value: &str) -> Result<RemoteConversationScope, String> {
    if value == "global" {
        return Ok(RemoteConversationScope::Global);
    }
    let (kind, id) = value.split_once(':').ok_or_else(|| {
        format!(
            "scope must be global, project:ID, or group:ID\n\n{}",
            usage()
        )
    })?;
    if id.trim().is_empty()
        || id.len() > 128
        || id.chars().any(char::is_control)
        || id.contains('/')
    {
        return Err(format!(
            "remote conversation scope id is invalid\n\n{}",
            usage()
        ));
    }
    match kind {
        "project" => Ok(RemoteConversationScope::Project(id.to_owned())),
        "group" => Ok(RemoteConversationScope::Group(id.to_owned())),
        _ => Err(format!(
            "scope must be global, project:ID, or group:ID\n\n{}",
            usage()
        )),
    }
}

#[cfg(test)]
#[path = "remote_args_tests.rs"]
mod tests;
