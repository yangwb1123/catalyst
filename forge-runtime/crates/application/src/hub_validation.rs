use std::path::{Component, Path, PathBuf};

use crate::{HubError, HubField, runtime_domain::ConversationScope};

pub const MAX_ENTITY_ID_BYTES: usize = 128;
pub const MAX_GROUP_NAME_BYTES: usize = 128;
pub const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_PROMPT_BYTES: usize = 256 * 1024;
pub const MAX_PROMPT_LIST_LIMIT: usize = 1_000;
pub const MAX_ROLE_BYTES: usize = 64;
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
