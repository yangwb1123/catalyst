//! Strict display-only consumer for the authenticated execution-consent
//! preview candidate.
//!
//! The candidate exposes the server-resolved project and execution profile so
//! a client can show the exact digest before a future consent flow.  Reading
//! this projection never grants consent, creates a Run, selects a device, or
//! contacts a Runner.

use std::io::Write;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{RemoteError, validation};

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_EXECUTION_CONSENT_TTL_MS: u64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionConsentPreview {
    pub(super) conversation_id: String,
    pub(super) project_id: String,
    pub(super) profile_id: String,
    pub(super) profile_sha256: String,
    pub(super) maximum_ttl_ms: u64,
}

pub(super) fn validate_response(
    value: &Value,
    conversation_id: &str,
) -> Result<ExecutionConsentPreview, RemoteError> {
    validation::validate_conversation_id(conversation_id)?;
    let response: ExecutionConsentPreview =
        serde_json::from_value(value.clone()).map_err(|_| {
            RemoteError("Forge API returned an invalid execution-consent preview".into())
        })?;
    validation::validate_conversation_id(&response.conversation_id).map_err(|_| {
        RemoteError("Forge API returned an invalid execution-consent preview".into())
    })?;
    validation::validate_entity_id(&response.project_id, "project").map_err(|_| {
        RemoteError("Forge API returned an invalid execution-consent preview".into())
    })?;
    validation::validate_entity_id(&response.profile_id, "execution profile").map_err(|_| {
        RemoteError("Forge API returned an invalid execution-consent preview".into())
    })?;
    if response.conversation_id != conversation_id
        || !is_lowercase_sha256(&response.profile_sha256)
        || response.maximum_ttl_ms == 0
        || response.maximum_ttl_ms > MAX_EXECUTION_CONSENT_TTL_MS
        || response.maximum_ttl_ms > MAX_SAFE_INTEGER
    {
        return Err(RemoteError(
            "Forge API returned an invalid execution-consent preview".into(),
        ));
    }
    Ok(response)
}

pub(super) fn render_human(
    value: &Value,
    conversation_id: &str,
    writer: &mut impl Write,
) -> Result<(), RemoteError> {
    let response = validate_response(value, conversation_id)?;
    writeln!(writer, "execution-consent preview [read-only candidate]")
        .map_err(|error| io_error(&error))?;
    writeln!(
        writer,
        "conversation={} project={} profile={} profile_sha256={} maximum_ttl_ms={}",
        response.conversation_id,
        response.project_id,
        response.profile_id,
        response.profile_sha256,
        response.maximum_ttl_ms
    )
    .map_err(|error| io_error(&error))?;
    writeln!(
        writer,
        "No consent was granted; no Run, device selection, lease, reservation, dispatch, or Runner request was created."
    )
    .map_err(|error| io_error(&error))
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn io_error(error: &std::io::Error) -> RemoteError {
    RemoteError(format!(
        "could not render execution-consent preview: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{MAX_EXECUTION_CONSENT_TTL_MS, render_human, validate_response};

    fn response() -> Value {
        json!({
            "conversation_id": "conversation-1",
            "project_id": "project-1",
            "profile_id": "profile-reviewed-v1",
            "profile_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "maximum_ttl_ms": MAX_EXECUTION_CONSENT_TTL_MS
        })
    }

    #[test]
    fn response_is_strict_and_bound_to_conversation() {
        let value = response();
        validate_response(&value, "conversation-1").expect("valid preview");
        assert!(validate_response(&value, "conversation-2").is_err());

        let mut unknown = value.clone();
        unknown["run_id"] = json!("must-not-be-present");
        assert!(validate_response(&unknown, "conversation-1").is_err());

        let mut digest = value.clone();
        digest["profile_sha256"] =
            json!("0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef");
        assert!(validate_response(&digest, "conversation-1").is_err());

        let mut ttl = value.clone();
        ttl["maximum_ttl_ms"] = json!(0);
        assert!(validate_response(&ttl, "conversation-1").is_err());
    }

    #[test]
    fn human_render_is_metadata_only() {
        let mut output = Vec::new();
        render_human(&response(), "conversation-1", &mut output).expect("render");
        let output = String::from_utf8(output).expect("UTF-8");
        assert!(output.contains("profile_sha256=0123456789abcdef"));
        assert!(output.contains("No consent was granted"));
        assert!(!output.contains("grant_id"));
        assert!(!output.contains("fencing_token"));
    }
}
