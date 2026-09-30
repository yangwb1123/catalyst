use super::{
    ClientInstanceSessionScope, MAX_SAFE_JSON_INTEGER, MAX_SESSION_LIST_PAGES,
    OwnedConversationChangePage, OwnedConversationEntry, OwnedConversationPage, PAGE_SIZE,
    PromptPageCursor, RemoteClient, RemoteConversationScope, RemoteError, Value,
    conversation_has_scope, json, prompt_page, scope_json, validate_conversation_id,
    validate_conversation_page, validate_entity_id, validate_owned_conversation_entry,
};

impl RemoteClient {
    pub(in crate::remote_command) async fn list_conversations(
        &self,
        after_id: Option<&str>,
    ) -> Result<OwnedConversationPage, RemoteError> {
        if let Some(after_id) = after_id {
            validate_conversation_id(after_id)?;
        }
        let mut request = self
            .http
            .get(self.endpoint("/api/v1/conversations")?)
            .query(&[("limit", PAGE_SIZE)]);
        if let Some(after_id) = after_id {
            request = request.query(&[("after_id", after_id)]);
        }
        let response = self.send_read_json(request).await?;
        let page: OwnedConversationPage = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid conversation page".into()))?;
        validate_conversation_page(&page, after_id)?;
        Ok(page)
    }

    pub(in crate::remote_command) async fn get_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<OwnedConversationEntry, RemoteError> {
        validate_conversation_id(conversation_id)?;
        let response = self
            .send_read_json(self.http.get(self.conversation_url(conversation_id)?))
            .await?;
        let entry: OwnedConversationEntry = serde_json::from_value(response.clone())
            .map_err(|_| RemoteError("Forge API returned an invalid conversation detail".into()))?;
        validate_owned_conversation_entry(&response, &entry)?;
        if entry.conversation.get("id").and_then(Value::as_str) != Some(conversation_id) {
            return Err(RemoteError(
                "Forge API returned another conversation".into(),
            ));
        }
        Ok(entry)
    }

    pub(in crate::remote_command) async fn list_conversations_json(
        &self,
        after_id: Option<&str>,
        scope: Option<&RemoteConversationScope>,
        all_pages: bool,
    ) -> Result<Value, RemoteError> {
        self.list_conversations_json_with_instance(after_id, scope, None, all_pages)
            .await
    }

    pub(in crate::remote_command) async fn list_conversations_json_with_instance(
        &self,
        after_id: Option<&str>,
        scope: Option<&RemoteConversationScope>,
        instance_scope: Option<&ClientInstanceSessionScope>,
        all_pages: bool,
    ) -> Result<Value, RemoteError> {
        let mut cursor = after_id.map(str::to_owned);
        let mut conversations = Vec::new();
        let mut pages_read = 0;
        loop {
            let page = self.list_conversations(cursor.as_deref()).await?;
            pages_read += 1;
            for entry in page.conversations {
                if scope.is_none_or(|scope| conversation_has_scope(&entry.conversation, scope))
                    && instance_scope.is_none_or(|instance| {
                        entry
                            .conversation
                            .get("id")
                            .and_then(Value::as_str)
                            .is_some_and(|id| instance.session_ids.contains(id))
                    })
                {
                    conversations.push(json!({
                        "conversation": entry.conversation,
                        "aggregate_version": entry.aggregate_version,
                    }));
                }
            }

            if !all_pages || !page.has_more || pages_read == MAX_SESSION_LIST_PAGES {
                return Ok(json!({
                    "conversations": conversations,
                    "next_after_id": page.next_after_id,
                    "has_more": page.has_more,
                }));
            }
            cursor = page.next_after_id;
        }
    }

    pub(in crate::remote_command) async fn conversation_changes_after(
        &self,
        after_cursor: u64,
    ) -> Result<OwnedConversationChangePage, RemoteError> {
        if after_cursor > MAX_SAFE_JSON_INTEGER {
            return Err(RemoteError("conversation change cursor is invalid".into()));
        }
        let limit = 128_usize;
        let response = self
            .send_read_json(
                self.http
                    .get(self.endpoint("/api/v1/conversation-changes")?)
                    .query(&[
                        ("after_cursor", after_cursor.to_string()),
                        ("limit", limit.to_string()),
                    ]),
            )
            .await?;
        let page: OwnedConversationChangePage = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid change page".into()))?;
        page.validate(after_cursor, limit).map_err(RemoteError)?;
        Ok(page)
    }

    pub(in crate::remote_command) async fn create_conversation(
        &self,
        title: &str,
        scope: &RemoteConversationScope,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        if title.trim().is_empty() || title.len() > 256 {
            return Err(RemoteError("conversation title is invalid".into()));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.endpoint("/api/v1/conversations")?)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({"scope": scope_json(scope), "title": title})),
            )
            .await?;
        super::super::validate_created_conversation(&response, scope, title)?;
        Ok(response)
    }

    pub(in crate::remote_command) async fn list_prompts(
        &self,
        conversation_id: &str,
        before: Option<&PromptPageCursor>,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        if let Some(cursor) = before {
            validate_entity_id(&cursor.prompt_id, "Prompt")?;
            if i64::try_from(cursor.created_at_ms).is_err() {
                return Err(RemoteError("Prompt cursor time is invalid".into()));
            }
        }
        let url = self.conversation_prompts_url(conversation_id)?;
        let mut request = self.http.get(url).query(&[("limit", PAGE_SIZE)]);
        if let Some(cursor) = before {
            request = request.query(&[
                ("before_created_at_ms", cursor.created_at_ms.to_string()),
                ("before_prompt_id", cursor.prompt_id.clone()),
            ]);
        }
        let page = self.send_read_json(request).await?;
        prompt_page::validate_prompt_page_before(&page, conversation_id, before)
            .map_err(RemoteError)?;
        Ok(page)
    }
}
