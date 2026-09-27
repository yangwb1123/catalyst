use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::RemoteCommand;

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("preview") => {
            let mut input = None;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
                    "--input" => return Err("--input was specified more than once".into()),
                    value => {
                        return Err(format!(
                            "invalid remote Run execution evidence option '{value}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            let input = input.ok_or_else(|| {
                format!(
                    "remote Run execution evidence preview requires --input FILE|-\n\n{}",
                    usage()
                )
            })?;
            if input.trim().is_empty() {
                return Err("--input requires a non-empty FILE|- value".into());
            }
            Ok(Command::Remote(
                RemoteCommand::RunExecutionEvidencePreview { input },
            ))
        }
        Some(value) => Err(format!(
            "unknown remote Run execution evidence command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote Run execution evidence command is required\n\n{}",
            usage()
        )),
    }
}
