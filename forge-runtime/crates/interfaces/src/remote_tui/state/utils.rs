use std::io::Write;

use rand::TryRngCore;
use serde_json::Value;

use super::TuiState;

use super::super::super::RemoteError;
use crate::runtime_domain::{
    PENDING_WRITE_RECOVERY_EVALUATION_MODE, PENDING_WRITE_RECOVERY_SCHEMA_VERSION,
    PendingWriteMetadata, project_pending_write,
};

/// Prints the metadata needed to replay a pending write without exposing prompt text.
///
/// # Errors
///
/// Returns an error when the terminal writer fails.
pub(crate) fn write_pending_recovery<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let metadata = if let Some(pending) = &state.pending_prompt {
        PendingWriteMetadata {
            operation: "append_prompt".into(),
            conversation_id: Some(pending.conversation_id.clone()),
            expected_version: Some(pending.expected_version),
            idempotency_key: pending.idempotency_key.clone(),
            state: "unconfirmed".into(),
            attempted_at_ms: None,
            last_observed_at_ms: None,
        }
    } else if let Some(pending) = &state.pending_run_intent {
        PendingWriteMetadata {
            operation: "submit_pending_run_intent".into(),
            conversation_id: Some(pending.conversation_id.clone()),
            expected_version: Some(pending.expected_version),
            idempotency_key: pending.idempotency_key.clone(),
            state: "unconfirmed".into(),
            attempted_at_ms: None,
            last_observed_at_ms: None,
        }
    } else if let Some(pending) = &state.pending_create {
        PendingWriteMetadata {
            operation: "create_conversation".into(),
            conversation_id: None,
            expected_version: None,
            idempotency_key: pending.idempotency_key.clone(),
            state: "unconfirmed".into(),
            attempted_at_ms: None,
            last_observed_at_ms: None,
        }
    } else {
        return Ok(());
    };
    project_pending_write(metadata.clone())
        .map_err(|_| RemoteError("pending write metadata is invalid".into()))?;
    let mut recovery = serde_json::to_value(metadata)
        .map_err(|_| RemoteError("pending write metadata could not be encoded".into()))?;
    let Some(object) = recovery.as_object_mut() else {
        return Err(RemoteError("pending write metadata is invalid".into()));
    };
    object.insert(
        "schema_version".into(),
        Value::String(PENDING_WRITE_RECOVERY_SCHEMA_VERSION.into()),
    );
    object.insert(
        "evaluation_mode".into(),
        Value::String(PENDING_WRITE_RECOVERY_EVALUATION_MODE.into()),
    );
    writeln!(writer, "Pending write recovery: {recovery}").map_err(io_error)
}

/// Creates a fresh, process-local idempotency key for a new TUI write.
///
/// # Errors
///
/// Returns an error when the operating system random source fails.
pub(crate) fn new_idempotency_key() -> Result<String, RemoteError> {
    let mut bytes = [0_u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| RemoteError("could not create an idempotency key".into()))?;
    let mut output = String::from("forge-tui-");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").map_err(io_error)?;
    }
    Ok(output)
}

pub(crate) fn json_text(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

/// Wraps terminal writer failures without leaking content or host details.
pub(crate) fn io_error(_: impl std::fmt::Display) -> RemoteError {
    RemoteError("could not write the terminal session".into())
}
