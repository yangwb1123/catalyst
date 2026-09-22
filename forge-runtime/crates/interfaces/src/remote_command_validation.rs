use std::{collections::HashSet, net::IpAddr};

use reqwest::{Response, Url};
use serde_json::Value;

use super::{
    OwnedConversationEntry, OwnedConversationPage, OwnedRunPageResponse, OwnedRunSummaryResponse,
    OwnedRunTimelinePageResponse, RemoteError,
};

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const RUN_PAGE_SIZE: usize = 25;
const RUN_TIMELINE_PAGE_SIZE: usize = 128;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn required_idempotency_key(key: Option<&str>) -> Result<&str, RemoteError> {
    let value = key
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RemoteError("remote writes require --idempotency-key".into()))?;
    if value.trim() != value || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(RemoteError("idempotency key is invalid".into()));
    }
    Ok(value)
}

pub(super) fn validate_conversation_id(value: &str) -> Result<(), RemoteError> {
    if value.trim().is_empty()
        || value.len() > 128
        || value.chars().any(char::is_control)
        || value.contains('/')
    {
        return Err(RemoteError("conversation id is invalid".into()));
    }
    Ok(())
}

pub(super) fn parse_api_url(value: &str) -> Result<Url, RemoteError> {
    let url = Url::parse(value).map_err(|_| RemoteError("FORGE_API_URL is invalid".into()))?;
    let local_http = url.scheme() == "http" && is_loopback_host(&url);
    if (url.scheme() != "https" && !local_http)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(RemoteError(
            "FORGE_API_URL must be an HTTPS origin (HTTP is allowed on loopback)".into(),
        ));
    }
    Ok(url)
}

pub(super) fn is_loopback_host(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        let host = host
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .unwrap_or(host);
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    })
}

pub(super) async fn read_json_response(response: Response) -> Result<Value, RemoteError> {
    let status = response.status();
    let mut response = response;
    let mut body = Vec::new();
    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(_) if !status.is_success() => return Err(http_status_error(status, &body)),
            Err(_) => return Err(RemoteError("Forge API response could not be read".into())),
        };
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            if !status.is_success() {
                return Err(http_status_error(status, &[]));
            }
            return Err(RemoteError(
                "Forge API response exceeded the size limit".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        return Err(http_status_error(status, &body));
    }
    crate::device_json_unique::reject_duplicate_keys(&body)
        .map_err(|_| RemoteError("Forge API returned duplicate JSON keys".into()))?;
    serde_json::from_slice(&body).map_err(|_| RemoteError("Forge API returned invalid JSON".into()))
}

pub(super) fn http_status_error(status: reqwest::StatusCode, body: &[u8]) -> RemoteError {
    let payload = serde_json::from_slice::<Value>(body).ok();
    let code = payload
        .as_ref()
        .and_then(|payload| payload.get("code"))
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
        .unwrap_or("request_failed");
    RemoteError(format!(
        "Forge API returned HTTP {} ({code})",
        status.as_u16()
    ))
}

