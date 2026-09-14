use super::{
    ConversationBootstrapCursor, ConversationBootstrapEntry, ConversationBootstrapPage,
    ConversationBootstrapPhase, ConversationChangeKind, HubEntity, HubStoreError,
    MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT, MemoryState,
};

pub(super) fn conversation_bootstrap_page(
    state: &MemoryState,
    cursor: Option<&ConversationBootstrapCursor>,
    limit: usize,
) -> Result<ConversationBootstrapPage, HubStoreError> {
    validate_limit(limit)?;
    let head = u64::try_from(state.conversation_changes.len()).expect("test change count fits u64");
    let snapshot_cursor = cursor.map_or(head, |value| value.snapshot_cursor);
    if snapshot_cursor > head {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: "in-memory bootstrap cursor exceeds the observed head".into(),
        });
    }
    match cursor {
        Some(value) if value.phase == ConversationBootstrapPhase::LegacyBaseline => {
            Ok(baseline_transition(snapshot_cursor))
        }
        Some(value) if value.phase == ConversationBootstrapPhase::ChangeLog => {
            let after = value.after_change_cursor.ok_or_else(invalid_cursor)?;
            change_page(state, snapshot_cursor, after, limit)
        }
        None => change_page(state, snapshot_cursor, 0, limit),
        Some(_) => Err(invalid_cursor()),
    }
}

fn validate_limit(limit: usize) -> Result<(), HubStoreError> {
    if (1..=MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT).contains(&limit) {
        return Ok(());
    }
    Err(HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: "invalid in-memory bootstrap page limit".into(),
    })
}

fn baseline_transition(snapshot_cursor: u64) -> ConversationBootstrapPage {
    let has_more = snapshot_cursor > 0;
    ConversationBootstrapPage {
        snapshot_cursor,
        conversations: Vec::new(),
        scanned_through_cursor: 0,
        next_cursor: has_more.then_some(ConversationBootstrapCursor {
            snapshot_cursor,
            phase: ConversationBootstrapPhase::ChangeLog,
            after_conversation_id: None,
            after_change_cursor: Some(0),
        }),
        has_more,
    }
}

fn change_page(
    state: &MemoryState,
    snapshot_cursor: u64,
    after_cursor: u64,
    limit: usize,
) -> Result<ConversationBootstrapPage, HubStoreError> {
    if after_cursor > snapshot_cursor {
        return Err(invalid_cursor());
    }
    let (changes, has_extra) = read_change_rows(state, after_cursor, snapshot_cursor, limit);
    let next_position = changes.last().map_or(after_cursor, |change| change.cursor);
    if !has_extra && next_position != snapshot_cursor {
        return Err(HubStoreError::Corrupt {
            message: "in-memory bootstrap change rows have a gap".into(),
        });
    }
    let conversations = changes
        .iter()
        .filter(|change| change.kind == ConversationChangeKind::ConversationCreated)
        .map(|change| bootstrap_entry(state, change.conversation_id.as_str(), change.cursor))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = has_extra || next_position < snapshot_cursor;
    Ok(ConversationBootstrapPage {
        snapshot_cursor,
        conversations,
        scanned_through_cursor: next_position,
        next_cursor: has_more.then_some(ConversationBootstrapCursor {
            snapshot_cursor,
            phase: ConversationBootstrapPhase::ChangeLog,
            after_conversation_id: None,
            after_change_cursor: Some(next_position),
        }),
        has_more,
    })
}

fn read_change_rows(
    state: &MemoryState,
    after_cursor: u64,
    snapshot_cursor: u64,
    limit: usize,
) -> (Vec<super::ConversationChange>, bool) {
    let mut changes = state
        .conversation_changes
        .iter()
        .filter(|change| change.cursor > after_cursor && change.cursor <= snapshot_cursor)
        .take(limit + 1)
        .cloned()
        .collect::<Vec<_>>();
    let has_extra = changes.len() > limit;
    if has_extra {
        changes.truncate(limit);
    }
    (changes, has_extra)
}

fn bootstrap_entry(
    state: &MemoryState,
    conversation_id: &str,
    creation_cursor: u64,
) -> Result<ConversationBootstrapEntry, HubStoreError> {
    let conversation = state
        .conversations
        .iter()
        .find(|item| item.id == conversation_id)
        .ok_or_else(|| HubStoreError::Corrupt {
            message: "in-memory create event has no Conversation".into(),
        })?
        .clone();
    let aggregate_version = state
        .conversation_changes
        .iter()
        .filter(|item| item.conversation_id == conversation.id)
        .map(|item| item.aggregate_version)
        .max()
        .unwrap_or(0);
    Ok(ConversationBootstrapEntry {
        conversation,
        creation_cursor,
        aggregate_version,
    })
}

fn invalid_cursor() -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: "invalid in-memory bootstrap cursor".into(),
    }
}
