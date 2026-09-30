#[path = "remote_command_client/conversations.rs"]
mod conversations;
#[path = "remote_command_client/device_observations.rs"]
mod device_observations;
#[path = "remote_command_client/pending_run_intents.rs"]
mod pending_run_intents;
#[path = "remote_command_client/prompt_append.rs"]
mod prompt_append;
#[path = "remote_command_client/run_observations.rs"]
mod run_observations;
#[path = "remote_command_client/scheduling.rs"]
mod scheduling;

use std::{env, sync::Arc, time::Duration};

use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client_instance_session_scope::ClientInstanceSessionScope;

#[path = "remote_command_client_changes_stream.rs"]
mod changes_stream;
#[path = "remote_command_client_changes_watch.rs"]
mod changes_watch;
#[path = "remote_command_client_execution_consent.rs"]
mod execution_consent;
#[path = "remote_command_client_import.rs"]
mod import_client;
#[path = "remote_command_client_inventory_convergence.rs"]
mod inventory_convergence;
#[path = "remote_lifecycle_registry.rs"]
mod lifecycle_registry;
#[path = "remote_command_client_local_runner_preview.rs"]
mod local_runner_preview;
#[path = "remote_pending_intent.rs"]
mod pending_intent;
#[path = "remote_command_client_prompt_append_receipt.rs"]
mod prompt_append_receipt;
#[path = "remote_command_client_retry.rs"]
mod retry;
#[path = "remote_command_client_run_execution_evidence.rs"]
mod run_execution_evidence;
#[path = "remote_command_client_runner_attempt_boundary.rs"]
mod runner_attempt_boundary;
#[path = "remote_command_client_runner_dispatch_admission.rs"]
mod runner_dispatch_admission;
#[path = "remote_command_client_runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_command_client_runner_execution_boundary.rs"]
mod runner_execution_boundary;
#[path = "remote_command_client_runner_execution_intent.rs"]
mod runner_execution_intent;
#[path = "remote_command_client_runner_transport_admission.rs"]
mod runner_transport_admission;
#[path = "remote_command_client_session_runner_receipt.rs"]
mod session_runner_receipt;
#[path = "remote_command_client_session_runner_receipt_history.rs"]
mod session_runner_receipt_history;
#[path = "remote_command_client_session_runner_reconciliation.rs"]
mod session_runner_reconciliation;
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

    pub(super) async fn authenticated_owner(
        &self,
    ) -> Result<
        crate::runtime_domain::execution::runner_execution_intent::RunnerExecutionOwner,
        RemoteError,
    > {
        let access_token = match &self.token_refresh {
            Some(provider) => provider.access_token().await?,
            None => self.access_token.clone(),
        };
        super::credentials::owner_from_access_token(&access_token)
            .map_err(|_| RemoteError("Forge access token has no valid owner declaration".into()))
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
}
