use std::io::Write;

use serde_json::Value;

use super::{RemoteClient, RemoteError, state::TuiState, writes};

pub(super) const PAGE_LIMIT: usize = 25;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) async fn command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(conversation_id) = selected_conversation_id(state, writer)?.map(str::to_owned) else {
        return Ok(());
    };
    let argument = argument.trim();
    if let Some(submit_argument) = argument.strip_prefix("submit") {
        if !submit_argument.is_empty()
            && !submit_argument
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        {
            return usage(writer);
        }
        let content = submit_argument.trim();
        if content.is_empty() {
            return usage(writer);
        }
        return writes::send_new_pending_run_intent(client, state, content, writer).await;
    }
    if let Some(timeline_argument) = argument.strip_prefix("timeline") {
        if !timeline_argument.is_empty()
            && !timeline_argument
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        {
            return usage(writer);
        }
        return timeline_command(
            client,
            state,
            &conversation_id,
            timeline_argument.trim(),
            writer,
        )
        .await;
    }
    list_command(client, state, &conversation_id, argument, writer).await
}

async fn timeline_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some((intent_id, requested_after_sequence, resume)) = parse_timeline(argument, writer)?
    else {
        return Ok(());
    };
    if !writes::ensure_pending_run_intent_visible_to_client_instance(
        state,
        conversation_id,
        writer,
    )? {
        return Ok(());
    }
    let after_sequence = if resume {
        let conversation_matches =
            state.selected_pending_run_intent_conversation_id.as_deref() == Some(conversation_id);
        let intent_matches =
            state.selected_pending_run_intent_id.as_deref() == Some(intent_id.as_str());
        if !conversation_matches || !intent_matches {
            writeln!(
                writer,
                "Pending Run-intent timeline resume requires a prior read for this session and intent."
            )
            .map_err(super::state::io_error)?;
            return Ok(());
        }
        state.pending_run_intent_timeline_sequence
    } else {
        requested_after_sequence.unwrap_or(0)
    };
    let page = match client
        .pending_run_intent_timeline(conversation_id, &intent_id, after_sequence, PAGE_LIMIT)
        .await
    {
        Ok(page) => page,
        Err(error) => {
            report_read_failure(state, &error, "timeline", writer)?;
            return Ok(());
        }
    };
    record_timeline(state, conversation_id, intent_id, &page, writer)
}

