use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

use super::RemoteCommand;

/// Parses the authenticated, read-only execution-consent preview candidate.
///
/// The command accepts one owner-visible Conversation path segment and an
/// optional local client-instance display projection. It has no grant, expiry,
/// idempotency, or execution arguments by design.
pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("preview") => {
            let conversation_id = next_value(tokens, "remote execution-consent preview")?;
            let mut instance_id = None;
            let mut instance_view = None;
            while let Some(option) = tokens.pop_front() {
                match option.as_str() {
                    "--instance" if instance_id.is_none() => {
                        let value = next_value(tokens, "--instance")?;
                        crate::client_instance_session_scope::validate_instance_id(&value)
                            .map_err(|error| {
                                format!("invalid --instance '{value}': {error}\n\n{}", usage())
                            })?;
                        instance_id = Some(value);
                    }
                    "--instance-view" if instance_view.is_none() => {
                        instance_view = Some(next_value(tokens, "--instance-view")?);
                    }
                    _ => {
                        return Err(format!(
                            "invalid remote execution-consent preview option '{option}'\n\n{}",
                            usage()
                        ));
                    }
                }
            }
            require_empty(tokens)?;
            if instance_view.is_some() && instance_id.is_none() {
                return Err(format!(
                    "remote execution-consent preview --instance-view requires --instance\n\n{}",
                    usage()
                ));
            }
            Ok(Command::Remote(RemoteCommand::ExecutionConsentPreview {
                conversation_id,
                instance_id,
                instance_view,
            }))
        }
        Some(value) => Err(format!(
            "unknown remote execution-consent command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote execution-consent command is required\n\n{}",
            usage()
        )),
    }
}
