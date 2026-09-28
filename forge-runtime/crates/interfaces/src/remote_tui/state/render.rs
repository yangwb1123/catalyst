use std::io::Write;

use serde_json::Value;

use crate::client_instance_session_scope;

use super::{TuiState, io_error, json_text, scope_filter_label, scope_filter_matches};

pub(crate) fn render<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    writeln!(writer, "\nForge shared sessions").map_err(io_error)?;
    render_client_instance_observation_status(state, writer)?;
    render_conversations(state, writer)?;
    render_selected_entry(state, writer)?;
    render_selected_history(state, writer)?;
    render_selected_run_observation(state, writer)?;
    render_pagination(state, writer)?;
    render_pending_writes(state, writer)
}

fn render_client_instance_observation_status<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    let Some(status) = state.client_instance_observation_status else {
        return Ok(());
    };
    let session = if status.session_view_present {
        "present"
    } else {
        "missing"
    };
    let resource = if status.resource_view_present {
        "present"
    } else {
        "missing"
    };
    writeln!(
        writer,
        "  Client-instance observations are not converged; instance filtering and private reads through this client-instance projection are blocked until both snapshots converge. Existing observation metadata is retained for display only (session-view={session}, resource-view={resource})."
    )
    .map_err(io_error)
}

fn render_selected_run_observation<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    let Some(observation) = state.selected_run_observed.as_ref() else {
        return Ok(());
    };
    if state.selected_run_id.as_deref() != Some(observation.run_id.as_str())
        || state.selected_id.as_deref() != Some(observation.conversation_id.as_str())
    {
        return Ok(());
    }
    writeln!(writer, "  Selected Run observation:").map_err(io_error)?;
    writeln!(
        writer,
        "    {}  prompt={}  status={}  latest_sequence={}  created_at_ms={}",
        json_text(&observation.run_id),
        json_text(&observation.prompt_id),
        json_text(observation.status),
        observation.latest_sequence,
        observation.created_at_ms
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "    metadata_only: metadata_observed={} content_included={}",
        observation.metadata_observed, observation.content_included
    )
    .map_err(io_error)
}

fn render_selected_history<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    let Some(selected_id) = state.selected_id.as_deref() else {
        return Ok(());
    };
    writeln!(writer, "  Prompt history for {}:", json_text(selected_id)).map_err(io_error)?;
    if state.history_loaded_for.as_deref() != Some(selected_id) {
        writeln!(writer, "    Not loaded; enter open {selected_id} or sync.").map_err(io_error)?;
        return Ok(());
    }
    if state.prompt_history.is_empty() {
        writeln!(writer, "    No prompts.").map_err(io_error)?;
    }
    for prompt in &state.prompt_history {
        let id = prompt
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let role = prompt
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let content = prompt
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let created_at_ms = prompt
            .get("created_at_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        writeln!(
            writer,
            "    {}  {}  {}  {}",
            created_at_ms,
            json_text(id),
            json_text(role),
            json_text(content)
        )
        .map_err(io_error)?;
    }
    if state.history_before.is_some() {
        writeln!(writer, "    More history is available: enter older.").map_err(io_error)?;
    }
    Ok(())
}

