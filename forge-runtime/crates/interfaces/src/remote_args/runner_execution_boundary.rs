use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::RemoteCommand;

/// Parses the authenticated Runner execution-boundary preview.
pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let (input, instance_id, instance_view) = parse_options(tokens)?;
    let input = input.ok_or_else(|| {
        format!(
            "remote Runner execution boundary requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote Runner execution boundary --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(
        RemoteCommand::RunnerExecutionBoundaryPreview {
            input,
            instance_id,
            instance_view,
        },
    ))
}

type InputOptions = (Option<String>, Option<String>, Option<String>);

fn parse_options(tokens: &mut VecDeque<String>) -> Result<InputOptions, String> {
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
                    "invalid remote Runner execution boundary option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    Ok((input, instance_id, instance_view))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_remote(tokens: &[&str]) -> Result<Command, String> {
        crate::args::parse_tokens(tokens.iter().map(|token| (*token).to_owned()))
            .map(|args| args.command)
    }

    #[test]
    fn accepts_instance_projection_and_rejects_partial_options() {
        assert_eq!(
            parse_remote(&[
                "remote",
                "placement",
                "runner-execution-boundary-preview",
                "--input",
                "boundary.json",
                "--instance",
                "client-cli-001",
                "--instance-view",
                "view.json",
            ])
            .unwrap(),
            Command::Remote(RemoteCommand::RunnerExecutionBoundaryPreview {
                input: "boundary.json".into(),
                instance_id: Some("client-cli-001".into()),
                instance_view: Some("view.json".into()),
            })
        );
        assert!(
            parse_remote(&[
                "remote",
                "placement",
                "runner-execution-boundary-preview",
                "--input",
                "boundary.json",
                "--instance-view",
                "view.json",
            ])
            .is_err()
        );
    }
}
