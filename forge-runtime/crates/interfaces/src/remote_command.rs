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
#[path = "remote_client_instance_convergence.rs"]
mod client_instance_convergence;
#[path = "remote_command_client_client_instance_convergence.rs"]
mod client_instance_convergence_client;
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
#[path = "remote_inventory_convergence.rs"]
mod inventory_convergence;
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
#[path = "remote_run_execution_evidence.rs"]
mod run_execution_evidence;
#[path = "remote_runner_attempt_boundary.rs"]
mod runner_attempt_boundary;
#[path = "remote_runner_dispatch_admission.rs"]
mod runner_dispatch_admission;
#[path = "remote_runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_runner_execution_boundary.rs"]
mod runner_execution_boundary;
#[path = "remote_runner_execution_intent.rs"]
mod runner_execution_intent;
#[path = "remote_runner_transport_admission.rs"]
mod runner_transport_admission;
#[path = "remote_scheduler_lease.rs"]
mod scheduler_lease;
#[path = "remote_scheduler_lease_release.rs"]
mod scheduler_lease_release;
#[path = "remote_scheduler_lease_renew.rs"]
mod scheduler_lease_renew;
#[path = "remote_scheduler_selection.rs"]
mod scheduler_selection;
#[path = "remote_session_observation.rs"]
mod session_observation;
#[path = "remote_session_runner_receipt.rs"]
mod session_runner_receipt;
#[path = "remote_session_runner_receipt_history.rs"]
mod session_runner_receipt_history;
#[path = "remote_session_runner_reconciliation.rs"]
mod session_runner_reconciliation;
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
    validate_conversation_page, validate_created_conversation, validate_entity_id,
    validate_owned_conversation_entry, validate_run_page, validate_run_page_request,
    validate_run_timeline, validate_timeline_request,
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

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

fn validate_import_result(
    result: &Value,
    requested_title: &str,
    expected_prompt_count: usize,
) -> Result<(), RemoteError> {
    let object = result
        .as_object()
        .ok_or_else(|| RemoteError("Forge API returned an invalid import result".into()))?;
    if object.len() != 4
        || ![
            "conversation",
            "aggregate_version",
            "imported_prompt_count",
            "replayed",
        ]
        .iter()
        .all(|field| object.contains_key(*field))
    {
        return Err(RemoteError(
            "Forge API returned an invalid import result".into(),
        ));
    }
    validate_created_conversation(
        object
            .get("conversation")
            .expect("validated import conversation field"),
        &RemoteConversationScope::Global,
        requested_title,
    )?;
    if object
        .get("aggregate_version")
        .and_then(Value::as_u64)
        .is_none_or(|version| version == 0 || version > MAX_SAFE_JSON_INTEGER)
        || object.get("imported_prompt_count").and_then(Value::as_u64)
            != u64::try_from(expected_prompt_count).ok()
        || object.get("replayed").and_then(Value::as_bool).is_none()
    {
        return Err(RemoteError(
            "Forge API returned an invalid import result".into(),
        ));
    }
    Ok(())
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
