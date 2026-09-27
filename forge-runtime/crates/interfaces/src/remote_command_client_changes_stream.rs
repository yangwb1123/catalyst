use std::str;

use futures_util::StreamExt;
use reqwest::{Response, header};
use serde::Serialize;
use serde_json::Value;

use super::super::changes::{OwnedConversationChange, OwnedConversationChangePage};
use super::{RemoteClient, RemoteError};

const CHANGE_PAGE_SIZE: usize = 128;
const MAX_STREAM_WAIT_MS: u64 = 10_000;
const MAX_STREAM_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnedConversationChangeStreamResult {
    start_cursor: u64,
    scanned_through_cursor: u64,
    timed_out: bool,
    has_more: bool,
    changes: Vec<OwnedConversationChange>,
}

impl RemoteClient {
    /// Reads one explicitly opt-in owner-bound SSE change page.
    ///
    /// The regular `conversation_changes_after` polling path remains the
    /// default. This method accepts only the Core's single-frame stream,
    /// validates the event/id/page binding, and advances a saved cursor only
    /// when the caller did not supply an explicit cursor.
    pub(crate) async fn stream_conversation_changes(
        &self,
        explicit_after: Option<u64>,
        wait_ms: u64,
    ) -> Result<Value, RemoteError> {
        let start_cursor = explicit_after.map_or_else(|| self.saved_change_cursor(), Ok)?;
        if start_cursor > MAX_SAFE_JSON_INTEGER || wait_ms > MAX_STREAM_WAIT_MS {
            return Err(RemoteError(
                "conversation change stream bounds are invalid".into(),
            ));
        }
        let access_token = match &self.token_refresh {
            Some(provider) => provider.access_token().await?,
            None => self.access_token.clone(),
        };
        let response = self
            .http
            .get(self.endpoint("/api/v1/conversation-changes/stream")?)
            .query(&[
                ("after_cursor", start_cursor.to_string()),
                ("limit", CHANGE_PAGE_SIZE.to_string()),
                ("wait_ms", wait_ms.to_string()),
            ])
            .header(header::ACCEPT, "text/event-stream")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|_| RemoteError("Forge API request failed".into()))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(';')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase()
            });
        let body = read_stream_body(response).await?;

        if status == reqwest::StatusCode::NO_CONTENT {
            if !body.is_empty() {
                return Err(RemoteError(
                    "Forge API returned a non-empty conversation change stream timeout".into(),
                ));
            }
            return serde_json::to_value(OwnedConversationChangeStreamResult {
                start_cursor,
                scanned_through_cursor: start_cursor,
                timed_out: true,
                has_more: false,
                changes: Vec::new(),
            })
            .map_err(|_| RemoteError("conversation change stream could not be encoded".into()));
        }
        if !status.is_success() {
            return Err(super::super::validation::http_status_error(status, &body));
        }
        if status != reqwest::StatusCode::OK || content_type.as_deref() != Some("text/event-stream")
        {
            return Err(RemoteError(
                "Forge API returned an invalid conversation change stream response".into(),
            ));
        }

        let page = parse_stream_page(&body, start_cursor)?;
        if explicit_after.is_none() && page.scanned_through_cursor > start_cursor {
            self.persist_change_cursor(page.scanned_through_cursor)?;
        }
        serde_json::to_value(OwnedConversationChangeStreamResult {
            start_cursor,
            scanned_through_cursor: page.scanned_through_cursor,
            timed_out: false,
            has_more: page.has_more,
            changes: page.changes,
        })
        .map_err(|_| RemoteError("conversation change stream could not be encoded".into()))
    }
}

