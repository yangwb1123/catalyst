use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::RemoteCommand;

/// Parses the authenticated Runner Attempt boundary preview.
pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let (input, instance_id, instance_view) = parse_options(tokens)?;
    let input = input.ok_or_else(|| {
        format!(
            "remote Runner Attempt boundary requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote Runner Attempt boundary --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(
        RemoteCommand::RunnerAttemptBoundaryPreview {
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
                    "invalid remote Runner Attempt boundary option '{value}'\n\n{}",
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

    fn parse_remote(options: &[&str]) -> Result<Command, String> {
        let mut tokens = vec!["remote", "placement", "runner-attempt-boundary-preview"];
        tokens.extend(options);
        crate::args::parse_tokens(tokens.into_iter().map(str::to_owned)).map(|args| args.command)
    }

    #[test]
    fn parses_explicit_file_or_stdin_without_instance_filter() {
        for input in ["request.json", "-"] {
            assert_eq!(
                parse_remote(&["--input", input]).unwrap(),
                Command::Remote(RemoteCommand::RunnerAttemptBoundaryPreview {
                    input: input.into(),
                    instance_id: None,
                    instance_view: None,
                })
            );
        }
    }

    #[test]
    fn parses_instance_with_online_or_local_projection() {
        for view in [None, Some("view.json"), Some("-")] {
            let mut options = vec!["--instance", "client-cli-001", "--input", "request.json"];
            if let Some(view) = view {
                options.extend(["--instance-view", view]);
            }
            assert_eq!(
                parse_remote(&options).unwrap(),
                Command::Remote(RemoteCommand::RunnerAttemptBoundaryPreview {
                    input: "request.json".into(),
                    instance_id: Some("client-cli-001".into()),
                    instance_view: view.map(str::to_owned),
                })
            );
        }
    }

    #[test]
    fn rejects_missing_duplicate_blank_or_unknown_input() {
        for options in [
            vec![],
            vec!["--input"],
            vec!["--input", ""],
            vec!["--input", " \t "],
            vec!["--input", "one", "--input", "two"],
            vec!["--input", "one", "--unknown"],
        ] {
            assert!(parse_remote(&options).is_err(), "options={options:?}");
        }
    }

    #[test]
    fn rejects_invalid_instance_or_view_without_instance() {
        for options in [
            vec!["--instance"],
            vec!["--instance", ""],
            vec!["--instance", " "],
            vec!["--instance", "invalid/instance"],
            vec!["--instance-view", "view.json"],
            vec!["--instance", "client-cli-001", "--instance-view"],
        ] {
            let mut input = vec!["--input", "request.json"];
            input.extend(&options);
            assert!(parse_remote(&input).is_err(), "options={options:?}");
        }
    }

    #[test]
    fn rejects_duplicate_instance_and_instance_view() {
        for option in ["--instance", "--instance-view"] {
            let mut options = vec!["--input", "request.json", "--instance", "client-cli-001"];
            if option == "--instance-view" {
                options.extend(["--instance-view", "view.json"]);
            }
            options.extend([option, "second"]);
            let error = parse_remote(&options).unwrap_err();
            assert!(error.contains(&format!("{option} was specified more than once")));
        }
    }
}
