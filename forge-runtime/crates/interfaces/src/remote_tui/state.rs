use serde_json::Value;

use super::super::{OwnedConversationEntry, RemoteError};
use crate::args::{PromptPageCursor, RemoteConversationScope};
use crate::client_instance_session_scope;
use crate::runtime_domain::run_observed::RunObserved;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ClientInstanceObservationStatus {
    pub(super) session_view_present: bool,
    pub(super) resource_view_present: bool,
}

#[derive(Default)]
pub(super) struct TuiState {
    pub(super) conversations: Vec<OwnedConversationEntry>,
    pub(super) scope_filter: Option<RemoteConversationScope>,
    /// Caller-selected local projection onto one observed client instance.
    /// This is display state only and never changes the authenticated API
    /// request or grants session/device authority.
    pub(super) client_instance_filter: Option<String>,
    pub(super) selected_id: Option<String>,
    pub(super) selected_entry: Option<OwnedConversationEntry>,
    pub(super) next_after_id: Option<String>,
    pub(super) has_more: bool,
    pub(super) change_cursor: u64,
    pub(super) history_loaded_for: Option<String>,
    pub(super) prompt_history: Vec<Value>,
    pub(super) history_before: Option<PromptPageCursor>,
    pub(super) pending_prompt: Option<PendingPrompt>,
    pub(super) pending_run_intent: Option<PendingRunIntent>,
    pub(super) pending_create: Option<PendingCreate>,
    /// Run selected by the last `timeline` command. This is process-local UI
    /// state; the durable owner-bound sequence remains in the Run timeline
    /// checkpoint used by explicit `--resume`.
    pub(super) selected_run_id: Option<String>,
    pub(super) run_timeline_sequence: u64,
    /// Latest metadata-only observation for the selected Run. This is
    /// process-local display state and never grants execution authority.
    pub(super) selected_run_observed: Option<RunObserved>,
    /// Latest owner-scoped v1 inventory observation requested by the user.
    /// This remains process-local and is refreshed by `sync` only after the
    /// explicit `inventory read` command has opted in.
    pub(super) device_inventory_observed: Option<Value>,
    /// Latest lossless v2 owner-scoped inventory observation requested by the
    /// user. This remains process-local and is refreshed by `sync` only after
    /// the explicit `inventory read-v2` command has opted in.
    pub(super) device_inventory_v2_observed: Option<Value>,
    /// Latest pending Run-intent metadata page explicitly requested by the
    /// user. This is process-local display state; `sync` refreshes only this
    /// exact owner/Conversation/cursor binding and never probes the private
    /// candidate for an ordinary session.
    pub(super) pending_run_intent_page: Option<PendingRunIntentPageObservation>,
    /// Pending Run-intent selected by the last `run-intents timeline` command.
    /// This is deliberately process-local: unlike an ordinary Run, pending
    /// intent observation has no durable checkpoint in this slice. The
    /// conversation and intent binding prevent a stale cursor being reused
    /// for another owner-scoped timeline.
    pub(super) selected_pending_run_intent_conversation_id: Option<String>,
    pub(super) selected_pending_run_intent_id: Option<String>,
    pub(super) pending_run_intent_timeline_sequence: u64,
    /// Latest owner-bound client-instance/session observation explicitly
    /// opened by the user. This is process-local display state; `sync`
    /// refreshes it only after the explicit authenticated read opts in.
    pub(super) client_instance_session_view_observed: Option<Value>,
    /// Latest owner-bound client-instance/resource observation explicitly
    /// opened by the user. It remains metadata-only and never becomes
    /// inventory, placement, reservation, or execution authority.
    pub(super) client_instance_resource_view_observed: Option<Value>,
    /// A paired client-instance observation can temporarily be divergent or
    /// incomplete while retaining the last JSON snapshots for display. This
    /// status is process-local UI state; it never grants authority and makes
    /// the local instance filter/private projection fail closed.
    pub(super) client_instance_observation_status: Option<ClientInstanceObservationStatus>,
}

#[derive(Clone)]
pub(super) struct PendingPrompt {
    pub(super) conversation_id: String,
    pub(super) expected_version: u64,
    pub(super) content: String,
    pub(super) idempotency_key: String,
}

#[derive(Clone)]
pub(super) struct PendingRunIntent {
    pub(super) conversation_id: String,
    pub(super) expected_version: u64,
    pub(super) content: String,
    pub(super) idempotency_key: String,
}

