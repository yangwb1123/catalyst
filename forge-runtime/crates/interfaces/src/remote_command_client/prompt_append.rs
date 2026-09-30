use super::{
    MAX_SAFE_JSON_INTEGER, RemoteClient, RemoteError, Value, json, prompt_append_receipt,
    validate_conversation_id,
};

impl RemoteClient {
    pub(in crate::remote_command) async fn append_prompt(
        &self,
        conversation_id: &str,
        expected_version: u64,
        content: &str,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        if content.trim().is_empty()
            || content.len() > 256 * 1024
            || expected_version == 0
            || expected_version > MAX_SAFE_JSON_INTEGER
        {
            return Err(RemoteError("prompt content is invalid".into()));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.conversation_prompts_url(conversation_id)?)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({
                        "content": content,
                        "expected_version": expected_version,
                    })),
            )
            .await?;
        prompt_append_receipt::validate_append_response_value(
            &response,
            conversation_id,
            expected_version,
            content,
        )?;
        validate_prompt_version_and_replay(&response, expected_version)?;
        Ok(response)
    }
}

fn validate_prompt_version_and_replay(
    response: &Value,
    expected_version: u64,
) -> Result<(), RemoteError> {
    let aggregate_version = response
        .get("aggregate_version")
        .and_then(Value::as_u64)
        .filter(|version| *version > 0 && *version <= MAX_SAFE_JSON_INTEGER)
        .ok_or_else(|| RemoteError("Forge API returned an invalid Prompt receipt".into()))?;
    // The Hub's owner-scoped Prompt write is a single aggregate-version
    // CAS. A successful new write or an idempotent replay therefore must
    // return exactly the next version requested by this client. Reject a
    // stale or skipped receipt before the TUI/CLI can advance local state.
    let expected_next_version = expected_version
        .checked_add(1)
        .filter(|version| *version <= MAX_SAFE_JSON_INTEGER)
        .ok_or_else(|| RemoteError("Forge API returned an invalid Prompt receipt".into()))?;
    if aggregate_version != expected_next_version {
        return Err(RemoteError(
            "Forge API returned an invalid Prompt receipt".into(),
        ));
    }
    // Keep the replay marker part of the authenticated write contract. A
    // missing or non-boolean marker would make a retry indistinguishable
    // from a new append at the CLI/TUI boundary, even though the Hub's
    // idempotency result is otherwise valid.
    if response.get("replayed").and_then(Value::as_bool).is_none() {
        return Err(RemoteError(
            "Forge API returned an invalid Prompt receipt".into(),
        ));
    }
    Ok(())
}
