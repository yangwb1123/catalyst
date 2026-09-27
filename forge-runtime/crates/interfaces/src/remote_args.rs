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
#[path = "remote_args/run_execution_evidence.rs"]
mod run_execution_evidence;
#[path = "remote_args/runner_admission.rs"]
mod runner_admission;
#[path = "remote_args/runner_attempt_boundary.rs"]
mod runner_attempt_boundary;
#[path = "remote_args/runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_args/runner_execution_boundary.rs"]
mod runner_execution_boundary;
#[path = "remote_args/runner_execution_intent.rs"]
mod runner_execution_intent;
#[path = "remote_args/session_runner_receipt.rs"]
mod session_runner_receipt;
#[path = "remote_args/session_runner_receipt_history.rs"]
mod session_runner_receipt_history;
#[path = "remote_args/session_runner_reconciliation.rs"]
mod session_runner_reconciliation;

#[path = "remote_args/changes.rs"]
mod changes;
#[path = "remote_args/command.rs"]
mod command;
#[path = "remote_args/pending_run_intents.rs"]
mod pending_run_intents;
#[path = "remote_args/placement.rs"]
mod placement;
#[path = "remote_args/runs.rs"]
mod runs;
#[path = "remote_args/sessions.rs"]
mod sessions;

use changes::parse_changes;
pub use command::{PromptPageCursor, RemoteCommand, RemoteConversationScope};
use pending_run_intents::parse_pending_run_intents;
use placement::parse_placement;
use runs::parse_runs;
pub(crate) use sessions::parse_scope;
use sessions::parse_sessions;

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
        Some("session-runner-receipt-history") => session_runner_receipt_history::parse(tokens),
        Some("session-runner-reconciliation") => session_runner_reconciliation::parse(tokens),
        Some("run-execution-evidence") => run_execution_evidence::parse(tokens),
        Some("runner") => local_runner_preview::parse(tokens),
        Some("run-attempt-lease-dispatch-preflight-preview") => {
            run_attempt_lease_dispatch_preflight::parse(tokens)
        }
        Some("runner-dispatch-plan-preview") => runner_dispatch_plan_preview::parse(tokens),
        Some("runner-execution-intent-preview") => runner_execution_intent::parse(tokens),
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
        Some("show-converged") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::InventoryShowConverged))
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
        Some("show-converged") => {
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::ClientInstancesShowConverged))
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

fn parse_instance_id(tokens: &mut VecDeque<String>) -> Result<String, String> {
    let value = next_value(tokens, "--instance")?;
    crate::client_instance_session_scope::validate_instance_id(&value)
        .map_err(|error| format!("invalid --instance '{value}': {error}\n\n{}", usage()))?;
    Ok(value)
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

#[cfg(test)]
#[path = "remote_args_tests.rs"]
mod tests;