#[derive(Clone)]
pub(super) struct PendingRunIntentPageObservation {
    pub(super) conversation_id: String,
    pub(super) before_submitted_at_ms: Option<u64>,
    pub(super) before_intent_id: Option<String>,
}

#[derive(Clone)]
pub(super) struct PendingCreate {
    pub(super) title: String,
    pub(super) scope: RemoteConversationScope,
    pub(super) idempotency_key: String,
}

impl TuiState {
    pub(super) fn has_pending_write(&self) -> bool {
        self.pending_prompt.is_some()
            || self.pending_run_intent.is_some()
            || self.pending_create.is_some()
    }

    pub(super) fn selected_conversation(&self) -> Option<&OwnedConversationEntry> {
        let selected_id = self.selected_id.as_deref()?;
        self.conversations
            .iter()
            .chain(self.selected_entry.iter())
            .find(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id))
    }

    pub(super) fn record_prompt_history(&mut self, conversation_id: &str, page: &Value) {
        if self.history_loaded_for.as_deref() != Some(conversation_id) {
            self.clear_prompt_history();
        }
        self.history_loaded_for = Some(conversation_id.to_owned());
        if let Some(prompts) = page.get("prompts").and_then(Value::as_array) {
            for prompt in prompts {
                let Some(id) = prompt.get("id").and_then(Value::as_str) else {
                    continue;
                };
                if let Some(existing) = self
                    .prompt_history
                    .iter_mut()
                    .find(|existing| existing.get("id").and_then(Value::as_str) == Some(id))
                {
                    *existing = prompt.clone();
                } else {
                    self.prompt_history.push(prompt.clone());
                }
            }
            self.prompt_history.sort_by(|left, right| {
                let left_key = (
                    left.get("created_at_ms")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    left.get("id").and_then(Value::as_str).unwrap_or_default(),
                );
                let right_key = (
                    right
                        .get("created_at_ms")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    right.get("id").and_then(Value::as_str).unwrap_or_default(),
                );
                left_key.cmp(&right_key)
            });
        }
        let page_cursor = page.get("next_cursor").and_then(|cursor| {
            Some(PromptPageCursor {
                created_at_ms: cursor.get("created_at_ms")?.as_u64()?,
                prompt_id: cursor.get("prompt_id")?.as_str()?.to_owned(),
            })
        });
        self.history_before = match (&self.history_before, page_cursor) {
            (Some(current), Some(incoming)) if cursor_precedes(current, &incoming) => {
                Some(current.clone())
            }
            (_, incoming) => incoming,
        };
    }

    pub(super) fn clear_prompt_history(&mut self) {
        self.history_loaded_for = None;
        self.prompt_history.clear();
        self.history_before = None;
    }

    pub(super) fn clear_run_timeline(&mut self) {
        self.selected_run_id = None;
        self.run_timeline_sequence = 0;
        self.selected_run_observed = None;
        self.clear_pending_run_intent_page();
        self.clear_pending_run_intent_timeline();
    }

    /// Revokes private Prompt/Run projections when the caller changes the
    /// client-instance display boundary. The selected Conversation, owner
    /// observations, and pending writes remain available for an explicit
    /// re-read or retry under the new local projection.
    pub(super) fn clear_client_instance_private_projection(&mut self) {
        self.clear_prompt_history();
        self.clear_run_timeline();
    }

    /// Drops one selected Conversation after an authenticated private read
    /// proves that the cached owner row is no longer readable. This keeps
    /// other owner rows available while revoking the selected Prompt/Run
    /// projection; a later refresh may discover a replacement row normally.
    pub(super) fn drop_selected_session(&mut self, conversation_id: &str) {
        if self.selected_id.as_deref() != Some(conversation_id) {
            return;
        }
        self.conversations.retain(|entry| {
            entry.conversation.get("id").and_then(Value::as_str) != Some(conversation_id)
        });
        self.selected_id = None;
        self.selected_entry = None;
        self.clear_prompt_history();
        self.clear_run_timeline();
    }

    pub(super) fn clear_device_inventory_v2(&mut self) {
        self.device_inventory_v2_observed = None;
    }

    pub(super) fn clear_device_inventory(&mut self) {
        self.device_inventory_observed = None;
    }

    pub(super) fn clear_client_instance_views(&mut self) {
        self.client_instance_session_view_observed = None;
        self.client_instance_resource_view_observed = None;
        self.client_instance_filter = None;
        self.client_instance_observation_status = None;
    }

    /// Records that the two independent readers cannot currently be treated
    /// as one projection. Existing observations remain available to the TUI
    /// renderer, while private Prompt/Run state is revoked immediately.
    pub(super) fn mark_client_instance_observations_not_converged(&mut self) {
        self.client_instance_observation_status = Some(ClientInstanceObservationStatus {
            session_view_present: self.client_instance_session_view_observed.is_some(),
            resource_view_present: self.client_instance_resource_view_observed.is_some(),
        });
        // The instance projection is an optional local boundary. Preserve
        // the existing default owner-read behavior when no filter is active;
        // once a caller selected an instance, revoke its private Prompt/Run
        // cache and selection until both readers converge again.
        if self.client_instance_filter.is_some() {
            self.clear_client_instance_private_projection();
            self.reconcile_client_instance_selection();
        }
    }

    /// Clears a transient observation issue only after both current readers
    /// describe the same owner-scoped instance image.
    pub(super) fn mark_client_instance_observations_converged(&mut self) {
        self.client_instance_observation_status = None;
    }

    /// Recomputes the paired-reader status after an explicit response has
    /// been installed. A single reader remains a valid display-only
    /// candidate; once an issue exists, the missing side keeps the projection
    /// blocked until both readers are present and equal.
    pub(super) fn refresh_client_instance_observation_status(&mut self) {
        match (
            self.client_instance_session_view_observed.as_ref(),
            self.client_instance_resource_view_observed.as_ref(),
        ) {
            (Some(session), Some(resource))
                if super::client_instance_convergence::observations_converged(
                    session, resource,
                ) =>
            {
                self.mark_client_instance_observations_converged();
            }
            (Some(_), Some(_)) => self.mark_client_instance_observations_not_converged(),
            (_, _) if self.client_instance_observation_status.is_some() => {
                self.mark_client_instance_observations_not_converged();
            }
            _ => {}
        }
    }

    pub(super) fn client_instance_observations_blocked(&self) -> bool {
        self.client_instance_observation_status.is_some()
            || matches!(
                (
                    self.client_instance_session_view_observed.as_ref(),
                    self.client_instance_resource_view_observed.as_ref(),
                ),
                (Some(session), Some(resource))
                    if !super::client_instance_convergence::observations_converged(
                        session, resource,
                    )
            )
    }

    /// Revokes one explicitly opened client-instance reader while retaining
    /// the caller's local filter. If the other reader does not provide a
    /// replacement projection, `reconcile_client_instance_selection` clears
    /// the selected private Prompt/Run state and leaves the list empty.
    pub(super) fn clear_client_instance_view(&mut self, kind: &str) -> bool {
        let had_pair = self.client_instance_session_view_observed.is_some()
            && self.client_instance_resource_view_observed.is_some();
        let cleared = match kind {
            "session-view" => self.client_instance_session_view_observed.take().is_some(),
            "resource-view" => self.client_instance_resource_view_observed.take().is_some(),
            _ => false,
        };
        if cleared {
            if self.client_instance_session_view_observed.is_none()
                && self.client_instance_resource_view_observed.is_none()
            {
                self.client_instance_observation_status = None;
                self.reconcile_client_instance_selection();
            } else if had_pair || self.client_instance_observation_status.is_some() {
                self.mark_client_instance_observations_not_converged();
            } else {
                self.reconcile_client_instance_selection();
            }
        }
        cleared
    }

    pub(super) fn active_client_instance_view(&self) -> Option<&Value> {
        // A session declaration and a resource declaration are one local
        // projection when both have been opened.  Do not let a Prompt or Run
        // read use the newer session_ids with an older resource image (or
        // vice versa): a mixed pair can briefly broaden or move the selected
        // instance boundary during refresh.  The pair remains display-only;
        // a drift simply makes the active filter empty until the next
        // converged refresh.
        if self.client_instance_observations_blocked() {
            return None;
        }
        self.client_instance_session_view_observed
            .as_ref()
            .or(self.client_instance_resource_view_observed.as_ref())
    }

    /// Keeps selection and the local Prompt/Run panels inside the selected
    /// instance projection. A filter never performs a network request.
    pub(super) fn reconcile_client_instance_selection(&mut self) {
        let Some(instance_id) = self.client_instance_filter.as_deref() else {
            return;
        };
        let view = self.active_client_instance_view().cloned();
        let selected_visible = self.selected_conversation().is_some_and(|entry| {
            client_instance_session_scope::matches_conversation(
                &entry.conversation,
                view.as_ref(),
                Some(instance_id),
            )
        });
        if selected_visible {
            return;
        }
        self.selected_id = self
            .conversations
            .iter()
            .find(|entry| {
                client_instance_session_scope::matches_conversation(
                    &entry.conversation,
                    view.as_ref(),
                    Some(instance_id),
                )
            })
            .and_then(|entry| entry.conversation.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.selected_entry = None;
        self.clear_prompt_history();
        self.clear_run_timeline();
    }

    pub(super) fn record_pending_run_intent_page(
        &mut self,
        conversation_id: &str,
        before: Option<(u64, String)>,
    ) {
        self.pending_run_intent_page = Some(PendingRunIntentPageObservation {
            conversation_id: conversation_id.to_owned(),
            before_submitted_at_ms: before.as_ref().map(|(time, _)| *time),
            before_intent_id: before.map(|(_, intent_id)| intent_id),
        });
    }

    pub(super) fn clear_pending_run_intent_page(&mut self) {
        self.pending_run_intent_page = None;
    }

    pub(super) fn clear_pending_run_intent_timeline(&mut self) {
        self.selected_pending_run_intent_conversation_id = None;
        self.selected_pending_run_intent_id = None;
        self.pending_run_intent_timeline_sequence = 0;
    }

    /// Drops all owner-scoped material after the API rejects the current
    /// authorization. The interactive process may continue running, but it
    /// must not render data fetched under the previous authorization state.
    pub(super) fn clear_remote_session_view(&mut self) {
        self.conversations.clear();
        self.selected_id = None;
        self.selected_entry = None;
        self.next_after_id = None;
        self.has_more = false;
        self.clear_prompt_history();
        self.pending_prompt = None;
        self.pending_run_intent = None;
        self.pending_create = None;
        self.clear_run_timeline();
        self.clear_device_inventory();
        self.clear_device_inventory_v2();
        self.clear_client_instance_views();
    }
}

