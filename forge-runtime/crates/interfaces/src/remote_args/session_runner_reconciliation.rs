use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::RemoteCommand;

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let subcommand = tokens.pop_front();
    match subcommand.as_deref() {
        Some("preview") | Some("remote-preview") => {
            let remote = subcommand.as_deref() == Some("remote-preview");
            let mut input = None;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
                    "--input" => return Err("--input was specified more than once".into()),
                    value => {
                        return Err(format!(
                            "invalid remote session Runner reconciliation preview option '{value}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            let input = input.ok_or_else(|| {
                format!(
                    "remote session Runner reconciliation preview requires --input FILE|-\n\n{}",
                    usage()
                )
            })?;
            if input.trim().is_empty() {
                return Err("--input requires a non-empty FILE|- value".into());
            }
            let command = if remote {
                RemoteCommand::SessionRunnerReconciliationRemotePreview { input }
            } else {
                RemoteCommand::SessionRunnerReconciliationPreview { input }
            };
            Ok(Command::Remote(command))
        }
        Some(value) => Err(format!(
            "unknown remote session Runner reconciliation command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote session Runner reconciliation command is required\n\n{}",
            usage()
        )),
    }
}

#[cfg(test)]
#[path = "session_runner_reconciliation_tests.rs"]
mod tests;
