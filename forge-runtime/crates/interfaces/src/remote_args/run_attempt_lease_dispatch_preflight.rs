use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::RemoteCommand;

/// Parses the authenticated, test-only Run/Attempt/lease preflight candidate.
pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "invalid remote Run/Attempt/lease preflight option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "remote Run/Attempt/lease preflight requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Remote(
        RemoteCommand::RunAttemptLeaseDispatchPreflightPreview { input },
    ))
}
