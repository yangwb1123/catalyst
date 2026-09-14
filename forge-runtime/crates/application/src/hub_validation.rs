use std::path::{Component, Path, PathBuf};

use crate::{
    HubError, HubField,
    runtime_domain::{
        ConversationBootstrapCursor, ConversationBootstrapPhase, ConversationPromptCursor,
        ConversationScope, MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT,
        MAX_CONVERSATION_CHANGE_PAGE_LIMIT, MAX_CONVERSATION_PROMPT_PAGE_LIMIT,
        MAX_HUB_ENTITY_ID_BYTES, MAX_HUB_ROLE_BYTES, MAX_PROMPT_CONTENT_BYTES,
    },
};

pub const MAX_ENTITY_ID_BYTES: usize = MAX_HUB_ENTITY_ID_BYTES;
pub const MAX_GROUP_NAME_BYTES: usize = 128;
pub const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_PROMPT_BYTES: usize = MAX_PROMPT_CONTENT_BYTES;
pub const MAX_PROMPT_LIST_LIMIT: usize = 1_000;
pub const MAX_ROLE_BYTES: usize = MAX_HUB_ROLE_BYTES;
pub const MAX_TITLE_BYTES: usize = 256;

pub(crate) fn required(value: &str, field: HubField, max_bytes: usize) -> Result<(), HubError> {
    if value.trim().is_empty() {
        return Err(HubError::Empty { field });
    }
    if value.len() > max_bytes {
        return Err(HubError::TooLong { field, max_bytes });
    }
    Ok(())
}

/// Validates the Hub's shared caller-owned idempotency-key bounds.
///
/// # Errors
///
/// Returns `HubError::Empty` for blank values and `HubError::TooLong` when the
/// UTF-8 byte length exceeds the shared maximum.
pub fn validate_idempotency_key(value: &str) -> Result<(), HubError> {
    required(value, HubField::IdempotencyKey, MAX_IDEMPOTENCY_KEY_BYTES)
}

pub(crate) fn scope(scope: &ConversationScope) -> Result<(), HubError> {
    match scope {
        ConversationScope::Global => Ok(()),
        ConversationScope::Project(id) => required_id(id, HubField::ProjectId),
        ConversationScope::Group(id) => required_id(id, HubField::GroupId),
    }
}

pub(crate) fn required_id(value: &str, field: HubField) -> Result<(), HubError> {
    required(value, field, MAX_ENTITY_ID_BYTES)
}

pub(crate) fn prompt_limit(limit: usize) -> Result<(), HubError> {
    if (1..=MAX_PROMPT_LIST_LIMIT).contains(&limit) {
        return Ok(());
    }
    Err(HubError::OutOfRange {
        field: HubField::PromptLimit,
        min: 1,
        max: MAX_PROMPT_LIST_LIMIT,
    })
}

pub(crate) fn conversation_change_page_limit(limit: usize) -> Result<(), HubError> {
    if (1..=MAX_CONVERSATION_CHANGE_PAGE_LIMIT).contains(&limit) {
        return Ok(());
    }
    Err(HubError::OutOfRange {
        field: HubField::ChangePageLimit,
        min: 1,
        max: MAX_CONVERSATION_CHANGE_PAGE_LIMIT,
    })
}

pub(crate) fn conversation_bootstrap_page(
    cursor: Option<&ConversationBootstrapCursor>,
    limit: usize,
) -> Result<(), HubError> {
    if !(1..=MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT).contains(&limit) {
        return Err(HubError::OutOfRange {
            field: HubField::BootstrapPageLimit,
            min: 1,
            max: MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT,
        });
    }
    let Some(cursor) = cursor else {
        return Ok(());
    };
    if cursor.snapshot_cursor > i64::MAX as u64 {
        return Err(bootstrap_cursor_range_error(i64::MAX as u64));
    }
    match cursor.phase {
        ConversationBootstrapPhase::LegacyBaseline => validate_baseline_cursor(cursor)?,
        ConversationBootstrapPhase::ChangeLog => validate_change_cursor(cursor)?,
    }
    Ok(())
}

fn validate_baseline_cursor(cursor: &ConversationBootstrapCursor) -> Result<(), HubError> {
    if cursor.after_change_cursor.is_some() {
        return Err(invalid_bootstrap_cursor());
    }
    let Some(after_id) = cursor.after_conversation_id.as_deref() else {
        return Err(invalid_bootstrap_cursor());
    };
    required_id(after_id, HubField::BootstrapCursor)
}

fn validate_change_cursor(cursor: &ConversationBootstrapCursor) -> Result<(), HubError> {
    if cursor.after_conversation_id.is_some() {
        return Err(invalid_bootstrap_cursor());
    }
    let Some(after_cursor) = cursor.after_change_cursor else {
        return Err(invalid_bootstrap_cursor());
    };
    if after_cursor > cursor.snapshot_cursor || after_cursor > i64::MAX as u64 {
        return Err(bootstrap_cursor_range_error(
            cursor.snapshot_cursor.min(i64::MAX as u64),
        ));
    }
    Ok(())
}

fn invalid_bootstrap_cursor() -> HubError {
    HubError::InvalidCharacters {
        field: HubField::BootstrapCursor,
    }
}

fn bootstrap_cursor_range_error(max: u64) -> HubError {
    HubError::OutOfRange {
        field: HubField::BootstrapCursor,
        min: 0,
        max: usize::try_from(max).unwrap_or(usize::MAX),
    }
}

pub(crate) fn conversation_prompt_page(
    before: Option<&ConversationPromptCursor>,
    limit: usize,
) -> Result<(), HubError> {
    if !(1..=MAX_CONVERSATION_PROMPT_PAGE_LIMIT).contains(&limit) {
        return Err(HubError::OutOfRange {
            field: HubField::PromptLimit,
            min: 1,
            max: MAX_CONVERSATION_PROMPT_PAGE_LIMIT,
        });
    }
    if let Some(cursor) = before {
        required_id(&cursor.prompt_id, HubField::PromptId)?;
        if cursor.created_at_ms > i64::MAX as u64 {
            return Err(HubError::OutOfRange {
                field: HubField::PromptCursor,
                min: 0,
                max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
            });
        }
    }
    Ok(())
}

pub(crate) fn normalized_absolute_path(path: &Path) -> Result<(), HubError> {
    let normalized = rebuild(path);
    if !path.is_absolute()
        || has_lexical_navigation(path)
        || normalized.as_os_str() != path.as_os_str()
    {
        return Err(HubError::InvalidProjectPath);
    }
    Ok(())
}

fn has_lexical_navigation(path: &Path) -> bool {
    path.components()
        .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
}

fn rebuild(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for part in path.components() {
        normalized.push(part.as_os_str());
    }
    normalized
}
