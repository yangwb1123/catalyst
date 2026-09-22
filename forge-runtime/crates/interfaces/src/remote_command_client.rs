use std::{env, sync::Arc, time::Duration};

use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client_instance_session_scope::ClientInstanceSessionScope;

#[path = "remote_command_client_changes_watch.rs"]
mod changes_watch;
#[path = "remote_command_client_execution_consent.rs"]
mod execution_consent;
#[path = "remote_command_client_import.rs"]
mod import_client;
#[path = "remote_lifecycle_registry.rs"]
mod lifecycle_registry;
#[path = "remote_command_client_local_runner_preview.rs"]
mod local_runner_preview;
#[path = "remote_pending_intent.rs"]
mod pending_intent;
#[path = "remote_command_client_retry.rs"]
mod retry;
#[path = "remote_command_client_runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_command_client_session_runner_receipt.rs"]
mod session_runner_receipt;
#[path = "remote_command_client_urls.rs"]
mod urls;

use crate::{
    args::{PromptPageCursor, RemoteConversationScope},
    runtime_domain::{OwnedRunStatus, run_observed::RunObserved},
};

use super::{
    RemoteError,
    changes::OwnedConversationChangePage,
    client_auth::{SavedTokenProvider, access_token_from_env},
    conversation_has_scope,
    credentials::ChangeCursorStore,
    parse_api_url, prompt_page, read_json_response, scope_json, validate_conversation_id,
    validate_conversation_page, validate_entity_id, validate_owned_conversation_entry,
    validate_run_page, validate_run_page_request, validate_run_timeline, validate_timeline_request,
};

use retry::{is_transient_read_error, retry_request_and_wait};

const PAGE_SIZE: &str = "128";
const MAX_SESSION_LIST_PAGES: usize = 64;
const MAX_READ_ATTEMPTS: usize = 3;
const READ_RETRY_BACKOFF_MS: [u64; MAX_READ_ATTEMPTS - 1] = [50, 100];
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedConversationPage {
    pub(super) conversations: Vec<OwnedConversationEntry>,
    #[serde(default)]
    pub(super) next_after_id: Option<String>,
    pub(super) has_more: bool,
}

#[derive(Clone, Deserialize, Serialize)]
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
    pub(super) token_refresh: Option<Arc<SavedTokenProvider>>,
}

impl RemoteClient {
    pub(super) async fn from_env() -> Result<Self, RemoteError> {
        let api_url = env::var("FORGE_API_URL")
            .map_err(|_| RemoteError("FORGE_API_URL is required".into()))?;
        if api_url.len() > 2048 {
            return Err(RemoteError(
                "Forge API configuration exceeds the size limit".into(),
            ));
        }
        let base_url = parse_api_url(&api_url)?;
        let (access_token, change_cursor, token_refresh) =
            access_token_from_env(base_url.as_str())?;
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
        let mut client = Self {
            http,
            base_url,
            access_token,
            change_cursor,
            token_refresh,
        };
        if let Some(provider) = &client.token_refresh {
            client.access_token = provider.access_token().await?;
        }
        Ok(client)
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
        self.send_json_with_policy(request, false).await
    }

    /// Sends an authenticated read request with a bounded retry window.
    ///
    /// This path is GET-only; POST uses `send_json` once so callers can
    /// explicitly retry uncertain writes with their idempotency key.
    async fn send_read_json(&self, request: reqwest::RequestBuilder) -> Result<Value, RemoteError> {
        self.send_json_with_policy(request, true).await
    }

