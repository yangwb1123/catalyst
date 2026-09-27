use std::collections::HashSet;

use serde::{Deserialize, Serialize};
pub(super) const PAGE_SIZE: usize = 25;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cursor {
    pub(super) submitted_at_ms: u64,
    pub(super) intent_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Intent {
    pub(super) intent_id: String,
    pub(super) conversation_id: String,
    pub(super) prompt_id: String,
    pub(super) project_id: String,
    pub(super) profile_id: String,
    pub(super) submitted_at_ms: u64,
    pub(super) aggregate_version: u64,
    pub(super) latest_sequence: u64,
    pub(super) status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Page {
    pub(super) conversation_id: String,
    pub(super) intents: Vec<Intent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) next_cursor: Option<Cursor>,
    pub(super) has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Event {
    pub(super) event_id: String,
    pub(super) seq: u64,
    pub(super) emitted_at_ms: u64,
    #[serde(rename = "type")]
    pub(super) event_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TimelinePage {
    pub(super) conversation_id: String,
    pub(super) intent_id: String,
    pub(super) after_sequence: u64,
    pub(super) scanned_through_sequence: u64,
    pub(super) has_more: bool,
    pub(super) events: Vec<Event>,
}

/// The immutable receipt returned by the private inert submission candidate.
/// This is deliberately a separate type from an ordinary Run: accepting the
/// receipt proves only that the Hub stored a Prompt and pending intent.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Submission {
    pub(super) prompt: Prompt,
    pub(super) intent: Intent,
    pub(super) initial_event: Event,
    pub(super) replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prompt {
    pub(super) id: String,
    pub(super) conversation_id: String,
    pub(super) role: String,
    pub(super) content: String,
    pub(super) created_at_ms: u64,
}

pub(super) fn validate_submission(
    submission: &Submission,
    conversation_id: &str,
    content: &str,
) -> Result<(), String> {
    if submission.prompt.conversation_id != conversation_id
        || submission.prompt.role != "user"
        || submission.prompt.content != content
        || submission.prompt.created_at_ms > MAX_SAFE_JSON_INTEGER
        || submission.intent.conversation_id != conversation_id
        || submission.intent.prompt_id != submission.prompt.id
        || submission.prompt.created_at_ms != submission.intent.submitted_at_ms
        || submission.initial_event.seq != 1
        || submission.initial_event.event_type != "submitted"
        || submission.initial_event.emitted_at_ms != submission.intent.submitted_at_ms
    {
        return Err(invalid_submission());
    }
    validate_id(&submission.prompt.id, "Prompt")?;
    validate_intent(&submission.intent, conversation_id)?;
    validate_event(&submission.initial_event)?;
    Ok(())
}

/// Binds a fresh pending-intent receipt to the caller's Conversation CAS.
/// An idempotent replay intentionally returns the original historical
/// receipt, so its aggregate version is not compared with the retry's
/// expected version.
pub(super) fn validate_submission_for_version(
    submission: &Submission,
    conversation_id: &str,
    content: &str,
    expected_version: u64,
) -> Result<(), String> {
    validate_submission(submission, conversation_id, content)?;
    if submission.replayed
        || (expected_version < MAX_SAFE_JSON_INTEGER
            && submission.intent.aggregate_version == expected_version + 1)
    {
        return Ok(());
    }
    Err(invalid_submission())
}

pub(super) fn validate_page(
    page: &Page,
    conversation_id: &str,
    before: Option<&Cursor>,
    limit: usize,
) -> Result<(), String> {
    if limit == 0
        || limit > PAGE_SIZE
        || page.conversation_id != conversation_id
        || page.intents.len() > limit
        || page.has_more != page.next_cursor.is_some()
        || (page.has_more && page.intents.is_empty())
    {
        return Err(invalid_page());
    }
    validate_id(conversation_id, "Conversation")?;
    let mut previous: Option<&Intent> = None;
    let mut seen = HashSet::with_capacity(page.intents.len());
    for intent in &page.intents {
        validate_intent(intent, conversation_id)?;
        if !seen.insert(intent.intent_id.as_str())
            || previous.is_some_and(|older| !newer(older, intent))
            || before.is_some_and(|cursor| !older_than(intent, cursor))
        {
            return Err(invalid_page());
        }
        previous = Some(intent);
    }
    if let Some(cursor) = &page.next_cursor {
        validate_cursor(cursor)?;
        let Some(last) = page.intents.last() else {
            return Err(invalid_page());
        };
        if cursor.submitted_at_ms != last.submitted_at_ms || cursor.intent_id != last.intent_id {
            return Err(invalid_page());
        }
    }
    Ok(())
}

pub(super) fn validate_timeline(
    page: &TimelinePage,
    conversation_id: &str,
    intent_id: &str,
    after_sequence: u64,
    limit: usize,
) -> Result<(), String> {
    if limit == 0
        || limit > PAGE_SIZE
        || page.conversation_id != conversation_id
        || page.intent_id != intent_id
        || page.after_sequence != after_sequence
        || page.after_sequence > MAX_SAFE_JSON_INTEGER
        || page.scanned_through_sequence > MAX_SAFE_JSON_INTEGER
        || page.scanned_through_sequence < page.after_sequence
        || page.events.len() > limit
        || page.has_more
    {
        return Err(invalid_timeline());
    }
    validate_id(conversation_id, "Conversation")?;
    validate_id(intent_id, "pending Run-intent")?;
    for event in &page.events {
        validate_event(event)?;
    }
    if after_sequence == 0 {
        if page.events.len() != 1 || page.scanned_through_sequence != 1 {
            return Err(invalid_timeline());
        }
    } else if !page.events.is_empty() || page.scanned_through_sequence != after_sequence {
        return Err(invalid_timeline());
    }
    Ok(())
}

fn validate_cursor(cursor: &Cursor) -> Result<(), String> {
    if cursor.submitted_at_ms > MAX_SAFE_JSON_INTEGER {
        return Err(invalid_page());
    }
    validate_id(&cursor.intent_id, "pending Run-intent")
}

fn validate_intent(intent: &Intent, conversation_id: &str) -> Result<(), String> {
    if intent.conversation_id != conversation_id
        || intent.submitted_at_ms > MAX_SAFE_JSON_INTEGER
        || intent.aggregate_version == 0
        || intent.aggregate_version > MAX_SAFE_JSON_INTEGER
        || intent.latest_sequence != 1
        || intent.status != "pending"
    {
        return Err(invalid_page());
    }
    for (value, label) in [
        (&intent.intent_id, "pending Run-intent"),
        (&intent.prompt_id, "Prompt"),
        (&intent.project_id, "Project"),
        (&intent.profile_id, "execution profile"),
    ] {
        validate_id(value, label)?;
    }
    Ok(())
}

fn validate_event(event: &Event) -> Result<(), String> {
    if event.seq == 0
        || event.seq > MAX_SAFE_JSON_INTEGER
        || event.emitted_at_ms > MAX_SAFE_JSON_INTEGER
        || event.event_type != "submitted"
    {
        return Err(invalid_timeline());
    }
    validate_id(&event.event_id, "pending Run-intent event")
}

fn validate_id(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(format!("{label} id is invalid"));
    }
    Ok(())
}

fn newer(left: &Intent, right: &Intent) -> bool {
    left.submitted_at_ms > right.submitted_at_ms
        || (left.submitted_at_ms == right.submitted_at_ms && left.intent_id > right.intent_id)
}

fn older_than(intent: &Intent, cursor: &Cursor) -> bool {
    intent.submitted_at_ms < cursor.submitted_at_ms
        || (intent.submitted_at_ms == cursor.submitted_at_ms && intent.intent_id < cursor.intent_id)
}

fn invalid_page() -> String {
    "Forge API returned an invalid pending Run-intent page".into()
}

fn invalid_timeline() -> String {
    "Forge API returned an invalid pending Run-intent timeline".into()
}

fn invalid_submission() -> String {
    "Forge API returned an invalid pending Run-intent submission".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(id: &str, submitted_at_ms: u64) -> Intent {
        Intent {
            intent_id: id.into(),
            conversation_id: "conversation-1".into(),
            prompt_id: format!("prompt-{id}"),
            project_id: "project-1".into(),
            profile_id: "profile-1".into(),
            submitted_at_ms,
            aggregate_version: 2,
            latest_sequence: 1,
            status: "pending".into(),
        }
    }

    #[test]
    fn page_requires_newest_first_and_cursor_binding() {
        let page = Page {
            conversation_id: "conversation-1".into(),
            intents: vec![intent("intent-2", 20), intent("intent-1", 10)],
            next_cursor: Some(Cursor {
                submitted_at_ms: 10,
                intent_id: "intent-1".into(),
            }),
            has_more: true,
        };
        assert!(validate_page(&page, "conversation-1", None, 2).is_ok());
        let mut bad = page.clone();
        bad.next_cursor.as_mut().unwrap().intent_id = "wrong".into();
        assert!(validate_page(&bad, "conversation-1", None, 2).is_err());
        bad = page;
        bad.intents.swap(0, 1);
        assert!(validate_page(&bad, "conversation-1", None, 2).is_err());
    }

    #[test]
    fn timeline_accepts_initial_and_empty_continuation_only() {
        let initial = TimelinePage {
            conversation_id: "conversation-1".into(),
            intent_id: "intent-1".into(),
            after_sequence: 0,
            scanned_through_sequence: 1,
            has_more: false,
            events: vec![Event {
                event_id: "event-1".into(),
                seq: 1,
                emitted_at_ms: 20,
                event_type: "submitted".into(),
            }],
        };
        assert!(validate_timeline(&initial, "conversation-1", "intent-1", 0, 25).is_ok());
        let continuation = TimelinePage {
            after_sequence: 1,
            scanned_through_sequence: 1,
            events: vec![],
            ..initial
        };
        assert!(validate_timeline(&continuation, "conversation-1", "intent-1", 1, 25).is_ok());
    }
}
