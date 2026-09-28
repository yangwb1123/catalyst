use super::{TuiState, json_text};
use serde_json::Value;

use crate::args::RemoteConversationScope;
use crate::client_instance_session_scope;

use super::super::super::{
    OwnedConversationEntry, OwnedConversationPage, RemoteClient, RemoteError,
};

pub(crate) fn scope_filter_matches(
    conversation: &Value,
    filter: Option<&RemoteConversationScope>,
) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let Some(scope) = conversation.get("scope") else {
        return false;
    };
    let kind = scope.get("kind").and_then(Value::as_str);
    let id = scope.get("id").and_then(Value::as_str);
    match filter {
        RemoteConversationScope::Global => kind == Some("global"),
        RemoteConversationScope::Project(expected_id) => {
            kind == Some("project") && id == Some(expected_id.as_str())
        }
        RemoteConversationScope::Group(expected_id) => {
            kind == Some("group") && id == Some(expected_id.as_str())
        }
    }
}

pub(crate) fn scope_filter_label(filter: &RemoteConversationScope) -> String {
    match filter {
        RemoteConversationScope::Global => "global".to_owned(),
        RemoteConversationScope::Project(id) => format!("project:{}", json_text(id)),
        RemoteConversationScope::Group(id) => format!("group:{}", json_text(id)),
    }
}

/// Loads a page and commits it to the local session view only after validation.
///
/// # Errors
///
/// Returns an error when the API request fails or the page violates its cursor contract.
pub(crate) async fn refresh_sessions(
    client: &RemoteClient,
    state: &mut TuiState,
    next_page: bool,
) -> Result<(), RemoteError> {
    // Once the caller selects an instance, do not commit an unfiltered owner
    // page to the TUI state. Rendering already applies this projection, but
    // keeping the hidden rows in process state would let a stale selection or
    // a later command observe data outside the selected instance. A missing
    // validated view therefore fails closed before the Conversation request.
    super::ensure_client_instance_projection(state)?;
    let after = next_page_after(state, next_page)?;
    if next_page && after.is_none() {
        return Ok(());
    }
    let page = filter_client_instance_page(state, client.list_conversations(after).await?)?;
    let revalidated_selected =
        match revalidate_missing_selected(client, state, &page, next_page).await {
            Ok(selected) => selected,
            Err(error) => {
                replace_page(state, page);
                clear_selected_projection(state);
                return Err(error);
            }
        };
    if next_page {
        append_page(state, page);
    } else {
        replace_page(state, page);
        if let Some((selected_id, detail)) = revalidated_selected {
            state.selected_id = Some(selected_id);
            state.selected_entry = Some(detail);
        }
    }
    Ok(())
}

fn next_page_after(state: &TuiState, next_page: bool) -> Result<Option<&str>, RemoteError> {
    if !next_page {
        return Ok(None);
    }
    if !state.has_more {
        return Ok(None);
    }
    state
        .next_after_id
        .as_deref()
        .map(Some)
        .ok_or_else(|| RemoteError("Forge API pagination cursor is unavailable".into()))
}

async fn revalidate_missing_selected(
    client: &RemoteClient,
    state: &TuiState,
    page: &OwnedConversationPage,
    next_page: bool,
) -> Result<Option<(String, OwnedConversationEntry)>, RemoteError> {
    if next_page {
        return Ok(None);
    }
    let Some(selected_id) = state
        .selected_id
        .clone()
        .filter(|selected_id| !page_contains_id(&page.conversations, selected_id))
    else {
        return Ok(None);
    };
    if !super::conversation_visible_to_selected_client_instance(
        state,
        &serde_json::json!({"id": selected_id}),
    ) {
        return Ok(None);
    }
    client
        .get_conversation(&selected_id)
        .await
        .map(|detail| Some((selected_id, detail)))
}

fn clear_selected_projection(state: &mut TuiState) {
    state.selected_id = None;
    state.selected_entry = None;
    state.clear_prompt_history();
    state.clear_run_timeline();
}

fn filter_client_instance_page(
    state: &TuiState,
    page: OwnedConversationPage,
) -> Result<OwnedConversationPage, RemoteError> {
    let Some(instance_id) = state.client_instance_filter.as_deref() else {
        return Ok(page);
    };
    let view = state
        .active_client_instance_view()
        .ok_or_else(|| RemoteError("remote TUI client-instance view is unavailable".into()))?;
    let conversations = page
        .conversations
        .into_iter()
        .filter(|entry| {
            client_instance_session_scope::matches_conversation(
                &entry.conversation,
                Some(view),
                Some(instance_id),
            )
        })
        .collect();
    Ok(OwnedConversationPage {
        conversations,
        next_after_id: page.next_after_id,
        has_more: page.has_more,
    })
}

fn append_page(state: &mut TuiState, page: OwnedConversationPage) {
    let OwnedConversationPage {
        conversations,
        next_after_id,
        has_more,
    } = page;
    for entry in conversations {
        let incoming_id = entry.conversation.get("id").and_then(Value::as_str);
        if let Some(incoming_id) = incoming_id
            && let Some(existing) = state.conversations.iter_mut().find(|existing| {
                existing.conversation.get("id").and_then(Value::as_str) == Some(incoming_id)
            })
        {
            *existing = entry;
        } else {
            state.conversations.push(entry);
        }
    }
    if selected_is_loaded(state) {
        state.selected_entry = None;
    }
    set_page_cursor(state, next_after_id, has_more);
    state.reconcile_client_instance_selection();
}

fn replace_page(state: &mut TuiState, page: OwnedConversationPage) {
    let OwnedConversationPage {
        conversations,
        next_after_id,
        has_more,
    } = page;
    let preserved_selection = preserve_selected_entry(state);
    let selected_id = state.selected_id.clone();
    state.conversations = conversations;
    state.selected_entry = selected_id.as_deref().and_then(|selected_id| {
        if page_contains_id(&state.conversations, selected_id) {
            None
        } else {
            preserved_selection.filter(|entry| {
                entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id)
            })
        }
    });
    if selected_is_available(state) {
        state.selected_id.clone_from(&selected_id);
    } else {
        state.selected_id = state
            .conversations
            .first()
            .and_then(|entry| entry.conversation.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    if state.selected_id != selected_id {
        state.clear_prompt_history();
        state.clear_run_timeline();
    }
    set_page_cursor(state, next_after_id, has_more);
    state.reconcile_client_instance_selection();
}

fn preserve_selected_entry(state: &TuiState) -> Option<OwnedConversationEntry> {
    let selected_id = state.selected_id.as_deref()?;
    state
        .conversations
        .iter()
        .chain(state.selected_entry.iter())
        .find(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id))
        .cloned()
}

fn selected_is_loaded(state: &TuiState) -> bool {
    state
        .selected_id
        .as_deref()
        .is_some_and(|selected_id| page_contains_id(&state.conversations, selected_id))
}

fn selected_is_available(state: &TuiState) -> bool {
    state.selected_id.as_deref().is_some_and(|selected_id| {
        page_contains_id(&state.conversations, selected_id)
            || state.selected_entry.as_ref().is_some_and(|entry| {
                entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id)
            })
    })
}

fn page_contains_id(page: &[OwnedConversationEntry], id: &str) -> bool {
    page.iter()
        .any(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(id))
}

fn set_page_cursor(state: &mut TuiState, next_after_id: Option<String>, has_more: bool) {
    state.next_after_id = next_after_id;
    state.has_more = has_more;
}