async fn read_stream_body(response: Response) -> Result<Vec<u8>, RemoteError> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| RemoteError("Forge API response could not be read".into()))?;
        if body.len().saturating_add(chunk.len()) > MAX_STREAM_RESPONSE_BYTES {
            return Err(RemoteError(
                "Forge API response exceeded the size limit".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn parse_stream_page(
    body: &[u8],
    requested_after: u64,
) -> Result<OwnedConversationChangePage, RemoteError> {
    let body = str::from_utf8(body)
        .map_err(|_| RemoteError("Forge API returned invalid UTF-8 SSE data".into()))?;
    let normalized = body.replace("\r\n", "\n");
    if normalized.contains('\r') || !normalized.ends_with("\n\n") {
        return Err(RemoteError(
            "Forge API returned malformed conversation SSE".into(),
        ));
    }
    let frame = &normalized[..normalized.len() - 2];
    let lines: Vec<&str> = frame.split('\n').collect();
    if lines.is_empty() || lines.iter().any(|line| line.is_empty()) {
        return Err(RemoteError(
            "Forge API returned malformed conversation SSE".into(),
        ));
    }

    let mut event = None;
    let mut id = None;
    let mut data = None;
    for line in lines {
        let Some((field, raw_value)) = line.split_once(':') else {
            return Err(RemoteError(
                "Forge API returned malformed conversation SSE".into(),
            ));
        };
        if field.is_empty() || !matches!(field, "event" | "id" | "data") {
            return Err(RemoteError(
                "Forge API returned malformed conversation SSE".into(),
            ));
        }
        let value = raw_value.strip_prefix(' ').unwrap_or(raw_value);
        match field {
            "event" if event.is_none() => event = Some(value),
            "id" if id.is_none() => id = Some(value),
            "data" if data.is_none() => data = Some(value),
            _ => {
                return Err(RemoteError(
                    "Forge API returned malformed conversation SSE".into(),
                ));
            }
        }
    }
    if event != Some("conversation_changes") {
        return Err(RemoteError(
            "Forge API returned an unknown conversation SSE event".into(),
        ));
    }
    let id =
        id.ok_or_else(|| RemoteError("Forge API returned an incomplete conversation SSE".into()))?;
    let scanned_cursor = parse_stream_id(id)?;
    let data = data
        .ok_or_else(|| RemoteError("Forge API returned an incomplete conversation SSE".into()))?;
    crate::device_json_unique::reject_duplicate_keys(data.as_bytes())
        .map_err(|_| RemoteError("Forge API returned duplicate JSON keys".into()))?;
    let page: OwnedConversationChangePage = serde_json::from_str(data)
        .map_err(|_| RemoteError("Forge API returned invalid conversation SSE JSON".into()))?;
    page.validate(requested_after, CHANGE_PAGE_SIZE)
        .map_err(RemoteError)?;
    if page.scanned_through_cursor != scanned_cursor {
        return Err(RemoteError(
            "Forge conversation SSE cursor does not match its page".into(),
        ));
    }
    Ok(page)
}

fn parse_stream_id(value: &str) -> Result<u64, RemoteError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RemoteError(
            "Forge API returned an invalid conversation SSE id".into(),
        ));
    }
    let parsed = value
        .parse::<u64>()
        .ok()
        .filter(|value| *value <= MAX_SAFE_JSON_INTEGER)
        .ok_or_else(|| RemoteError("Forge API returned an invalid conversation SSE id".into()))?;
    if parsed.to_string() != value {
        return Err(RemoteError(
            "Forge API returned an invalid conversation SSE id".into(),
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::parse_stream_page;

    #[test]
    fn stream_parser_rejects_duplicate_fields_and_cursor_drift() {
        let data = r#"{"after_cursor":4,"scanned_through_cursor":5,"has_more":false,"changes":[{"cursor":5,"schema_version":1,"conversation_id":"c-1","entity_id":"p-1","aggregate_version":1,"kind":"prompt_appended","created_at_ms":1}]}"#;
        let valid = format!("event: conversation_changes\nid: 5\ndata: {data}\n\n");
        assert!(parse_stream_page(valid.as_bytes(), 4).is_ok());
        assert!(parse_stream_page(
            format!("event: conversation_changes\nevent: conversation_changes\nid: 5\ndata: {data}\n\n").as_bytes(),
            4
        )
        .is_err());
        assert!(
            parse_stream_page(
                format!("event: conversation_changes\nid: 4\ndata: {data}\n\n").as_bytes(),
                4
            )
            .is_err()
        );
    }

    #[test]
    fn stream_parser_rejects_unsafe_and_noncanonical_ids() {
        let data = r#"{"after_cursor":0,"scanned_through_cursor":0,"has_more":false,"changes":[]}"#;
        for id in ["01", "9007199254740992", "x"] {
            let frame = format!("event: conversation_changes\nid: {id}\ndata: {data}\n\n");
            assert!(parse_stream_page(frame.as_bytes(), 0).is_err(), "{id}");
        }
    }
}