pub(super) fn validate_conversation_page(
    page: &OwnedConversationPage,
    after_id: Option<&str>,
) -> Result<(), RemoteError> {
    if page.conversations.len() > 128
        || (page.has_more && page.conversations.len() != 128)
        || (page.has_more && page.next_after_id.is_none())
        || (!page.has_more && page.next_after_id.is_some())
    {
        return Err(RemoteError(
            "Forge API returned an invalid conversation page".into(),
        ));
    }
    let mut previous_id = after_id;
    for entry in &page.conversations {
        if entry.aggregate_version == 0
            || entry.aggregate_version > MAX_SAFE_INTEGER
            || !is_valid_conversation_projection(&entry.conversation)
        {
            return Err(RemoteError(
                "Forge API returned an invalid conversation page".into(),
            ));
        }
        let Some(id) = entry.conversation.get("id").and_then(Value::as_str) else {
            return Err(RemoteError(
                "Forge API returned an invalid conversation page".into(),
            ));
        };
        validate_conversation_id(id)
            .map_err(|_| RemoteError("Forge API returned an invalid conversation page".into()))?;
        if previous_id.is_some_and(|previous| previous >= id) {
            return Err(RemoteError(
                "Forge API returned an invalid conversation page".into(),
            ));
        }
        previous_id = Some(id);
    }
    if page.has_more
        && page.next_after_id.as_deref()
            != page
                .conversations
                .last()
                .and_then(|entry| entry.conversation.get("id"))
                .and_then(Value::as_str)
    {
        return Err(RemoteError(
            "Forge API returned an invalid conversation page".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_owned_conversation_entry(
    data: &Value,
    entry: &OwnedConversationEntry,
) -> Result<(), RemoteError> {
    let object = data
        .as_object()
        .ok_or_else(|| RemoteError("Forge API returned an invalid conversation detail".into()))?;
    if object.len() != 2
        || !object.contains_key("conversation")
        || !object.contains_key("aggregate_version")
        || entry.aggregate_version == 0
        || entry.aggregate_version > MAX_SAFE_INTEGER
        || !is_valid_conversation_projection(&entry.conversation)
    {
        return Err(RemoteError(
            "Forge API returned an invalid conversation detail".into(),
        ));
    }
    Ok(())
}

fn is_valid_conversation_projection(conversation: &Value) -> bool {
    let Some(conversation) = conversation.as_object() else {
        return false;
    };
    if conversation.len() != 5
        || !["id", "scope", "title", "created_at_ms", "updated_at_ms"]
            .iter()
            .all(|key| conversation.contains_key(*key))
        || conversation
            .get("id")
            .and_then(Value::as_str)
            .is_none_or(|id| validate_conversation_id(id).is_err())
        || conversation
            .get("title")
            .and_then(Value::as_str)
            .is_none_or(|title| title.trim().is_empty())
    {
        return false;
    }
    let Some(created_at_ms) = conversation.get("created_at_ms").and_then(Value::as_u64) else {
        return false;
    };
    let Some(updated_at_ms) = conversation.get("updated_at_ms").and_then(Value::as_u64) else {
        return false;
    };
    if created_at_ms > updated_at_ms
        || created_at_ms > MAX_SAFE_INTEGER
        || updated_at_ms > MAX_SAFE_INTEGER
    {
        return false;
    }
    let Some(scope) = conversation.get("scope").and_then(Value::as_object) else {
        return false;
    };
    match scope.get("kind").and_then(Value::as_str) {
        Some("global") => scope.len() == 1,
        Some("project" | "group") => {
            scope.len() == 2
                && scope
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| validate_entity_id(id, "scope").is_ok())
        }
        _ => false,
    }
}

pub(super) fn validate_run_page_request(
    limit: usize,
    before_created_at_ms: Option<u64>,
    before_run_id: Option<&str>,
) -> Result<(), RemoteError> {
    if limit == 0
        || limit > RUN_PAGE_SIZE
        || before_created_at_ms.is_some() != before_run_id.is_some()
        || before_created_at_ms.is_some_and(|value| value > MAX_SAFE_INTEGER)
    {
        return Err(RemoteError("Run page request is invalid".into()));
    }
    if let Some(run_id) = before_run_id {
        validate_entity_id(run_id, "Run")?;
    }
    Ok(())
}

pub(super) fn validate_timeline_request(
    after_sequence: u64,
    limit: usize,
) -> Result<(), RemoteError> {
    if after_sequence > MAX_SAFE_INTEGER || limit == 0 || limit > RUN_TIMELINE_PAGE_SIZE {
        return Err(RemoteError("Run timeline request is invalid".into()));
    }
    Ok(())
}

pub(super) fn validate_run_page(
    page: &OwnedRunPageResponse,
    conversation_id: &str,
    limit: usize,
    before_created_at_ms: Option<u64>,
    before_run_id: Option<&str>,
) -> Result<(), String> {
    validate_run_page_shape(page, conversation_id, limit)?;
    let mut previous: Option<&OwnedRunSummaryResponse> = None;
    let mut run_ids = HashSet::with_capacity(page.runs.len());
    for run in &page.runs {
        if !run_ids.insert(run.run_id.as_str()) {
            return Err(invalid_run_page());
        }
        validate_run_summary(run, previous, before_created_at_ms, before_run_id)?;
        previous = Some(run);
    }
    validate_run_page_cursor(page)
}

fn validate_run_page_shape(
    page: &OwnedRunPageResponse,
    conversation_id: &str,
    limit: usize,
) -> Result<(), String> {
    if page.conversation_id != conversation_id
        || page.runs.len() > limit
        || (page.has_more && page.runs.is_empty())
        || (page.has_more && page.next_cursor.is_none())
        || (!page.has_more && page.next_cursor.is_some())
    {
        return Err(invalid_run_page());
    }
    Ok(())
}

fn validate_run_summary(
    run: &OwnedRunSummaryResponse,
    previous: Option<&OwnedRunSummaryResponse>,
    before_created_at_ms: Option<u64>,
    before_run_id: Option<&str>,
) -> Result<(), String> {
    if validate_entity_id(&run.run_id, "Run").is_err()
        || validate_entity_id(&run.prompt_id, "Prompt").is_err()
        || run.created_at_ms > MAX_SAFE_INTEGER
        || run.latest_sequence == 0
        || run.latest_sequence > MAX_SAFE_INTEGER
        || previous.is_some_and(|older| {
            older.created_at_ms < run.created_at_ms
                || (older.created_at_ms == run.created_at_ms && older.run_id <= run.run_id)
        })
        || is_after_run_cursor(run, before_created_at_ms, before_run_id)
    {
        return Err(invalid_run_page());
    }
    Ok(())
}

fn is_after_run_cursor(
    run: &OwnedRunSummaryResponse,
    before_created_at_ms: Option<u64>,
    before_run_id: Option<&str>,
) -> bool {
    before_created_at_ms.is_some_and(|cursor_time| {
        run.created_at_ms > cursor_time
            || (run.created_at_ms == cursor_time
                && before_run_id.is_some_and(|cursor_id| run.run_id.as_str() >= cursor_id))
    })
}

fn validate_run_page_cursor(page: &OwnedRunPageResponse) -> Result<(), String> {
    if let Some(cursor) = &page.next_cursor {
        let matches_last = page.runs.last().is_some_and(|last| {
            last.created_at_ms == cursor.created_at_ms && last.run_id == cursor.run_id
        });
        if validate_entity_id(&cursor.run_id, "Run").is_err()
            || cursor.created_at_ms > MAX_SAFE_INTEGER
            || !matches_last
        {
            return Err(invalid_run_page());
        }
    }
    Ok(())
}

fn invalid_run_page() -> String {
    "Forge API returned an invalid Run page".into()
}

pub(super) fn validate_run_timeline(
    page: &OwnedRunTimelinePageResponse,
    conversation_id: &str,
    run_id: &str,
    after_sequence: u64,
    limit: usize,
) -> Result<(), String> {
    if page.conversation_id != conversation_id
        || page.run_id != run_id
        || page.after_sequence != after_sequence
        || page.events.len() > limit
        || (page.has_more && page.events.is_empty())
        || page.scanned_through_sequence > MAX_SAFE_INTEGER
        || page.scanned_through_sequence < after_sequence
    {
        return Err("Forge API returned an invalid Run timeline".into());
    }
    let mut previous_sequence = after_sequence;
    for event in &page.events {
        let Some(expected_sequence) = previous_sequence.checked_add(1) else {
            return Err("Forge API returned an invalid Run timeline".into());
        };
        if event.seq != expected_sequence
            || event.seq > MAX_SAFE_INTEGER
            || event.emitted_at_ms > MAX_SAFE_INTEGER
        {
            return Err("Forge API returned an invalid Run timeline".into());
        }
        previous_sequence = event.seq;
    }
    if page.scanned_through_sequence != previous_sequence {
        return Err("Forge API returned an invalid Run timeline".into());
    }
    Ok(())
}

pub(super) fn validate_entity_id(value: &str, entity_name: &str) -> Result<(), RemoteError> {
    if value.trim().is_empty()
        || value.len() > 128
        || value.chars().any(char::is_control)
        || value.contains('/')
    {
        return Err(RemoteError(format!("{entity_name} id is invalid")));
    }
    Ok(())
}