/// Fails closed before an owner Conversation page is requested when the
/// caller selected a client-instance projection but has no validated view to
/// apply. The view remains a local display declaration and never changes the
/// authenticated request or grants instance authority.
pub(super) fn ensure_client_instance_projection(state: &TuiState) -> Result<(), RemoteError> {
    if let Some(instance_id) = state
        .client_instance_filter
        .as_deref()
        .filter(|_| state.active_client_instance_view().is_none())
    {
        return Err(RemoteError(format!(
            "remote TUI client-instance filter {instance_id:?} has no validated view"
        )));
    }
    Ok(())
}

/// Fails closed before an owner-wide Conversation create when the caller has
/// selected a client-instance projection without a converged session/resource
/// pair. A create has no existing Conversation to re-check, so a single
/// display observation would otherwise make an instance-scoped write look
/// more precise than its resource boundary.
pub(super) fn ensure_converged_client_instance_projection(
    state: &TuiState,
) -> Result<(), RemoteError> {
    if state.client_instance_filter.is_none() {
        return Ok(());
    }
    if state.client_instance_observations_blocked()
        || state.client_instance_session_view_observed.is_none()
        || state.client_instance_resource_view_observed.is_none()
    {
        return Err(RemoteError(
            "selected client-instance create requires converged session/resource observations"
                .into(),
        ));
    }
    Ok(())
}

/// Returns whether one owner Conversation belongs to the selected local
/// client-instance projection. Missing or divergent observations fail
/// closed so callers cannot accidentally broaden a private read during a
/// refresh gap.
pub(super) fn conversation_visible_to_selected_client_instance(
    state: &TuiState,
    conversation: &Value,
) -> bool {
    let Some(instance_id) = state.client_instance_filter.as_deref() else {
        return true;
    };
    let Some(view) = state.active_client_instance_view() else {
        return false;
    };
    client_instance_session_scope::matches_conversation(conversation, Some(view), Some(instance_id))
}

fn cursor_precedes(left: &PromptPageCursor, right: &PromptPageCursor) -> bool {
    left.created_at_ms < right.created_at_ms
        || (left.created_at_ms == right.created_at_ms && left.prompt_id < right.prompt_id)
}

#[path = "state/render.rs"]
mod render;
#[path = "state/sessions.rs"]
mod sessions;
#[path = "state/utils.rs"]
mod utils;

pub(super) use render::{render, write_help};
pub(super) use sessions::{refresh_sessions, scope_filter_label, scope_filter_matches};
pub(super) use utils::{io_error, json_text, new_idempotency_key, write_pending_recovery};