    async fn send_json_with_policy(
        &self,
        request: reqwest::RequestBuilder,
        retry_transient_read: bool,
    ) -> Result<Value, RemoteError> {
        let access_token = match &self.token_refresh {
            Some(provider) => provider.access_token().await?,
            None => self.access_token.clone(),
        };
        let mut request = Some(request.bearer_auth(access_token));
        for attempt in 0..MAX_READ_ATTEMPTS {
            let current_request = request
                .take()
                .expect("the current request is restored before another attempt");
            let retry_request = if retry_transient_read && attempt + 1 < MAX_READ_ATTEMPTS {
                current_request.try_clone()
            } else {
                None
            };
            match current_request.send().await {
                Ok(response) => match read_json_response(response).await {
                    Ok(value) => return Ok(value),
                    Err(error) if retry_request.is_some() && is_transient_read_error(&error) => {
                        request = Some(retry_request_and_wait(retry_request, attempt).await);
                    }
                    Err(error) => return Err(error),
                },
                Err(_) if retry_request.is_some() => {
                    request = Some(retry_request_and_wait(retry_request, attempt).await);
                }
                Err(_) => return Err(RemoteError("Forge API request failed".into())),
            }
        }
        unreachable!("the bounded read-attempt loop returns on every request")
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
        let response = self.send_read_json(request).await?;
        let page: OwnedConversationPage = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid conversation page".into()))?;
        validate_conversation_page(&page, after_id)?;
        Ok(page)
    }

