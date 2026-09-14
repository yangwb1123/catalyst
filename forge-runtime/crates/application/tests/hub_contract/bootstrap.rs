use super::*;

#[test]
fn conversation_bootstrap_pages_metadata_and_advances_across_prompt_only_rows() {
    let service = service();
    let conversation = service
        .create_session(&ConversationScope::Global, "bootstrap", "bootstrap-session")
        .expect("Conversation");
    service
        .append_prompt(&conversation.id, "user", "hello", "bootstrap-prompt")
        .expect("Prompt");

    let first = service
        .conversation_bootstrap_page(None, 1)
        .expect("bootstrap first page");
    assert_eq!(first.snapshot_cursor, 2);
    assert_eq!(first.conversations.len(), 1);
    assert_eq!(first.conversations[0].conversation.id, conversation.id);
    assert_eq!(first.conversations[0].creation_cursor, 1);
    assert_eq!(first.conversations[0].aggregate_version, 2);
    assert!(first.has_more);

    let second = service
        .conversation_bootstrap_page(first.next_cursor.as_ref(), 1)
        .expect("Prompt-only bootstrap page");
    assert!(second.conversations.is_empty());
    assert!(!second.has_more);
    assert!(second.next_cursor.is_none());

    assert_bootstrap_limits_and_cursor(&service);
    assert_baseline_cursor_continues(&service);
}

fn assert_bootstrap_limits_and_cursor(service: &HubService) {
    for limit in [0, MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT + 1] {
        assert!(matches!(
            service.conversation_bootstrap_page(None, limit),
            Err(HubError::OutOfRange {
                field: HubField::BootstrapPageLimit,
                ..
            })
        ));
    }
    let malformed_cursor = ConversationBootstrapCursor {
        snapshot_cursor: 2,
        phase: ConversationBootstrapPhase::ChangeLog,
        after_conversation_id: Some("unexpected".into()),
        after_change_cursor: Some(1),
    };
    assert!(matches!(
        service.conversation_bootstrap_page(Some(&malformed_cursor), 1),
        Err(HubError::InvalidCharacters {
            field: HubField::BootstrapCursor
        })
    ));
}

fn assert_baseline_cursor_continues(service: &HubService) {
    let baseline_cursor = ConversationBootstrapCursor {
        snapshot_cursor: 2,
        phase: ConversationBootstrapPhase::LegacyBaseline,
        after_conversation_id: Some("legacy-id".into()),
        after_change_cursor: None,
    };
    let transition = service
        .conversation_bootstrap_page(Some(&baseline_cursor), 1)
        .expect("valid legacy baseline continuation");
    let continuation = transition.next_cursor.expect("transition to change log");
    assert_eq!(continuation.phase, ConversationBootstrapPhase::ChangeLog);
    assert_eq!(continuation.after_change_cursor, Some(0));
}
