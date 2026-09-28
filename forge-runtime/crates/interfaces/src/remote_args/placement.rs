use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::{
    RemoteCommand, parse_instance_id, runner_admission, runner_attempt_boundary,
    runner_execution_boundary,
};

fn parse_scheduler_lease_options(
    tokens: &mut VecDeque<String>,
    label: &str,
    input_allows_stdin: bool,
) -> Result<(String, Option<String>, Option<String>), String> {
    let (input, instance_id, instance_view) = parse_scheduler_input_options(tokens, label)?;
    let input = input.ok_or_else(|| {
        let kind = if input_allows_stdin { "FILE|-" } else { "FILE" };
        format!(
            "remote {label} requires --input {kind} and --idempotency-key\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() || (!input_allows_stdin && input == "-") {
        let kind = if input_allows_stdin {
            "FILE|-"
        } else {
            "regular FILE"
        };
        return Err(format!("--input requires a non-empty {kind} value"));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote {label} --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok((input, instance_id, instance_view))
}

pub(super) fn parse_placement(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("preview") => parse_preview(tokens),
        Some("registry-preview") => parse_registry_preview(tokens),
        Some("scheduler-preview") => parse_scheduler_preview(tokens),
        Some("scheduler-lease") => parse_scheduler_lease(tokens),
        Some("scheduler-lease-renew") => parse_scheduler_lease_renew(tokens),
        Some("scheduler-lease-release") => parse_scheduler_lease_release(tokens),
        Some("runner-dispatch-admission-preview") => {
            runner_admission::parse(tokens, runner_admission::Kind::Dispatch)
        }
        Some("runner-transport-admission-preview") => {
            runner_admission::parse(tokens, runner_admission::Kind::Transport)
        }
        Some("runner-execution-boundary-preview") => runner_execution_boundary::parse(tokens),
        Some("runner-attempt-boundary-preview") => runner_attempt_boundary::parse(tokens),
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

fn parse_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
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

fn parse_registry_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
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

fn parse_scheduler_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => {
                input = Some(next_value(tokens, "--input")?);
            }
            "--input" => return Err("--input was specified more than once".into()),
            "--instance" if instance_id.is_none() => {
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance" => return Err("--instance was specified more than once".into()),
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            "--instance-view" => {
                return Err("--instance-view was specified more than once".into());
            }
            value => {
                return Err(format!(
                    "invalid remote scheduler selection preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "remote scheduler selection preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote scheduler selection preview --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::SchedulerSelectionPreview {
        input,
        instance_id,
        instance_view,
    }))
}

fn parse_scheduler_lease(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let (input, instance_id, instance_view) =
        parse_scheduler_lease_options(tokens, "scheduler lease", true)?;
    Ok(Command::Remote(RemoteCommand::SchedulerSelectionLease {
        input,
        instance_id,
        instance_view,
    }))
}

fn parse_scheduler_lease_renew(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let (input, instance_id, instance_view) =
        parse_scheduler_lease_options(tokens, "scheduler lease renewal", false)?;
    Ok(Command::Remote(
        RemoteCommand::SchedulerSelectionLeaseRenew {
            input,
            instance_id,
            instance_view,
        },
    ))
}

fn parse_scheduler_lease_release(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let (input, instance_id, instance_view) =
        parse_scheduler_lease_options(tokens, "scheduler lease release", false)?;
    Ok(Command::Remote(
        RemoteCommand::SchedulerSelectionLeaseRelease {
            input,
            instance_id,
            instance_view,
        },
    ))
}

type InputOptions = (Option<String>, Option<String>, Option<String>);

fn parse_scheduler_input_options(
    tokens: &mut VecDeque<String>,
    label: &str,
) -> Result<InputOptions, String> {
    let mut input = None;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => {
                input = Some(next_value(tokens, "--input")?);
            }
            "--input" => return Err("--input was specified more than once".into()),
            "--instance" if instance_id.is_none() => {
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance" => return Err("--instance was specified more than once".into()),
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            "--instance-view" => {
                return Err("--instance-view was specified more than once".into());
            }
            value => {
                return Err(format!(
                    "invalid remote {label} option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    Ok((input, instance_id, instance_view))
}