    /// Reads the private, owner-bound inventory candidate. The server derives
    /// the owner from the verified bearer claims; the client accepts only the
    /// shared unverified observation envelope and never treats it as authority.
    pub(super) async fn read_device_inventory(&self) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(self.http.get(self.endpoint("/api/v1/devices")?))
            .await?;
        crate::device_inventory_command::validate_remote_response(&response)
            .map_err(|_| RemoteError("Forge API returned an invalid device inventory".into()))?;
        Ok(response)
    }

    /// Reads the explicitly requested lifecycle-registry candidate. This is
    /// a GET-only value observation; it is never part of startup or `sync`.
    pub(super) async fn read_lifecycle_registry(&self) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(self.http.get(self.lifecycle_registry_url()?))
            .await?;
        lifecycle_registry::validate_response(&response)?;
        Ok(response)
    }

    /// Posts one explicitly requested metadata-only credential lifecycle
    /// candidate.  This is an injected, owner-scoped seam: the caller must
    /// provide the reviewed request image, the POST is sent exactly once, and
    /// no credential material is accepted or returned by this client layer.
    pub(super) async fn preview_device_credential_candidate(
        &self,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        crate::device_credential_candidate_command::validate_remote_request(request)
            .map_err(|error| RemoteError(error.to_string()))?;
        let response = self
            .send_json(
                self.http
                    .post(self.credential_candidate_url()?)
                    .json(request),
            )
            .await?;
        crate::device_credential_candidate_command::validate_remote_response(&response, request)
            .map_err(|error| RemoteError(error.to_string()))?;
        Ok(response)
    }

    /// Reads the private, owner-bound lossless v2 inventory candidate. The
    /// candidate is never mounted by production constructors; this client
    /// method is useful for explicit test/injected sources only.
    pub(super) async fn read_device_inventory_v2(&self) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(
                self.http
                    .get(self.endpoint("/api/v1/devices/observations/v2")?),
            )
            .await?;
        crate::device_inventory_observation_v2_command::validate_remote_response(&response)
            .map_err(|_| RemoteError("Forge API returned an invalid v2 device inventory".into()))?;
        Ok(response)
    }

    /// Reads the private owner-bound client-instance/session observation.
    /// Forge mounts this path only behind the accepted device-fabric
    /// activation assembly; the ordinary server constructor remains closed.
    /// The client validates and returns only the strict display-only envelope.
    pub(super) async fn read_client_instance_session_view(&self) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(
                self.http
                    .get(self.endpoint("/api/v1/client-instances/session-view")?),
            )
            .await?;
        crate::device_client_session_view_command::validate_remote_response(&response).map_err(
            |_| RemoteError("Forge API returned an invalid client-instance session view".into()),
        )?;
        Ok(response)
    }

    /// Reads the private owner-bound client-instance/resource observation.
    /// The accepted activation assembly may mount it, while the ordinary
    /// server constructor remains closed. It is a strict display-only
    /// composition and carries no inventory, scheduling, reservation, or
    /// execution authority.
    pub(super) async fn read_client_instance_resource_view(&self) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(
                self.http
                    .get(self.endpoint("/api/v1/client-instances/resource-view")?),
            )
            .await?;
        crate::device_client_instance_resource_view_command::validate_remote_response(&response)
            .map_err(|_| {
                RemoteError("Forge API returned an invalid client-instance resource view".into())
            })?;
        Ok(response)
    }

    pub(super) async fn get_conversation(
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

    pub(super) async fn list_conversations_json(
        &self,
        after_id: Option<&str>,
        scope: Option<&RemoteConversationScope>,
        all_pages: bool,
    ) -> Result<Value, RemoteError> {
        self.list_conversations_json_with_instance(after_id, scope, None, all_pages)
            .await
    }

    pub(super) async fn list_conversations_json_with_instance(
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

    pub(super) async fn conversation_changes_after(
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

    pub(super) async fn preview_device_placement(
        &self,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        self.send_json(
            self.http
                .post(self.endpoint("/api/v1/device-placement/preview")?)
                .json(request),
        )
        .await
    }

    /// Posts one explicit owner-bound registry placement preview. The
    /// candidate returns only a deterministic v2 observation; it never
    /// selects, reserves, schedules, dispatches, or executes work.
    pub(super) async fn preview_device_placement_registry(
        &self,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        self.send_json(
            self.http
                .post(self.endpoint("/api/v1/device-placement/registry-preview")?)
                .json(request),
        )
        .await
    }

    pub(super) async fn preview_session_device_observation(
        &self,
        conversation_id: &str,
        run_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        self.send_json(
            self.http
                .post(self.session_device_observation_url(conversation_id, run_id)?)
                .json(request),
        )
        .await
    }

    /// Posts one authenticated Run/Attempt/lease preflight declaration. The
    /// candidate is stateless and metadata-only, so this uses the one-shot
    /// POST path and never retries an uncertain write.
    pub(super) async fn preview_run_attempt_lease_dispatch_preflight(
        &self,
        conversation_id: &str,
        run_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        self.send_json(
            self.http
                .post(self.run_attempt_lease_dispatch_preflight_url(conversation_id, run_id)?)
                .json(request),
        )
        .await
    }

    /// Posts one caller-supplied restart image to the authenticated
    /// reconciliation candidate. The request is sent exactly once; the
    /// caller must treat the response as metadata-only classification.
    pub(super) async fn preview_execution_reconciliation(
        &self,
        conversation_id: &str,
        run_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        self.send_json(
            self.http
                .post(self.execution_reconciliation_preview_url(conversation_id, run_id)?)
                .json(request),
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
        let page = self.send_read_json(request).await?;
        prompt_page::validate_prompt_page_before(&page, conversation_id, before)
            .map_err(RemoteError)?;
        Ok(page)
    }

    pub(super) async fn list_pending_run_intents(
        &self,
        conversation_id: &str,
        limit: usize,
        before_submitted_at_ms: Option<u64>,
        before_intent_id: Option<&str>,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        if limit == 0 || limit > pending_intent::PAGE_SIZE {
            return Err(RemoteError(
                "pending Run-intent page request is invalid".into(),
            ));
        }
        if before_submitted_at_ms.is_some() != before_intent_id.is_some()
            || before_submitted_at_ms.is_some_and(|value| value > MAX_SAFE_JSON_INTEGER)
        {
            return Err(RemoteError("pending Run-intent cursor is invalid".into()));
        }
        if let Some(intent_id) = before_intent_id {
            validate_entity_id(intent_id, "pending Run-intent")?;
        }
        let mut request = self
            .http
            .get(self.pending_run_intents_url(conversation_id)?)
            .query(&[("limit", limit.to_string())]);
        if let (Some(submitted_at_ms), Some(intent_id)) = (before_submitted_at_ms, before_intent_id)
        {
            request = request.query(&[
                ("before_submitted_at_ms", submitted_at_ms.to_string()),
                ("before_intent_id", intent_id.to_owned()),
            ]);
        }
        let response = self.send_read_json(request).await?;
        let page: pending_intent::Page = serde_json::from_value(response).map_err(|_| {
            RemoteError("Forge API returned an invalid pending Run-intent page".into())
        })?;
        let before =
            before_submitted_at_ms
                .zip(before_intent_id)
                .map(|(submitted_at_ms, intent_id)| pending_intent::Cursor {
                    submitted_at_ms,
                    intent_id: intent_id.to_owned(),
                });
        pending_intent::validate_page(&page, conversation_id, before.as_ref(), limit)
            .map_err(RemoteError)?;
        serde_json::to_value(page)
            .map_err(|_| RemoteError("pending Run-intent page could not be encoded".into()))
    }

    /// Submits a Prompt to the private inert pending Run-intent candidate.
    /// The response is an immutable receipt only; it does not create an
    /// ordinary Run, select a device, or authorize execution.
    pub(super) async fn submit_pending_run_intent(
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
            return Err(RemoteError("pending Run-intent content is invalid".into()));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.pending_run_intents_url(conversation_id)?)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({
                        "content": content,
                        "expected_version": expected_version,
                    })),
            )
            .await?;
        let submission: pending_intent::Submission = serde_json::from_value(response.clone())
            .map_err(|_| {
                RemoteError("Forge API returned an invalid pending Run-intent submission".into())
            })?;
        pending_intent::validate_submission(&submission, conversation_id, content)
            .map_err(RemoteError)?;
        serde_json::to_value(submission)
            .map_err(|_| RemoteError("pending Run-intent submission could not be encoded".into()))
    }

    pub(super) async fn pending_run_intent_timeline(
        &self,
        conversation_id: &str,
        intent_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(intent_id, "pending Run-intent")?;
        if after_sequence > MAX_SAFE_JSON_INTEGER || limit == 0 || limit > pending_intent::PAGE_SIZE
        {
            return Err(RemoteError(
                "pending Run-intent timeline request is invalid".into(),
            ));
        }
        let url = self.pending_run_intent_timeline_url(conversation_id, intent_id)?;
        let request = self.http.get(url).query(&[
            ("after_sequence", after_sequence.to_string()),
            ("limit", limit.to_string()),
        ]);
        let response = self.send_read_json(request).await?;
        let page: pending_intent::TimelinePage =
            serde_json::from_value(response).map_err(|_| {
                RemoteError("Forge API returned an invalid pending Run-intent timeline".into())
            })?;
        pending_intent::validate_timeline(&page, conversation_id, intent_id, after_sequence, limit)
            .map_err(RemoteError)?;
        serde_json::to_value(page)
            .map_err(|_| RemoteError("pending Run-intent timeline could not be encoded".into()))
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
        let response = self.send_read_json(request).await?;
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

    /// Reads one owner-bound, content-free Run observation from the explicit
    /// authenticated candidate. The server derives the owner from the bearer
    /// claims; the client binds the returned projection to the requested
    /// Conversation and Run and accepts no execution authority.
    pub(super) async fn read_run_observation(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<RunObserved, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        let response = self
            .send_read_json(
                self.http
                    .get(self.run_observation_url(conversation_id, run_id)?),
            )
            .await?;
        let observation: RunObserved = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid Run observation".into()))?;
        if observation.conversation_id != conversation_id || observation.run_id != run_id {
            return Err(RemoteError(
                "Forge API returned a Run observation with mismatched binding".into(),
            ));
        }
        observation
            .validate()
            .map_err(|_| RemoteError("Forge API returned an invalid Run observation".into()))?;
        Ok(observation)
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
            .send_read_json(
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
        Ok(response)
    }
}
