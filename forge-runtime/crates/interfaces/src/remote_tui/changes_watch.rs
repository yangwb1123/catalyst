use std::io::Write;

use serde::Deserialize;
use serde_json::Value;

use super::super::super::changes::OwnedConversationChange;
use super::state::{TuiState, io_error, json_text};
use super::{RemoteClient, RemoteError};

const DEFAULT_POLLS: usize = 8;
const DEFAULT_MIN_DELAY_MS: u64 = 250;
const DEFAULT_MAX_DELAY_MS: u64 = 5_000;
const MAX_POLLS: usize = 64;
const MAX_MIN_DELAY_MS: u64 = 10_000;
const MAX_MAX_DELAY_MS: u64 = 60_000;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy)]
struct WatchOptions {
    after_cursor: Option<u64>,
    polls: usize,
    min_delay_ms: u64,
    max_delay_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WatchResult {
    start_cursor: u64,
    scanned_through_cursor: u64,
    polls: usize,
    has_more: bool,
    changes: Vec<OwnedConversationChange>,
}

/// Consumes the same bounded owner-feed watch exposed by the standalone
/// `remote changes watch` command. The TUI only renders validated metadata and
/// updates its in-process cursor; the underlying client owns saved-cursor
/// persistence and explicit-cursor one-off semantics.
pub(super) async fn changes_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let mut tokens = argument.split_whitespace();
    if tokens.next() != Some("watch") {
        return write_usage(writer);
    }
    let options = match parse_options(tokens, writer)? {
        Some(options) => options,
        None => return Ok(()),
    };
    watch(client, state, options, writer).await
}

/// Short alias for interactive users; `changes watch` remains the documented
/// spelling so the TUI and standalone CLI expose the same command shape.
pub(super) async fn watch_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let mut tokens = argument.split_whitespace();
    let options = match parse_options(tokens.by_ref(), writer)? {
        Some(options) => options,
        None => return Ok(()),
    };
    if tokens.next().is_some() {
        return write_usage(writer);
    }
    watch(client, state, options, writer).await
}

fn parse_options<'a, I, W>(
    mut tokens: I,
    writer: &mut W,
) -> Result<Option<WatchOptions>, RemoteError>
where
    I: Iterator<Item = &'a str>,
    W: Write,
{
    let mut options = WatchOptions {
        after_cursor: None,
        polls: DEFAULT_POLLS,
        min_delay_ms: DEFAULT_MIN_DELAY_MS,
        max_delay_ms: DEFAULT_MAX_DELAY_MS,
    };
    let mut after_seen = false;
    let mut polls_seen = false;
    let mut min_seen = false;
    let mut max_seen = false;

    while let Some(option) = tokens.next() {
        match option {
            "--after-cursor" if !after_seen => {
                let Some(value) = tokens.next() else {
                    return usage_error(writer, "--after-cursor requires a value");
                };
                let Some(value) = parse_safe_u64(value) else {
                    return usage_error(
                        writer,
                        "--after-cursor must be a JSON-safe unsigned integer",
                    );
                };
                options.after_cursor = Some(value);
                after_seen = true;
            }
            "--polls" if !polls_seen => {
                let Some(value) = tokens.next() else {
                    return usage_error(writer, "--polls requires a value");
                };
                let Some(value) = parse_bounded_usize(value, 1, MAX_POLLS) else {
                    return usage_error(writer, "--polls must be between 1 and 64");
                };
                options.polls = value;
                polls_seen = true;
            }
            "--min-delay-ms" if !min_seen => {
                let Some(value) = tokens.next() else {
                    return usage_error(writer, "--min-delay-ms requires a value");
                };
                let Some(value) = parse_bounded_u64(value, 0, MAX_MIN_DELAY_MS) else {
                    return usage_error(writer, "--min-delay-ms must be between 0 and 10000");
                };
                options.min_delay_ms = value;
                min_seen = true;
            }
            "--max-delay-ms" if !max_seen => {
                let Some(value) = tokens.next() else {
                    return usage_error(writer, "--max-delay-ms requires a value");
                };
                let Some(value) = parse_bounded_u64(value, 0, MAX_MAX_DELAY_MS) else {
                    return usage_error(writer, "--max-delay-ms must be between 0 and 60000");
                };
                options.max_delay_ms = value;
                max_seen = true;
            }
            _ => return usage_error(writer, "unknown or duplicate changes watch option"),
        }
    }
    if options.max_delay_ms < options.min_delay_ms {
        return usage_error(
            writer,
            "--max-delay-ms must be greater than or equal to --min-delay-ms",
        );
    }
    Ok(Some(options))
}

