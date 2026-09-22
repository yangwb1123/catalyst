use std::{error::Error, fmt};

use serde_json::{Value, json};

use crate::args::RemoteConversationScope;
use crate::runtime_domain::ConversationImportPrompt;

#[path = "remote_changes.rs"]
mod changes;
#[path = "remote_command_client.rs"]
mod client;
#[path = "remote_client_auth.rs"]
mod client_auth;
#[path = "remote_client_checkpoint.rs"]
mod client_checkpoint;
#[path = "remote_client_run_timeline.rs"]
mod client_run_timeline;
#[cfg(test)]
#[path = "remote_client_run_timeline_tests.rs"]
mod client_run_timeline_tests;
#[path = "remote_credentials.rs"]
mod credentials;
#[path = "remote_command_dispatch.rs"]
mod dispatch;
#[path = "remote_execution_consent_preview.rs"]
mod execution_consent_preview;
#[path = "remote_execution_reconciliation_preview.rs"]
mod execution_reconciliation;
#[path = "remote_command_import.rs"]
mod import;
#[path = "remote_local_runner_preview.rs"]
mod local_runner_preview;
#[path = "remote_login.rs"]
mod login;
#[path = "remote_placement.rs"]
mod placement;
#[path = "remote_placement_registry.rs"]
mod placement_registry;
#[path = "remote_prompt_page.rs"]
mod prompt_page;
#[path = "remote_run_attempt_lease_dispatch_preflight.rs"]
mod run_attempt_lease_dispatch_preflight;
#[path = "remote_runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_session_observation.rs"]
mod session_observation;
#[path = "remote_session_runner_receipt.rs"]
mod session_runner_receipt;
#[path = "remote_command_validation.rs"]
mod validation;

#[cfg(test)]
use client::OwnedRunCursorResponse;
use client::{
    OwnedConversationEntry, OwnedConversationPage, OwnedRunPageResponse, OwnedRunSummaryResponse,
    OwnedRunTimelinePageResponse, RemoteClient,
};
#[cfg(test)]
use client_auth::resolve_access_token;

pub(crate) use dispatch::{execute, execute_with_resolved_prompt};
use validation::is_loopback_host;
use validation::{
    parse_api_url, read_json_response, required_idempotency_key, validate_conversation_id,
    validate_conversation_page, validate_entity_id, validate_owned_conversation_entry,
    validate_run_page, validate_run_page_request, validate_run_timeline, validate_timeline_request,
};

#[derive(Debug)]
pub(crate) struct RemoteError(pub(crate) String);

impl fmt::Display for RemoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RemoteError {}

fn scope_json(scope: &RemoteConversationScope) -> Value {
    match scope {
        RemoteConversationScope::Global => json!({"kind": "global"}),
        RemoteConversationScope::Project(id) => json!({"kind": "project", "id": id}),
        RemoteConversationScope::Group(id) => json!({"kind": "group", "id": id}),
    }
}

fn conversation_has_scope(conversation: &Value, scope: &RemoteConversationScope) -> bool {
    let Some(actual) = conversation.get("scope") else {
        return false;
    };
    match scope {
        RemoteConversationScope::Global => actual == &json!({"kind": "global"}),
        RemoteConversationScope::Project(id) => actual == &json!({"kind": "project", "id": id}),
        RemoteConversationScope::Group(id) => actual == &json!({"kind": "group", "id": id}),
    }
}

pub(crate) async fn run_login() -> Result<(), Box<dyn Error>> {
    login::run().await
}

pub(crate) fn credential_storage_status() -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::to_value(
        credentials::credential_storage_capabilities(),
    )?)
}

pub(crate) async fn run_import(
    state_dir: Option<&std::path::Path>,
    conversation_id: &str,
    confirm: Option<&str>,
    json_output: bool,
) -> Result<(), Box<dyn Error>> {
    import::run(state_dir, conversation_id, confirm, json_output).await
}

fn import_payload(title: &str, prompts: &[ConversationImportPrompt]) -> Value {
    json!({"title": title, "prompts": prompts})
}

#[path = "remote_tui.rs"]
mod tui;

/// Runs the interactive shared-session terminal interface.
///
/// # Errors
///
/// Returns an error when API configuration is invalid or the terminal session fails.
pub(crate) async fn run_tui(state_dir: Option<&std::path::Path>) -> Result<(), Box<dyn Error>> {
    tui::run(state_dir).await.map_err(Into::into)
}

#[cfg(test)]
#[path = "remote_command_tests.rs"]
mod tests;
