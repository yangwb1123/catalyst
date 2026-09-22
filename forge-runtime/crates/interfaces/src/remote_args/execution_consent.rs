use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

use super::RemoteCommand;

/// Parses the authenticated, read-only execution-consent preview candidate.
///
/// The command accepts only the owner-visible Conversation path segment. It
/// has no grant, expiry, idempotency, or execution arguments by design.
pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("preview") => {
            let conversation_id = next_value(tokens, "remote execution-consent preview")?;
            require_empty(tokens)?;
            Ok(Command::Remote(RemoteCommand::ExecutionConsentPreview {
                conversation_id,
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
