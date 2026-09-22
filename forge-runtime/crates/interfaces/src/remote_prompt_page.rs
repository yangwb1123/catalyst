use std::collections::HashSet;

use crate::args::PromptPageCursor;

use serde_json::Value;

const PROMPT_PAGE_LIMIT: usize = 128;
const MAX_PROMPT_ID_BYTES: usize = 128;
const MAX_PROMPT_ROLE_BYTES: usize = 64;
const MAX_PROMPT_CONTENT_BYTES: usize = 256 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn validate_prompt_page(page: &Value, conversation_id: &str) -> Result<(), String> {
    let object = page.as_object().ok_or_else(invalid_prompt_page)?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "conversation_id" | "prompts" | "next_cursor" | "has_more"
        )
    }) || object.get("conversation_id").and_then(Value::as_str) != Some(conversation_id)
    {
        return Err(invalid_prompt_page());
    }
    let prompts = object
        .get("prompts")
        .and_then(Value::as_array)
        .ok_or_else(invalid_prompt_page)?;
    let has_more = object
        .get("has_more")
        .and_then(Value::as_bool)
        .ok_or_else(invalid_prompt_page)?;
    // The Hub can stop a page before the row limit when the next prompt would
    // exceed its content-byte budget. In that case a non-full page with
    // `has_more` and a cursor is a valid continuation.
    if prompts.len() > PROMPT_PAGE_LIMIT {
        return Err(invalid_prompt_page());
    }
    validate_prompt_entries(prompts, conversation_id)?;
    validate_prompt_cursor(object, prompts, has_more)
}

pub(super) fn validate_prompt_page_before(
    page: &Value,
    conversation_id: &str,
    before: Option<&PromptPageCursor>,
) -> Result<(), String> {
    validate_prompt_page(page, conversation_id)?;
    let Some(before) = before else {
        return Ok(());
    };
    let prompts = page
        .get("prompts")
        .and_then(Value::as_array)
        .ok_or_else(invalid_prompt_page)?;
    if prompts.iter().any(|prompt| {
        let created_at_ms = prompt.get("created_at_ms").and_then(Value::as_u64);
        let prompt_id = prompt.get("id").and_then(Value::as_str);
        !matches!((created_at_ms, prompt_id), (Some(time), Some(id))
            if time < before.created_at_ms || (time == before.created_at_ms && id < before.prompt_id.as_str()))
    }) {
        return Err(invalid_prompt_page());
    }
    Ok(())
}

fn validate_prompt_entries(prompts: &[Value], conversation_id: &str) -> Result<(), String> {
    let mut previous: Option<(u64, &str)> = None;
    let mut seen = HashSet::with_capacity(prompts.len());
    let mut content_bytes = 0_usize;
    for prompt in prompts {
        let (id, created_at_ms) = validate_prompt_entry(prompt, conversation_id)?;
        if !seen.insert(id) {
            return Err(invalid_prompt_page());
        }
        if previous.is_some_and(|(time, old_id)| {
            time < created_at_ms || (time == created_at_ms && old_id <= id)
        }) {
            return Err(invalid_prompt_page());
        }
        previous = Some((created_at_ms, id));
        content_bytes = content_bytes.saturating_add(
            prompt
                .get("content")
                .and_then(Value::as_str)
                .map_or(0, str::len),
        );
        if content_bytes > MAX_PROMPT_CONTENT_BYTES {
            return Err(invalid_prompt_page());
        }
    }
    Ok(())
}

fn validate_prompt_entry<'a>(
    prompt: &'a Value,
    conversation_id: &str,
) -> Result<(&'a str, u64), String> {
    let fields = ["id", "conversation_id", "role", "content", "created_at_ms"];
    let object = prompt.as_object().ok_or_else(invalid_prompt_page)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid_prompt_page());
    }
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_prompt_page)?;
    let role = object
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(invalid_prompt_page)?;
    let content = object
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(invalid_prompt_page)?;
    let created_at_ms = object
        .get("created_at_ms")
        .and_then(Value::as_u64)
        .filter(|value| *value <= MAX_SAFE_JSON_INTEGER)
        .ok_or_else(invalid_prompt_page)?;
    if object.get("conversation_id").and_then(Value::as_str) != Some(conversation_id)
        || !valid_prompt_id(id)
        || role.is_empty()
        || role.len() > MAX_PROMPT_ROLE_BYTES
        || content.len() > MAX_PROMPT_CONTENT_BYTES
    {
        return Err(invalid_prompt_page());
    }
    Ok((id, created_at_ms))
}

fn validate_prompt_cursor(
    page: &serde_json::Map<String, Value>,
    prompts: &[Value],
    has_more: bool,
) -> Result<(), String> {
    let Some(cursor) = page.get("next_cursor") else {
        return if has_more {
            Err(invalid_prompt_page())
        } else {
            Ok(())
        };
    };
    let Some(last) = prompts.last() else {
        return Err(invalid_prompt_page());
    };
    let cursor = cursor.as_object().ok_or_else(invalid_prompt_page)?;
    if !has_more
        || cursor.len() != 2
        || cursor.get("created_at_ms") != last.get("created_at_ms")
        || cursor.get("prompt_id") != last.get("id")
    {
        return Err(invalid_prompt_page());
    }
    Ok(())
}

fn valid_prompt_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= MAX_PROMPT_ID_BYTES && !id.chars().any(char::is_control)
}

fn invalid_prompt_page() -> String {
    "Forge API returned an invalid prompt page".into()
}

#[cfg(test)]
#[path = "remote_prompt_page_tests.rs"]
mod tests;