fn render_conversations<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    render_conversation_filters(state, writer)?;
    let mut visible = state
        .conversations
        .iter()
        .filter(|entry| scope_filter_matches(&entry.conversation, state.scope_filter.as_ref()))
        .filter(|entry| {
            client_instance_session_scope::matches_conversation(
                &entry.conversation,
                state.active_client_instance_view(),
                state.client_instance_filter.as_deref(),
            )
        })
        .peekable();
    if visible.peek().is_none() {
        let empty_message = empty_conversation_message(state);
        writeln!(writer, "{empty_message}").map_err(io_error)?;
    }
    for entry in visible {
        let Some(id) = entry.conversation.get("id").and_then(Value::as_str) else {
            continue;
        };
        let title = entry
            .conversation
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        let marker = if state.selected_id.as_deref() == Some(id) {
            "*"
        } else {
            " "
        };
        writeln!(
            writer,
            " {marker} {}  {}  [{}]  (version {})",
            json_text(id),
            json_text(title),
            conversation_scope_label(&entry.conversation),
            entry.aggregate_version
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn empty_conversation_message(state: &TuiState) -> &'static str {
    if state.scope_filter.is_some() {
        "  No sessions match this scope filter in the loaded pages."
    } else if state.client_instance_filter.is_some() {
        "  No sessions match this client-instance filter in the loaded pages."
    } else {
        "  No sessions on this page."
    }
}

fn render_conversation_filters<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    if let Some(scope) = &state.scope_filter {
        writeln!(
            writer,
            "  Scope filter: {} (organization-only display filter; not authorization or device identity).",
            scope_filter_label(scope)
        )
        .map_err(io_error)?;
    }
    if let Some(instance_id) = state.client_instance_filter.as_deref() {
        writeln!(
            writer,
            "  Client-instance filter: {} (local display filter; instance/session metadata is unverified).",
            json_text(instance_id)
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn render_selected_entry<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    if let (Some(selected_id), Some(entry)) = (&state.selected_id, &state.selected_entry)
        && entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id)
    {
        let title = entry
            .conversation
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        let filter_note = state
            .scope_filter
            .as_ref()
            .filter(|scope| !scope_filter_matches(&entry.conversation, Some(scope)))
            .map(|_| "; outside current list filter, still selected and openable")
            .unwrap_or_default();
        let instance_filter_note = state
            .client_instance_filter
            .as_deref()
            .filter(|instance_id| {
                !client_instance_session_scope::matches_conversation(
                    &entry.conversation,
                    state.active_client_instance_view(),
                    Some(instance_id),
                )
            })
            .map(|_| "; outside current client-instance filter, still selected and openable")
            .unwrap_or_default();
        writeln!(
            writer,
            " * {}  {}  [{}]  (version {}, outside loaded pages{filter_note}{instance_filter_note})",
            json_text(selected_id),
            json_text(title),
            conversation_scope_label(&entry.conversation),
            entry.aggregate_version
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn render_pagination<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    if state.has_more {
        writeln!(writer, "  More sessions are available: enter next.").map_err(io_error)?;
    }
    Ok(())
}

fn render_pending_writes<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), super::super::super::RemoteError> {
    if let Some(pending) = &state.pending_prompt {
        writeln!(
            writer,
            "  Prompt outcome is not confirmed for {}; enter retry to resend the identical request.\n",
            json_text(&pending.conversation_id)
        )
        .map_err(io_error)?;
    }
    if let Some(pending) = &state.pending_run_intent {
        writeln!(
            writer,
            "  Pending Run-intent outcome is not confirmed for {}; enter retry to resend the identical request.\n",
            json_text(&pending.conversation_id)
        )
        .map_err(io_error)?;
    }
    if state.pending_create.is_some() {
        writeln!(
            writer,
            "  Session creation outcome is not confirmed; enter retry to resend the identical request.\n"
        )
        .map_err(io_error)?;
    }
    Ok(())
}

const HELP_TEXT: &str = concat!(
    "Commands: list | next | filter global|project:ID|group:ID | filter instance:INSTANCE_ID | ",
    "filter clear | instance INSTANCE_ID|list|clear | detail ID | open ID | older | changes list ",
    "[--after-cursor N] [--instance INSTANCE_ID] | changes watch [--after-cursor N] [--polls 1..64] ",
    "[--min-delay-ms 0..10000] [--max-delay-ms 0..60000] [--instance INSTANCE_ID] | changes stream ",
    "[--after-cursor N] [--wait-ms 0..10000] [--instance INSTANCE_ID] | import LOCAL_CONVERSATION_ID",
    " [--confirm SHA256] | runs [--before TIME RUN_ID] | timeline RUN_ID [AFTER_SEQUENCE|--resume] |",
    " run-observed RUN_ID | run-intents [--before TIME INTENT_ID] | run-intents timeline INTENT_ID ",
    "[AFTER_SEQUENCE|--resume] | run-intents submit TEXT | execution-consent-preview | ",
    "lifecycle-registry show | inventory read | inventory read-v2 | inventory show-converged | ",
    "client-instances session-view | client-instances resource-view | client-instances ",
    "show-converged | inventory show --input FILE | inventory persistence-preview --input FILE | ",
    "inventory persisted-observation --input FILE | inventory persisted-observation-v2 --input FILE ",
    "| inventory placement-evaluation --input FILE | inventory placement-evaluation-v2 --input FILE ",
    "| inventory status --input FILE | inventory snapshot-canonical --input FILE | inventory ",
    "resource-summary --input FILE | inventory session-observation --input FILE | inventory ",
    "placement-batch-evaluation --input FILE | heartbeat-persistence-preview --input FILE | ",
    "identity-proof-preview --input FILE | placement-preview --input FILE | ",
    "placement-registry-preview --input FILE | session-observation-preview --input FILE | ",
    "run-intent-preview --input RUN_FILE --placement-input SESSION_FILE | runner-receipt-preview ",
    "--input FILE | runner-lease-fencing-preview --input FILE | execution-lease-checkpoint-preview ",
    "--input FILE | client-session-view-preview --input FILE | client-instance-resource-view-preview",
    " --input FILE | runner-execution-intent-preview --input FILE | session-runner-receipt-preview ",
    "--input FILE | session-runner-receipt-history-preview --input FILE | ",
    "session-runner-reconciliation-preview --input FILE | ",
    "session-runner-reconciliation-remote-preview --input FILE | ",
    "session-runner-receipt-offline-preview --input FILE | ",
    "session-runner-receipt-history-offline-preview --input FILE | ",
    "runner-execution-readiness-preview --input FILE | run-observed-preview --input FILE | ",
    "run-execution-evidence-preview --input FILE | run-attempt-lease-dispatch-preflight-preview ",
    "--input FILE | run-attempt-lease-dispatch-preflight-remote-preview --input FILE | ",
    "runner-dispatch-plan-preview --input FILE | runner-dispatch-plan-remote-preview --input FILE | ",
    "create [--scope global|project:ID|group:ID] TITLE | prompt TEXT | retry | quit | quit ",
    "--discard-pending\nThe scope filter only organizes the displayed session list; it is not ",
    "authorization and does not identify a device. After an explicit authenticated client-instances ",
    "read, `filter instance:INSTANCE_ID` or `instance INSTANCE_ID` applies the caller-declared ",
    "session_ids locally; the instance filter is display-only and does not change authorization. ",
    "Changes list/watch/stream consume the authenticated owner-visible Conversation feed; an ",
    "explicit `--instance` first refreshes a converged session/resource pair, renders only changes ",
    "whose Conversation is declared for that instance, and still advances the local cursor through ",
    "hidden owner rows. Watch uses finite polls and bounded backoff; stream is a bounded one-page ",
    "SSE read and a 204 timeout leaves the cursor unchanged. Import previews the local ownerless Hub",
    " transcript and uploads only after its current digest is confirmed. Inventory read/read-v2 and ",
    "inventory show-converged, lifecycle-registry show, authenticated client-instances reads, and ",
    "all file previews are local or explicitly injected, unverified observations only; ",
    "lifecycle-registry show is an explicit candidate GET and is not refreshed by `sync`. `inventory",
    " show-converged` performs one authenticated v2 inventory GET and one resource-view GET, commits",
    " both only after their owner, device, and revision/generation/heartbeat/resource fields ",
    "converge, and retains the previous pair after a non-authority failure. After an explicit ",
    "`inventory read-v2`, `sync` refreshes that same owner-scoped observation. `client-instances ",
    "show-converged` performs one explicit session-view GET and one resource-view GET, commits both ",
    "only after their owner and instance rows converge, and retains the previous pair after a ",
    "non-authority failure. Pending Run-intent reads are owner-scoped metadata observations; ",
    "`timeline INTENT_ID --resume` reuses only the exact in-process session/intent cursor and never ",
    "creates a Run; submit stores a consent-checked inert receipt and never starts a Run or selects ",
    "a device. The execution-consent preview reads only the server-resolved ",
    "project/profile/digest/maximum TTL for the selected session; it grants no consent, creates no ",
    "Run, selects no device, acquires no lease, reserves no capacity, dispatches no command, and ",
    "contacts no Runner. Placement, session-observation, session-bound Runner receipt, ",
    "lease/fencing, execution-lease checkpoint, client-instance/session-view, ",
    "client-instance-resource-view, and session Runner receipt-history previews are local or ",
    "authenticated stateless read-only value checks; receipt-history reduction rejects unknown, ",
    "duplicate, trailing, lifecycle, and summary-drift input and never retries or requests a device.",
    " The local `session-runner-reconciliation-preview` recomputes its manual projection from an ",
    "explicit file and opens no remote client. The authenticated ",
    "`session-runner-reconciliation-remote-preview` first canonicalizes the full ",
    "owner/Conversation/Run-bound history, then forwards the returned history to the projection ",
    "route; it never issues a lease, reserves a target, dispatches a process, persists a receipt, ",
    "retries, or publishes Audit. The authenticated `session-runner-receipt-history-preview` command",
    " uses the opt-in Core read route once and revalidates the canonical reduction; it does not ",
    "persist history or create retry/selection/dispatch authority. The local Runner ",
    "execution-readiness preview is a test-only authenticated candidate that may invoke an injected ",
    "local executor once; it returns metadata-only receipt fields and never creates a Run, persists ",
    "a receipt, selects a device, reserves capacity, or enables production execution. Run observed ",
    "and Run execution-evidence previews are local metadata-only file reads and never contact a ",
    "device.",
);

pub(crate) fn write_help<W: Write>(writer: &mut W) -> Result<(), super::super::super::RemoteError> {
    writeln!(writer, "{HELP_TEXT}").map_err(io_error)?;
    writeln!(
        writer,
        "runner-execution-intent-remote-preview --input FILE (authenticated Prompt/Run/Runner binding preview; all authority remains false) | run-execution-evidence-remote-preview --input FILE (authenticated metadata-only preview; all authority remains false)"
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "runner-attempt-boundary-preview --input FILE (offline lifecycle metadata preview; strict and all authority remains false)"
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "scheduler-selection-preview --input FILE (planning-only comparison; all authority remains false)"
    )
    .map_err(io_error)
}

fn conversation_scope_label(conversation: &Value) -> String {
    let Some(scope) = conversation.get("scope") else {
        return "scope unknown".to_owned();
    };
    let kind = scope
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match (kind, scope.get("id").and_then(Value::as_str)) {
        ("global", _) => "global".to_owned(),
        ("project" | "group", Some(id)) => format!("{kind}:{}", json_text(id)),
        _ => "scope unknown".to_owned(),
    }
}
