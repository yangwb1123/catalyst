use std::{env, time::Duration};

use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    args::{PromptPageCursor, RemoteConversationScope},
    runtime_domain::{ConversationImportPrompt, OwnedRunStatus},
};

use super::{
    RemoteError, changes::OwnedConversationChangePage, client_auth::access_token_from_env,
    conversation_has_scope, credentials::ChangeCursorStore, import_payload, parse_api_url,
    prompt_page, read_json_response, scope_json, validate_conversation_id,
    validate_conversation_page, validate_entity_id, validate_run_page, validate_run_page_request,
    validate_run_timeline, validate_timeline_request,
};

const PAGE_SIZE: &str = "128";
const MAX_SESSION_LIST_PAGES: usize = 64;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedConversationPage {
    pub(super) conversations: Vec<OwnedConversationEntry>,
    #[serde(default)]
    pub(super) next_after_id: Option<String>,
    pub(super) has_more: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedConversationEntry {
    pub(super) conversation: Value,
    pub(super) aggregate_version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedRunPageResponse {
    pub(super) conversation_id: String,
    pub(super) runs: Vec<OwnedRunSummaryResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) next_cursor: Option<OwnedRunCursorResponse>,
    pub(super) has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedRunSummaryResponse {
    pub(super) run_id: String,
    pub(super) prompt_id: String,
    pub(super) created_at_ms: u64,
    pub(super) latest_sequence: u64,
    pub(super) status: OwnedRunStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedRunCursorResponse {
    pub(super) created_at_ms: u64,
    pub(super) run_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum OwnedRunTimelineEventTypeResponse {
    RunStarted,
    TurnStarted,
    Activity,
    RunFinished,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedRunTimelineEventResponse {
    pub(super) seq: u64,
    pub(super) emitted_at_ms: u64,
    #[serde(rename = "type")]
    pub(super) event_type: OwnedRunTimelineEventTypeResponse,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedRunTimelinePageResponse {
    pub(super) conversation_id: String,
    pub(super) run_id: String,
    pub(super) after_sequence: u64,
    pub(super) scanned_through_sequence: u64,
    pub(super) has_more: bool,
    pub(super) events: Vec<OwnedRunTimelineEventResponse>,
}

pub(super) struct RemoteClient {
    pub(super) http: Client,
    pub(super) base_url: Url,
    pub(super) access_token: String,
    pub(super) change_cursor: Option<ChangeCursorStore>,
}

impl RemoteClient {
    pub(super) fn from_env() -> Result<Self, RemoteError> {
        let api_url = env::var("FORGE_API_URL")
            .map_err(|_| RemoteError("FORGE_API_URL is required".into()))?;
        if api_url.len() > 2048 {
            return Err(RemoteError(
                "Forge API configuration exceeds the size limit".into(),
            ));
        }
        let base_url = parse_api_url(&api_url)?;
        let (access_token, change_cursor) = access_token_from_env(base_url.as_str())?;
        if access_token.len() > 8192 {
            return Err(RemoteError(
                "Forge API configuration exceeds the size limit".into(),
            ));
        }
        let http = Client::builder()
            .redirect(Policy::none())
            .https_only(base_url.scheme() == "https")
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| RemoteError("could not configure the Forge API client".into()))?;
        Ok(Self {
            http,
            base_url,
            access_token,
            change_cursor,
        })
    }

    pub(super) fn endpoint(&self, path: &str) -> Result<Url, RemoteError> {
        self.base_url
            .join(path)
            .map_err(|_| RemoteError("Forge API endpoint is invalid".into()))
    }

    pub(super) async fn send_json(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<Value, RemoteError> {
        let response = request
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|_| RemoteError("Forge API request failed".into()))?;
        read_json_response(response).await
    }

    pub(super) async fn list_conversations(
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
        let response = self.send_json(request).await?;
        let page: OwnedConversationPage = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid conversation page".into()))?;
        validate_conversation_page(&page, after_id)?;
        Ok(page)
    }

    pub(super) async fn list_conversations_json(
        &self,
        after_id: Option<&str>,
        scope: Option<&RemoteConversationScope>,
        all_pages: bool,
    ) -> Result<Value, RemoteError> {
        let mut cursor = after_id.map(str::to_owned);
        let mut conversations = Vec::new();
        let mut pages_read = 0;
        loop {
            let page = self.list_conversations(cursor.as_deref()).await?;
            pages_read += 1;
            for entry in page.conversations {
                if scope.is_none_or(|scope| conversation_has_scope(&entry.conversation, scope)) {
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

    pub(super) async fn conversation_changes_after(
        &self,
        after_cursor: u64,
    ) -> Result<OwnedConversationChangePage, RemoteError> {
        if i64::try_from(after_cursor).is_err() {
            return Err(RemoteError("conversation change cursor is invalid".into()));
        }
        let limit = 128_usize;
        let response = self
            .send_json(
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

    pub(super) async fn create_conversation(
        &self,
        title: &str,
        scope: &RemoteConversationScope,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        if title.trim().is_empty() || title.len() > 256 {
            return Err(RemoteError("conversation title is invalid".into()));
        }
        self.send_json(
            self.http
                .post(self.endpoint("/api/v1/conversations")?)
                .header("Idempotency-Key", idempotency_key)
                .json(&json!({"scope": scope_json(scope), "title": title})),
        )
        .await
    }

    pub(super) async fn list_prompts(
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
        let page = self.send_json(request).await?;
        prompt_page::validate_prompt_page_before(&page, conversation_id, before)
            .map_err(RemoteError)?;
        Ok(page)
    }

    pub(super) async fn list_runs(
        &self,
        conversation_id: &str,
        limit: usize,
        before_created_at_ms: Option<u64>,
        before_run_id: Option<&str>,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_run_page_request(limit, before_created_at_ms, before_run_id)?;
        let mut request = self
            .http
            .get(self.conversation_runs_url(conversation_id)?)
            .query(&[("limit", limit.to_string())]);
        if let (Some(created_at_ms), Some(run_id)) = (before_created_at_ms, before_run_id) {
            request = request.query(&[
                ("before_created_at_ms", created_at_ms.to_string()),
                ("before_run_id", run_id.to_owned()),
            ]);
        }
        let response = self.send_json(request).await?;
        let page: OwnedRunPageResponse = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid Run page".into()))?;
        validate_run_page(
            &page,
            conversation_id,
            limit,
            before_created_at_ms,
            before_run_id,
        )
        .map_err(RemoteError)?;
        serde_json::to_value(page).map_err(|_| RemoteError("Run page could not be encoded".into()))
    }

    pub(super) async fn run_timeline(
        &self,
        conversation_id: &str,
        run_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        validate_timeline_request(after_sequence, limit)?;
        let response = self
            .send_json(
                self.http
                    .get(self.run_timeline_url(conversation_id, run_id)?)
                    .query(&[
                        ("after_sequence", after_sequence.to_string()),
                        ("limit", limit.to_string()),
                    ]),
            )
            .await?;
        let page: OwnedRunTimelinePageResponse = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid Run timeline".into()))?;
        validate_run_timeline(&page, conversation_id, run_id, after_sequence, limit)
            .map_err(RemoteError)?;
        serde_json::to_value(page)
            .map_err(|_| RemoteError("Run timeline could not be encoded".into()))
    }

    pub(super) async fn append_prompt(
        &self,
        conversation_id: &str,
        expected_version: u64,
        content: &str,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        if content.trim().is_empty() || content.len() > 256 * 1024 {
            return Err(RemoteError("prompt content is invalid".into()));
        }
        self.send_json(
            self.http
                .post(self.conversation_prompts_url(conversation_id)?)
                .header("Idempotency-Key", idempotency_key)
                .json(&json!({
                    "content": content,
                    "expected_version": expected_version,
                })),
        )
        .await
    }

    pub(super) async fn import_owned_conversation(
        &self,
        title: &str,
        prompts: &[ConversationImportPrompt],
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        if title.trim().is_empty() || title.len() > 256 || prompts.len() > 128 {
            return Err(RemoteError("conversation import is invalid".into()));
        }
        let mut total_bytes = 0_usize;
        for prompt in prompts {
            if !matches!(prompt.role.as_str(), "user" | "assistant")
                || prompt.content.trim().is_empty()
                || prompt.content.len() > 256 * 1024
            {
                return Err(RemoteError("conversation import is invalid".into()));
            }
            total_bytes = total_bytes.saturating_add(prompt.content.len());
            if total_bytes > 256 * 1024 {
                return Err(RemoteError("conversation import is invalid".into()));
            }
        }
        self.send_json(
            self.http
                .post(self.endpoint("/api/v1/conversations/import")?)
                .header("Idempotency-Key", idempotency_key)
                .json(&import_payload(title, prompts)),
        )
        .await
    }

    pub(super) fn conversation_prompts_url(
        &self,
        conversation_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.endpoint("/api/v1/conversations")?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .pop_if_empty()
            .push(conversation_id)
            .push("prompts");
        Ok(url)
    }

    pub(super) fn conversation_runs_url(&self, conversation_id: &str) -> Result<Url, RemoteError> {
        let mut url = self.endpoint("/api/v1/conversations")?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .pop_if_empty()
            .push(conversation_id)
            .push("runs");
        Ok(url)
    }

    pub(super) fn run_timeline_url(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_runs_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(run_id)
            .push("timeline");
        Ok(url)
    }
}