fn selected_conversation_id<'a, W: Write>(
    state: &'a TuiState,
    writer: &mut W,
) -> Result<Option<&'a str>, RemoteError> {
    let Some(conversation_id) = state.selected_id.as_deref() else {
        writeln!(
            writer,
            "Open a session before viewing its pending Run-intents."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    };
    if state.selected_conversation().is_none() {
        writeln!(
            writer,
            "Refresh or open the selected session before viewing its pending Run-intents."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    }
    Ok(Some(conversation_id))
}

fn parse_cursor<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<Option<ParsedCursor>, RemoteError> {
    if argument.is_empty() {
        return Ok(Some(ParsedCursor { before: None }));
    }
    let Some((flag, remainder)) = argument.split_once(char::is_whitespace) else {
        write_cursor_usage(writer)?;
        return Ok(None);
    };
    if flag != "--before" {
        write_cursor_usage(writer)?;
        return Ok(None);
    }
    let Some((time, intent_id)) = remainder.trim_start().split_once(char::is_whitespace) else {
        writeln!(
            writer,
            "Pending Run-intent cursor requires a time and intent ID."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    };
    let Some(time) = time
        .parse::<u64>()
        .ok()
        .filter(|value| *value <= MAX_SAFE_JSON_INTEGER)
    else {
        writeln!(
            writer,
            "Pending Run-intent cursor time must be an unsigned integer."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    };
    let intent_id = intent_id.trim();
    if intent_id.is_empty() || intent_id.chars().any(char::is_whitespace) {
        writeln!(writer, "Pending Run-intent cursor ID must be one ID.")
            .map_err(super::state::io_error)?;
        return Ok(None);
    }
    Ok(Some(ParsedCursor {
        before: Some((time, intent_id.to_owned())),
    }))
}

fn parse_timeline<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<Option<(String, Option<u64>, bool)>, RemoteError> {
    let Some((intent_id, remainder)) = argument.split_once(char::is_whitespace) else {
        if argument.is_empty() {
            writeln!(
                writer,
                "Use run-intents timeline INTENT_ID [AFTER_SEQUENCE|--resume]."
            )
            .map_err(super::state::io_error)?;
            return Ok(None);
        }
        return Ok(Some((argument.to_owned(), Some(0), false)));
    };
    if intent_id.is_empty() || intent_id.chars().any(char::is_whitespace) {
        writeln!(writer, "Pending Run-intent ID must be one ID.")
            .map_err(super::state::io_error)?;
        return Ok(None);
    }
    let remainder = remainder.trim();
    if remainder == "--resume" {
        return Ok(Some((intent_id.to_owned(), None, true)));
    }
    let after_sequence = if remainder.is_empty() {
        0
    } else if let Some(value) = remainder
        .parse::<u64>()
        .ok()
        .filter(|value| *value <= MAX_SAFE_JSON_INTEGER)
    {
        value
    } else {
        writeln!(
            writer,
            "Pending Run-intent timeline cursor must be an unsigned integer."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    };
    Ok(Some((intent_id.to_owned(), Some(after_sequence), false)))
}

pub(super) fn render_page<W: Write>(page: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let intents = page
        .get("intents")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RemoteError("Forge API returned an invalid pending Run-intent page".into())
        })?;
    if intents.is_empty() {
        writeln!(
            writer,
            "No pending Run-intents are recorded for this session."
        )
        .map_err(super::state::io_error)?;
    }
    for intent in intents {
        render_intent(intent, writer)?;
    }
    if page.get("has_more").and_then(Value::as_bool) == Some(true)
        && let Some(cursor) = page.get("next_cursor")
    {
        let time = cursor
            .get("submitted_at_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let id = cursor
            .get("intent_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        writeln!(
            writer,
            "More Run-intents available: enter run-intents --before {time} {}.",
            super::state::json_text(id)
        )
        .map_err(super::state::io_error)?;
    }
    Ok(())
}

fn render_timeline<W: Write>(page: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let events = page
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RemoteError("Forge API returned an invalid pending Run-intent timeline".into())
        })?;
    if events.is_empty() {
        writeln!(
            writer,
            "No pending Run-intent timeline events on this page."
        )
        .map_err(super::state::io_error)?;
    }
    for event in events {
        let seq = event.get("seq").and_then(Value::as_u64).unwrap_or_default();
        let emitted_at_ms = event
            .get("emitted_at_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        writeln!(
            writer,
            "Run-intent event seq={seq} emitted_at_ms={emitted_at_ms} type={}",
            super::state::json_text(event_type)
        )
        .map_err(super::state::io_error)?;
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(writer, "Use run-intents, run-intents --before TIME INTENT_ID, run-intents timeline INTENT_ID [AFTER_SEQUENCE|--resume], or run-intents submit TEXT.")
        .map_err(super::state::io_error)
}

async fn list_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(ParsedCursor { before: cursor }) = parse_cursor(argument, writer)? else {
        return Ok(());
    };
    if !writes::ensure_pending_run_intent_visible_to_client_instance(
        state,
        conversation_id,
        writer,
    )? {
        return Ok(());
    }
    let page = match client
        .list_pending_run_intents(
            conversation_id,
            PAGE_LIMIT,
            cursor.as_ref().map(|(time, _)| *time),
            cursor.as_ref().map(|(_, id)| id.as_str()),
        )
        .await
    {
        Ok(page) => page,
        Err(error) => {
            report_read_failure(state, &error, "list", writer)?;
            return Ok(());
        }
    };
    render_page(&page, writer)?;
    // Remember only the exact owner/Conversation page the user explicitly
    // opened. `sync` may refresh this observation later, while an ordinary
    // session sync remains request-free for the pending-intent candidate.
    state.record_pending_run_intent_page(conversation_id, cursor);
    Ok(())
}

fn report_read_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    operation: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::clear_session_view_after_authorization_error(state, error);
    writeln!(writer, "Pending Run-intent {operation} failed: {error}")
        .map_err(super::state::io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(super::state::io_error)?;
    }
    Ok(())
}

fn record_timeline<W: Write>(
    state: &mut TuiState,
    conversation_id: &str,
    intent_id: String,
    page: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let scanned_through_sequence = page
        .get("scanned_through_sequence")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RemoteError("Forge API returned an invalid pending Run-intent timeline".into())
        })?;
    let same_binding = state.selected_pending_run_intent_conversation_id.as_deref()
        == Some(conversation_id)
        && state.selected_pending_run_intent_id.as_deref() == Some(intent_id.as_str());
    if same_binding && scanned_through_sequence < state.pending_run_intent_timeline_sequence {
        writeln!(
            writer,
            "Pending Run-intent timeline cursor regressed; the cached cursor was not changed."
        )
        .map_err(super::state::io_error)?;
        return Ok(());
    }
    state.selected_pending_run_intent_conversation_id = Some(conversation_id.to_owned());
    state.selected_pending_run_intent_id = Some(intent_id);
    state.pending_run_intent_timeline_sequence = scanned_through_sequence;
    render_timeline(page, writer)
}

struct ParsedCursor {
    before: Option<(u64, String)>,
}

fn render_intent<W: Write>(intent: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let id = intent
        .get("intent_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let status = intent
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let profile = intent
        .get("profile_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let submitted_at_ms = intent
        .get("submitted_at_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let latest_sequence = intent
        .get("latest_sequence")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    writeln!(
        writer,
        "Run-intent {} status={} profile={} submitted_at_ms={submitted_at_ms} latest_sequence={latest_sequence}",
        super::state::json_text(id),
        super::state::json_text(status),
        super::state::json_text(profile),
    )
    .map_err(super::state::io_error)?;
    Ok(())
}

fn write_cursor_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use run-intents with no argument, or run-intents --before TIME INTENT_ID."
    )
    .map_err(super::state::io_error)
}
