use std::io::Write;

use serde::Deserialize;

use crate::client_instance_session_scope;

use super::super::super::changes::OwnedConversationChange;
use super::state::{TuiState, io_error};
use super::{RemoteClient, RemoteError};

const DEFAULT_POLLS: usize = 8;
const DEFAULT_MIN_DELAY_MS: u64 = 250;
const DEFAULT_MAX_DELAY_MS: u64 = 5_000;
const MAX_POLLS: usize = 64;
const MAX_MIN_DELAY_MS: u64 = 10_000;
const MAX_MAX_DELAY_MS: u64 = 60_000;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[path = "changes_feed.rs"]
mod feed;

#[derive(Clone, Debug)]
struct ListOptions {
    after_cursor: Option<u64>,
    instance_id: Option<String>,
}

#[derive(Clone, Debug)]
struct WatchOptions {
    after_cursor: Option<u64>,
    polls: usize,
    min_delay_ms: u64,
    max_delay_ms: u64,
    instance_id: Option<String>,
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

#[derive(Clone, Debug)]
struct StreamOptions {
    after_cursor: Option<u64>,
    wait_ms: u64,
    instance_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamResult {
    start_cursor: u64,
    scanned_through_cursor: u64,
    timed_out: bool,
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
    match tokens.next() {
        Some("list") => {
            let options = match parse_list_options(tokens, writer)? {
                Some(options) => options,
                None => return Ok(()),
            };
            feed::list(client, state, options, writer).await
        }
        Some("watch") => {
            let options = match parse_options(tokens, writer)? {
                Some(options) => options,
                None => return Ok(()),
            };
            feed::watch(client, state, options, writer).await
        }
        Some("stream") => {
            let options = match parse_stream_options(tokens, writer)? {
                Some(options) => options,
                None => return Ok(()),
            };
            feed::stream(client, state, options, writer).await
        }
        _ => write_usage(writer),
    }
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
    feed::watch(client, state, options, writer).await
}

fn parse_list_options<'a, I, W>(
    mut tokens: I,
    writer: &mut W,
) -> Result<Option<ListOptions>, RemoteError>
where
    I: Iterator<Item = &'a str>,
    W: Write,
{
    let mut options = ListOptions {
        after_cursor: None,
        instance_id: None,
    };
    let mut after_seen = false;
    let mut instance_seen = false;
    while let Some(option) = tokens.next() {
        match option {
            "--after-cursor" if !after_seen => {
                let Some(value) = tokens.next() else {
                    return list_usage_error(writer, "--after-cursor requires a value");
                };
                let Some(value) = parse_safe_u64(value) else {
                    return list_usage_error(
                        writer,
                        "--after-cursor must be a JSON-safe unsigned integer",
                    );
                };
                options.after_cursor = Some(value);
                after_seen = true;
            }
            "--instance" if !instance_seen => {
                let Some(value) = tokens.next() else {
                    return list_usage_error(writer, "--instance requires a value");
                };
                if client_instance_session_scope::validate_instance_id(value).is_err() {
                    return list_usage_error(
                        writer,
                        "--instance must be a valid client-instance id",
                    );
                }
                options.instance_id = Some(value.to_owned());
                instance_seen = true;
            }
            _ => return list_usage_error(writer, "unknown or duplicate changes list option"),
        }
    }
    Ok(Some(options))
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
        instance_id: None,
    };
    let mut after_seen = false;
    let mut polls_seen = false;
    let mut min_seen = false;
    let mut max_seen = false;
    let mut instance_seen = false;

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
            "--instance" if !instance_seen => {
                let Some(value) = tokens.next() else {
                    return usage_error(writer, "--instance requires a value");
                };
                if client_instance_session_scope::validate_instance_id(value).is_err() {
                    return usage_error(writer, "--instance must be a valid client-instance id");
                }
                options.instance_id = Some(value.to_owned());
                instance_seen = true;
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

fn parse_stream_options<'a, I, W>(
    mut tokens: I,
    writer: &mut W,
) -> Result<Option<StreamOptions>, RemoteError>
where
    I: Iterator<Item = &'a str>,
    W: Write,
{
    let mut options = StreamOptions {
        after_cursor: None,
        wait_ms: 5_000,
        instance_id: None,
    };
    let mut after_seen = false;
    let mut wait_seen = false;
    let mut instance_seen = false;
    while let Some(option) = tokens.next() {
        match option {
            "--after-cursor" if !after_seen => {
                let Some(value) = tokens.next() else {
                    return stream_usage_error(writer, "--after-cursor requires a value");
                };
                let Some(value) = parse_safe_u64(value) else {
                    return stream_usage_error(
                        writer,
                        "--after-cursor must be a JSON-safe unsigned integer",
                    );
                };
                options.after_cursor = Some(value);
                after_seen = true;
            }
            "--wait-ms" if !wait_seen => {
                let Some(value) = tokens.next() else {
                    return stream_usage_error(writer, "--wait-ms requires a value");
                };
                let Some(value) = parse_bounded_u64(value, 0, 10_000) else {
                    return stream_usage_error(writer, "--wait-ms must be between 0 and 10000");
                };
                options.wait_ms = value;
                wait_seen = true;
            }
            "--instance" if !instance_seen => {
                let Some(value) = tokens.next() else {
                    return stream_usage_error(writer, "--instance requires a value");
                };
                if client_instance_session_scope::validate_instance_id(value).is_err() {
                    return stream_usage_error(
                        writer,
                        "--instance must be a valid client-instance id",
                    );
                }
                options.instance_id = Some(value.to_owned());
                instance_seen = true;
            }
            _ => return stream_usage_error(writer, "unknown or duplicate changes stream option"),
        }
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

fn list_usage_error<W: Write>(
    writer: &mut W,
    message: &str,
) -> Result<Option<ListOptions>, RemoteError> {
    writeln!(writer, "Changes list option error: {message}.").map_err(io_error)?;
    write_usage(writer)?;
    Ok(None)
}

fn stream_usage_error<W: Write>(
    writer: &mut W,
    message: &str,
) -> Result<Option<StreamOptions>, RemoteError> {
    writeln!(writer, "Changes stream option error: {message}.").map_err(io_error)?;
    write_usage(writer)?;
    Ok(None)
}

fn write_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use changes list [--after-cursor N] [--instance INSTANCE_ID], changes watch [--after-cursor N] [--polls 1..64] [--min-delay-ms 0..10000] [--max-delay-ms 0..60000] [--instance INSTANCE_ID], or changes stream [--after-cursor N] [--wait-ms 0..10000] [--instance INSTANCE_ID]."
    )
    .map_err(io_error)
}
