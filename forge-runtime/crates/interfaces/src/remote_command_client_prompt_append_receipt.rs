use forge_runtime_domain::{ConversationPrompt, execution::prompt_append_receipt};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{MAX_SAFE_JSON_INTEGER, RemoteClient, RemoteError};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AppendResponse {
    prompt: ConversationPromptResponse,
    aggregate_version: u64,
    replayed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConversationPromptResponse {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    created_at_ms: u64,
}

impl From<ConversationPromptResponse> for ConversationPrompt {
    fn from(prompt: ConversationPromptResponse) -> Self {
        Self {
            id: prompt.id,
            conversation_id: prompt.conversation_id,
            role: prompt.role,
            content: prompt.content,
            created_at_ms: prompt.created_at_ms,
        }
    }
}

impl RemoteClient {
    /// Appends one Prompt through the existing authenticated owner/CAS route,
    /// then reduces its response to the content-free compatibility receipt.
    /// The POST remains single-shot; callers may explicitly retry with the
    /// same idempotency key when they can distinguish an uncertain write.
    pub(in crate::remote_command) async fn append_prompt_receipt(
        &self,
        conversation_id: &str,
        expected_version: u64,
        content: &str,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        let owner = self.authenticated_owner().await?;
        let response = self
            .append_prompt(conversation_id, expected_version, content, idempotency_key)
            .await?;
        let response: AppendResponse = serde_json::from_value(response).map_err(|_| {
            RemoteError("Forge API returned an invalid Prompt append response".into())
        })?;
        validate_append_response(&response, conversation_id, expected_version, content)?;
        let observation =
            prompt_append_receipt::observe(prompt_append_receipt::PromptAppendReceiptInput {
                owner,
                conversation_id: conversation_id.to_owned(),
                expected_version,
                role: "user".into(),
                content: content.to_owned(),
                idempotency_key: idempotency_key.to_owned(),
                prompt_id: response.prompt.id.clone(),
                created_at_ms: response.prompt.created_at_ms,
                replayed: response.replayed,
            })
            .map_err(|_| {
                RemoteError("Forge API returned an invalid Prompt append receipt".into())
            })?;
        if response.aggregate_version != observation.receipt.aggregate_version {
            return Err(RemoteError(
                "Forge API returned an invalid Prompt append receipt".into(),
            ));
        }
        serde_json::to_value(observation)
            .map_err(|_| RemoteError("Prompt append receipt could not be encoded".into()))
    }
}

/// Validates the full response returned by the ordinary Prompt append path.
/// The storage-only `prompts add` command returns this response directly, so
/// it must enforce the same Conversation, role, content, identity, timestamp,
/// CAS, replay marker, and closed-field contract as the content-free receipt
/// projection.
pub(super) fn validate_append_response_value(
    response: &Value,
    conversation_id: &str,
    expected_version: u64,
    content: &str,
) -> Result<(), RemoteError> {
    let response: AppendResponse =
        serde_json::from_value(response.clone()).map_err(|_| invalid_prompt_receipt())?;
    validate_append_response(&response, conversation_id, expected_version, content)
        .map_err(|_| invalid_prompt_receipt())
}

fn invalid_prompt_receipt() -> RemoteError {
    RemoteError("Forge API returned an invalid Prompt receipt".into())
}

/// Binds the typed response body back to the exact authenticated append
/// request before either the ordinary response or the content-free receipt
/// projection is returned. Prompt identity and timestamp are checked here as
/// bounded metadata; the canonical domain projection repeats the same
/// invariants before serialization.
fn validate_append_response(
    response: &AppendResponse,
    conversation_id: &str,
    expected_version: u64,
    content: &str,
) -> Result<(), RemoteError> {
    let expected_next_version = expected_version
        .checked_add(1)
        .filter(|version| *version <= MAX_SAFE_JSON_INTEGER)
        .ok_or_else(invalid_append_response)?;
    let prompt = &response.prompt;
    if prompt.conversation_id != conversation_id
        || prompt.role != "user"
        || prompt.content != content
        || prompt.id.trim().is_empty()
        || prompt.id.len() > 128
        || prompt.id.chars().any(char::is_control)
        || prompt.created_at_ms > MAX_SAFE_JSON_INTEGER
        || response.aggregate_version != expected_next_version
    {
        return Err(invalid_append_response());
    }
    Ok(())
}

fn invalid_append_response() -> RemoteError {
    RemoteError("Forge API returned an invalid Prompt append receipt".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_runtime_domain::execution::runner_execution_intent::RunnerExecutionOwner;

    fn response() -> AppendResponse {
        AppendResponse {
            prompt: ConversationPromptResponse {
                id: "prompt-1".into(),
                conversation_id: "conversation-1".into(),
                role: "user".into(),
                content: "send this".into(),
                created_at_ms: 300,
            },
            aggregate_version: 3,
            replayed: false,
        }
    }

    #[test]
    fn append_response_requires_exact_request_binding() {
        let mut value = response();
        assert!(validate_append_response(&value, "conversation-1", 2, "send this").is_ok());

        value.prompt.conversation_id = "conversation-foreign".into();
        assert!(validate_append_response(&value, "conversation-1", 2, "send this").is_err());

        let mut value = response();
        value.prompt.role = "assistant".into();
        assert!(validate_append_response(&value, "conversation-1", 2, "send this").is_err());

        let mut value = response();
        value.prompt.content = "foreign content".into();
        assert!(validate_append_response(&value, "conversation-1", 2, "send this").is_err());
    }

    #[test]
    fn ordinary_append_response_rejects_unknown_fields() {
        let mut value = serde_json::to_value(response()).unwrap();
        value["unexpected"] = Value::String("drift".into());
        assert!(validate_append_response_value(&value, "conversation-1", 2, "send this").is_err());

        let mut value = serde_json::to_value(response()).unwrap();
        value["prompt"]["unexpected"] = Value::String("drift".into());
        assert!(validate_append_response_value(&value, "conversation-1", 2, "send this").is_err());
    }

    #[test]
    fn append_response_requires_bounded_prompt_metadata_and_cas() {
        let mut value = response();
        value.prompt.id.clear();
        assert!(validate_append_response(&value, "conversation-1", 2, "send this").is_err());

        let mut value = response();
        value.prompt.created_at_ms = 9_007_199_254_740_992;
        assert!(validate_append_response(&value, "conversation-1", 2, "send this").is_err());

        let mut value = response();
        value.aggregate_version = 4;
        assert!(validate_append_response(&value, "conversation-1", 2, "send this").is_err());

        assert!(validate_append_response(&response(), "conversation-1", 0, "send this").is_err());
        assert!(
            validate_append_response(
                &response(),
                "conversation-1",
                9_007_199_254_740_991,
                "send this",
            )
            .is_err()
        );
    }

    #[test]
    fn owner_is_projected_from_the_authenticated_token_only() {
        let owner = RunnerExecutionOwner {
            issuer: "https://id.example".into(),
            subject: "user-1".into(),
            tenant_id: "tenant-1".into(),
        };
        let observation =
            prompt_append_receipt::observe(prompt_append_receipt::PromptAppendReceiptInput {
                owner: owner.clone(),
                conversation_id: "conversation-1".into(),
                expected_version: 2,
                role: "user".into(),
                content: "send this".into(),
                idempotency_key: "prompt-key".into(),
                prompt_id: "prompt-1".into(),
                created_at_ms: 300,
                replayed: false,
            })
            .unwrap();
        assert_eq!(observation.owner, owner);
        assert!(
            !serde_json::to_string(&observation)
                .unwrap()
                .contains("send this")
        );
    }
}
