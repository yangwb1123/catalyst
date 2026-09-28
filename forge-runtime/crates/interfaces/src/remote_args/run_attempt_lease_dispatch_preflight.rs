use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::RemoteCommand;

/// Parses the authenticated, test-only Run/Attempt/lease preflight candidate.
pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            "--instance" if instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                instance_id = Some(value);
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
                    "invalid remote Run/Attempt/lease preflight option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = required_input(input)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote Run/Attempt/lease preflight --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(
        RemoteCommand::RunAttemptLeaseDispatchPreflightPreview {
            input,
            instance_id,
            instance_view,
        },
    ))
}

fn required_input(input: Option<String>) -> Result<String, String> {
    let input = input.ok_or_else(|| {
        format!(
            "remote Run/Attempt/lease preflight requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(input)
}
