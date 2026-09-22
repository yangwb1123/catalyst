use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::RemoteCommand;

/// Parses the explicit, test-only local Runner execution-readiness candidate.
/// The input remains a caller-supplied file so the command cannot silently
/// read prompts, leases, or device state from another API.
pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("execution-readiness-preview") => {
            let mut input = None;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--input" if input.is_none() => {
                        input = Some(next_value(tokens, "--input")?);
                    }
                    "--input" => return Err("--input was specified more than once".into()),
                    value => {
                        return Err(format!(
                            "invalid remote local Runner preview option '{value}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            let input = input.ok_or_else(|| {
                format!(
                    "remote local Runner preview requires --input FILE|-\n\n{}",
                    usage()
                )
            })?;
            if input.trim().is_empty() {
                return Err("--input requires a non-empty FILE|- value".into());
            }
            Ok(Command::Remote(RemoteCommand::LocalRunnerPreview { input }))
        }
        Some(value) => Err(format!(
            "unknown remote Runner command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("remote Runner command is required\n\n{}", usage())),
    }
}
