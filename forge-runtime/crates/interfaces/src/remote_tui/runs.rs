use std::io::Write;

use serde_json::Value;

use super::{RemoteClient, RemoteError, state::TuiState};

const RUN_PAGE_LIMIT: usize = 25;
pub(super) const RUN_TIMELINE_LIMIT: usize = 128;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
type ParsedRunCursor = Option<(Option<u64>, Option<String>)>;

pub(super) async fn runs_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(conversation_id) = selected_conversation_id(state, writer)? else {
        return Ok(());
    };
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        conversation_id,
        writer,
    )? {
        return Ok(());
    }
    let Some((before_created_at_ms, before_run_id)) = parse_run_cursor(argument, writer)? else {
        return Ok(());
    };
    let page = match client
        .list_runs(
            conversation_id,
            RUN_PAGE_LIMIT,
            before_created_at_ms,
            before_run_id.as_deref(),
        )
        .await
    {
        Ok(page) => page,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Run list failed: {error}").map_err(super::state::io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(super::state::io_error)?;
            }
            return Ok(());
        }
    };
    render_run_page(&page, writer)
}

pub(super) async fn timeline_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(conversation_id) = selected_conversation_id(state, writer)? else {
        return Ok(());
    };
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        conversation_id,
        writer,
    )? {
        return Ok(());
    }
    let Some((run_id, after_sequence, resume)) = parse_timeline_cursor(argument, writer)? else {
        return Ok(());
    };
    let page = match if resume {
        client
            .resumed_run_timeline(conversation_id, &run_id, RUN_TIMELINE_LIMIT)
            .await
    } else {
        client
            .run_timeline(conversation_id, &run_id, after_sequence, RUN_TIMELINE_LIMIT)
            .await
    } {
        Ok(page) => page,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Run timeline failed: {error}").map_err(super::state::io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(super::state::io_error)?;
            }
            return Ok(());
        }
    };
    let scanned_through_sequence = page
        .get("scanned_through_sequence")
        .and_then(Value::as_u64)
        .ok_or_else(|| RemoteError("Forge API returned an invalid Run timeline".into()))?;
    state.selected_run_id = Some(run_id);
    state.run_timeline_sequence = scanned_through_sequence;
    render_timeline(&page, writer)
}

pub(super) fn selected_conversation_id<'a, W: Write>(
    state: &'a TuiState,
    writer: &mut W,
) -> Result<Option<&'a str>, RemoteError> {
    let Some(conversation_id) = state.selected_id.as_deref() else {
        writeln!(writer, "Open a session before viewing its Runs.")
            .map_err(super::state::io_error)?;
        return Ok(None);
    };
    if state.selected_conversation().is_none() {
        writeln!(
            writer,
            "Refresh or open the selected session before viewing its Runs."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    }
    Ok(Some(conversation_id))
}

fn parse_run_cursor<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<ParsedRunCursor, RemoteError> {
    if argument.trim().is_empty() {
        return Ok(Some((None, None)));
    }
    let Some((flag, remainder)) = argument.split_once(char::is_whitespace) else {
        writeln!(
            writer,
            "Use runs with no argument, or runs --before TIME RUN_ID."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    };
    if flag != "--before" {
        writeln!(
            writer,
            "Use runs with no argument, or runs --before TIME RUN_ID."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    }
    let Some((timestamp, run_id_argument)) = remainder.trim_start().split_once(char::is_whitespace)
    else {
        writeln!(writer, "Run cursor requires a time and Run ID.")
            .map_err(super::state::io_error)?;
        return Ok(None);
    };
    let Some(timestamp) = timestamp
        .parse::<u64>()
        .ok()
        .filter(|value| *value <= MAX_SAFE_JSON_INTEGER)
    else {
        writeln!(writer, "Run cursor time must be an unsigned integer.")
            .map_err(super::state::io_error)?;
        return Ok(None);
    };
    let Some(run_id) = parse_run_id_argument(run_id_argument.trim()) else {
        writeln!(writer, "Run cursor ID must be one ID or a JSON string.")
            .map_err(super::state::io_error)?;
        return Ok(None);
    };
    Ok(Some((Some(timestamp), Some(run_id))))
}

fn parse_timeline_cursor<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<Option<(String, u64, bool)>, RemoteError> {
    let Some((run_id, remainder)) = split_run_id_argument(argument.trim()) else {
        writeln!(writer, "Use timeline RUN_ID [AFTER_SEQUENCE|--resume].")
            .map_err(super::state::io_error)?;
        return Ok(None);
    };
    let after_sequence = match remainder.trim() {
        "" => 0,
        "--resume" => 0,
        value => {
            if let Some(value) = value
                .parse::<u64>()
                .ok()
                .filter(|value| *value <= MAX_SAFE_JSON_INTEGER)
            {
                value
            } else {
                writeln!(
                    writer,
                    "Timeline cursor must be an unsigned integer or --resume."
                )
                .map_err(super::state::io_error)?;
                return Ok(None);
            }
        }
    };
    Ok(Some((
        run_id,
        after_sequence,
        remainder.trim() == "--resume",
    )))
}

pub(super) fn parse_run_id_argument(value: &str) -> Option<String> {
    if value.starts_with('"') {
        serde_json::from_str(value).ok()
    } else if value.chars().any(char::is_whitespace) {
        None
    } else {
        Some(value.to_owned())
    }
}

fn split_run_id_argument(value: &str) -> Option<(String, &str)> {
    if value.starts_with('"') {
        let end = json_string_end(value)?;
        let run_id = serde_json::from_str(&value[..end]).ok()?;
        Some((run_id, &value[end..]))
    } else {
        let (run_id, remainder) = value.split_once(char::is_whitespace).unwrap_or((value, ""));
        (!run_id.is_empty()).then(|| (run_id.to_owned(), remainder))
    }
}

fn json_string_end(value: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, character) in value.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(index + character.len_utf8());
        }
    }
    None
}