fn parse_safe_u64(value: &str) -> Option<u64> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value <= MAX_SAFE_JSON_INTEGER)
}

fn parse_bounded_u64(value: &str, min: u64, max: u64) -> Option<u64> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| (*value >= min) && (*value <= max))
}

fn parse_bounded_usize(value: &str, min: u64, max: usize) -> Option<usize> {
    parse_bounded_u64(value, min, max as u64).and_then(|value| usize::try_from(value).ok())
}

fn usage_error<W: Write>(
    writer: &mut W,
    message: &str,
) -> Result<Option<WatchOptions>, RemoteError> {
    writeln!(writer, "Changes watch option error: {message}.").map_err(io_error)?;
    write_usage(writer)?;
    Ok(None)
}

async fn watch<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    options: WatchOptions,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let expected_start = match options.after_cursor {
        Some(cursor) => cursor,
        None => client.saved_change_cursor()?,
    };
    let response = match client
        .watch_conversation_changes(
            options.after_cursor,
            options.polls,
            options.min_delay_ms,
            options.max_delay_ms,
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            writeln!(
                writer,
                "Changes watch request failed: {error}. The in-memory TUI cursor was not advanced."
            )
            .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    let result = match decode_result(&response, expected_start, options.polls) {
        Ok(result) => result,
        Err(error) => {
            writeln!(writer, "Changes watch response failed validation: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };

    for change in &result.changes {
        super::super::sync::apply_conversation_change(state, change);
    }
    state.change_cursor = result.scanned_through_cursor;

    let checkpoint = if options.after_cursor.is_some() {
        "one-off; saved checkpoint unchanged"
    } else {
        "saved checkpoint advanced per valid page"
    };
    writeln!(
        writer,
        "Changes watch start_cursor={} scanned_through_cursor={} polls={} changes={} has_more={} ({checkpoint}).",
        result.start_cursor,
        result.scanned_through_cursor,
        result.polls,
        result.changes.len(),
        result.has_more,
    )
    .map_err(io_error)?;
    for change in &result.changes {
        writeln!(
            writer,
            "  Change cursor={} conversation={} entity={} aggregate_version={} kind={} created_at_ms={}",
            change.cursor,
            json_text(&change.conversation_id),
            json_text(&change.entity_id),
            change.aggregate_version,
            json_text(&change.kind),
            change.created_at_ms,
        )
        .map_err(io_error)?;
    }
    if result.has_more {
        writeln!(
            writer,
            "  More owner-visible changes remain; run changes watch again to continue."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn decode_result(
    value: &Value,
    expected_start: u64,
    expected_polls: usize,
) -> Result<WatchResult, RemoteError> {
    let result: WatchResult = serde_json::from_value(value.clone())
        .map_err(|_| RemoteError("Forge API returned an invalid change watch".into()))?;
    if result.start_cursor != expected_start
        || result.polls != expected_polls
        || result.scanned_through_cursor < result.start_cursor
    {
        return Err(RemoteError(
            "Forge API returned an invalid change watch".into(),
        ));
    }
    let mut previous = result.start_cursor;
    for change in &result.changes {
        let expected = previous
            .checked_add(1)
            .ok_or_else(|| RemoteError("Forge API returned an invalid change watch".into()))?;
        if change.cursor != expected || change.cursor > result.scanned_through_cursor {
            return Err(RemoteError(
                "Forge API returned an invalid change watch".into(),
            ));
        }
        previous = change.cursor;
    }
    if previous != result.scanned_through_cursor {
        return Err(RemoteError(
            "Forge API returned an invalid change watch".into(),
        ));
    }
    Ok(result)
}

fn write_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use changes watch [--after-cursor N] [--polls 1..64] [--min-delay-ms 0..10000] [--max-delay-ms 0..60000]."
    )
    .map_err(io_error)
}
