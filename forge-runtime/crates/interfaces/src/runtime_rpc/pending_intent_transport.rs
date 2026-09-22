use crate::runtime_domain::{
    ConversationPrompt, PendingRunIntent, PendingRunIntentCursor, PendingRunIntentPage,
    PendingRunIntentSubmissionResult, PendingRunIntentTimelineEvent, PendingRunIntentTimelinePage,
};

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) trait PendingRunIntentJsonSafe {
    fn pending_run_intent_json_safe(&self) -> bool;
}

fn prompt_json_safe(prompt: &ConversationPrompt) -> bool {
    prompt.created_at_ms <= MAX_SAFE_JSON_INTEGER
}

impl PendingRunIntentJsonSafe for PendingRunIntentCursor {
    fn pending_run_intent_json_safe(&self) -> bool {
        self.submitted_at_ms <= MAX_SAFE_JSON_INTEGER
    }
}

impl PendingRunIntentJsonSafe for PendingRunIntent {
    fn pending_run_intent_json_safe(&self) -> bool {
        self.submitted_at_ms <= MAX_SAFE_JSON_INTEGER
            && self.aggregate_version <= MAX_SAFE_JSON_INTEGER
            && self.latest_sequence <= MAX_SAFE_JSON_INTEGER
    }
}

impl PendingRunIntentJsonSafe for PendingRunIntentTimelineEvent {
    fn pending_run_intent_json_safe(&self) -> bool {
        self.seq <= MAX_SAFE_JSON_INTEGER && self.emitted_at_ms <= MAX_SAFE_JSON_INTEGER
    }
}

impl PendingRunIntentJsonSafe for PendingRunIntentSubmissionResult {
    fn pending_run_intent_json_safe(&self) -> bool {
        prompt_json_safe(&self.prompt)
            && self.intent.pending_run_intent_json_safe()
            && self.initial_event.pending_run_intent_json_safe()
    }
}

impl PendingRunIntentJsonSafe for PendingRunIntentPage {
    fn pending_run_intent_json_safe(&self) -> bool {
        self.intents
            .iter()
            .all(PendingRunIntentJsonSafe::pending_run_intent_json_safe)
            && self
                .next_cursor
                .as_ref()
                .is_none_or(PendingRunIntentJsonSafe::pending_run_intent_json_safe)
    }
}

impl PendingRunIntentJsonSafe for PendingRunIntentTimelinePage {
    fn pending_run_intent_json_safe(&self) -> bool {
        self.after_sequence <= MAX_SAFE_JSON_INTEGER
            && self.scanned_through_sequence <= MAX_SAFE_JSON_INTEGER
            && self
                .events
                .iter()
                .all(PendingRunIntentJsonSafe::pending_run_intent_json_safe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_domain::{PendingRunIntentStatus, PendingRunIntentTimelineEventType};

    fn intent(value: u64) -> PendingRunIntent {
        PendingRunIntent {
            intent_id: "intent-1".into(),
            conversation_id: "conversation-1".into(),
            prompt_id: "prompt-1".into(),
            project_id: "project-1".into(),
            profile_id: "profile-1".into(),
            submitted_at_ms: value,
            aggregate_version: value,
            latest_sequence: value,
            status: PendingRunIntentStatus::Pending,
        }
    }

    fn event(value: u64) -> PendingRunIntentTimelineEvent {
        PendingRunIntentTimelineEvent {
            event_id: "event-1".into(),
            seq: value,
            emitted_at_ms: value,
            event_type: PendingRunIntentTimelineEventType::Submitted,
        }
    }

    #[test]
    fn submission_accepts_ceiling_and_rejects_next_integer() {
        let mut result = PendingRunIntentSubmissionResult {
            prompt: ConversationPrompt {
                id: "prompt-1".into(),
                conversation_id: "conversation-1".into(),
                role: "user".into(),
                content: "prompt".into(),
                created_at_ms: MAX_SAFE_JSON_INTEGER,
            },
            intent: intent(MAX_SAFE_JSON_INTEGER),
            initial_event: event(MAX_SAFE_JSON_INTEGER),
            replayed: false,
        };
        assert!(result.pending_run_intent_json_safe());
        result.prompt.created_at_ms += 1;
        assert!(!result.pending_run_intent_json_safe());
        result.prompt.created_at_ms = MAX_SAFE_JSON_INTEGER;
        result.intent.aggregate_version += 1;
        assert!(!result.pending_run_intent_json_safe());
        result.intent.aggregate_version = MAX_SAFE_JSON_INTEGER;
        result.initial_event.emitted_at_ms += 1;
        assert!(!result.pending_run_intent_json_safe());
    }

    #[test]
    fn pages_accept_ceiling_and_reject_next_integer() {
        let mut page = PendingRunIntentPage {
            conversation_id: "conversation-1".into(),
            intents: vec![intent(MAX_SAFE_JSON_INTEGER)],
            next_cursor: Some(PendingRunIntentCursor {
                submitted_at_ms: MAX_SAFE_JSON_INTEGER,
                intent_id: "intent-1".into(),
            }),
            has_more: true,
        };
        assert!(page.pending_run_intent_json_safe());
        page.next_cursor.as_mut().unwrap().submitted_at_ms += 1;
        assert!(!page.pending_run_intent_json_safe());

        let mut timeline = PendingRunIntentTimelinePage {
            conversation_id: "conversation-1".into(),
            intent_id: "intent-1".into(),
            after_sequence: MAX_SAFE_JSON_INTEGER,
            scanned_through_sequence: MAX_SAFE_JSON_INTEGER,
            has_more: false,
            events: vec![event(MAX_SAFE_JSON_INTEGER)],
        };
        assert!(timeline.pending_run_intent_json_safe());
        timeline.events[0].seq += 1;
        assert!(!timeline.pending_run_intent_json_safe());
    }
}