fn render_run_page<W: Write>(page: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let runs = page
        .get("runs")
        .and_then(Value::as_array)
        .ok_or_else(|| RemoteError("Forge API returned an invalid Run page".into()))?;
    if runs.is_empty() {
        writeln!(writer, "No Runs are recorded for this session.")
            .map_err(super::state::io_error)?;
    }
    for run in runs {
        render_run_summary(run, writer)?;
    }
    render_run_page_cursor(page, writer)
}

fn render_run_summary<W: Write>(run: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let run_id = run
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let status = run
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let sequence = run
        .get("latest_sequence")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let created_at_ms = run
        .get("created_at_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    writeln!(
        writer,
        "Run {} status={} latest_sequence={} created_at_ms={created_at_ms}",
        super::state::json_text(run_id),
        super::state::json_text(status),
        sequence
    )
    .map_err(super::state::io_error)
}

fn render_run_page_cursor<W: Write>(page: &Value, writer: &mut W) -> Result<(), RemoteError> {
    if page.get("has_more").and_then(Value::as_bool) == Some(true)
        && let Some(cursor) = page.get("next_cursor")
    {
        let time = cursor
            .get("created_at_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let run_id = cursor
            .get("run_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        writeln!(
            writer,
            "More Runs available: enter runs --before {time} {}.",
            super::state::json_text(run_id)
        )
        .map_err(super::state::io_error)?;
    }
    Ok(())
}

pub(super) fn render_timeline<W: Write>(page: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let events = page
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| RemoteError("Forge API returned an invalid Run timeline".into()))?;
    if events.is_empty() {
        writeln!(writer, "No Run timeline events on this page.").map_err(super::state::io_error)?;
    }
    for event in events {
        let sequence = event.get("seq").and_then(Value::as_u64).unwrap_or_default();
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
            "Event seq={sequence} emitted_at_ms={emitted_at_ms} type={}",
            super::state::json_text(event_type)
        )
        .map_err(super::state::io_error)?;
    }
    if page.get("has_more").and_then(Value::as_bool) == Some(true) {
        let run_id = page
            .get("run_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let cursor = page
            .get("scanned_through_sequence")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        writeln!(
            writer,
            "More events available: enter timeline {} {cursor}.",
            super::state::json_text(run_id)
        )
        .map_err(super::state::io_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_run_cursor, parse_timeline_cursor};

    #[test]
    fn tui_run_cursors_reject_values_above_json_safe_integer() {
        let mut writer = Vec::new();
        assert!(
            parse_run_cursor("--before 9007199254740992 run-1", &mut writer)
                .unwrap()
                .is_none()
        );
        assert!(
            parse_timeline_cursor("run-1 9007199254740992", &mut writer)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn timeline_parser_keeps_manual_cursor_and_explicit_resume_distinct() {
        let mut output = Vec::new();
        assert_eq!(
            parse_timeline_cursor("run-1", &mut output).unwrap(),
            Some(("run-1".into(), 0, false))
        );
        assert_eq!(
            parse_timeline_cursor("run-1 9", &mut output).unwrap(),
            Some(("run-1".into(), 9, false))
        );
        assert_eq!(
            parse_timeline_cursor("run-1 --resume", &mut output).unwrap(),
            Some(("run-1".into(), 0, true))
        );
        assert!(
            parse_timeline_cursor("run-1 not-a-cursor", &mut output)
                .unwrap()
                .is_none()
        );
    }
}
