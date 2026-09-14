use forge_runtime_domain::{
    HubEntity, HubStore, HubStoreError, MAX_OWNED_RUN_PAGE_LIMIT,
    MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT, MAX_RUN_EVENT_JSON_BYTES, OwnedRunCursor, OwnedRunStatus,
    OwnedRunTimelineEventType,
};
#[path = "owned_run_read/fixture.rs"]
mod fixture;
use fixture::OwnedRunFixture;

#[test]
fn owner_run_pages_are_keyset_ordered_and_return_only_scalar_summaries() {
    let fixture = OwnedRunFixture::new();
    let newest = fixture.add_run(&fixture.owner_a, &fixture.conversation_a, "run-z", 200);
    let older = fixture.add_run(&fixture.owner_a, &fixture.conversation_a, "run-a", 200);
    fixture.add_run(
        &fixture.owner_b,
        &fixture.conversation_b,
        "run-foreign",
        300,
    );
    fixture.add_private_events(&newest);
    fixture.add_single_event(&older, 1, "run_started", "prompt", "older prompt");
    assert_keyset_order(&fixture, &newest.prompt_id);
}

fn assert_keyset_order(fixture: &OwnedRunFixture, newest_prompt_id: &str) {
    let first = fixture
        .store
        .owned_run_page(&fixture.owner_a, &fixture.conversation_a, None, 1)
        .expect("first owner Run page");
    assert_eq!(first.conversation_id, fixture.conversation_a);
    assert_eq!(first.runs.len(), 1);
    assert_eq!(first.runs[0].run_id, "run-z");
    assert_eq!(first.runs[0].prompt_id, newest_prompt_id);
    assert_eq!(first.runs[0].created_at_ms, 200);
    assert_eq!(first.runs[0].latest_sequence, 5);
    assert_eq!(first.runs[0].status, OwnedRunStatus::Completed);
    assert!(first.has_more);
    assert_eq!(
        first.next_cursor,
        Some(OwnedRunCursor {
            created_at_ms: 200,
            run_id: "run-z".into(),
        })
    );

    let second = fixture
        .store
        .owned_run_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            first.next_cursor.as_ref(),
            1,
        )
        .expect("second owner Run page");
    assert_eq!(second.runs.len(), 1);
    assert_eq!(second.runs[0].run_id, "run-a");
    assert_eq!(second.runs[0].latest_sequence, 1);
    assert_eq!(second.runs[0].status, OwnedRunStatus::Nonterminal);
    assert!(!second.has_more);
    assert!(second.next_cursor.is_none());

    assert_summary_hides_private_data(&first);
}

fn assert_summary_hides_private_data(page: &forge_runtime_domain::OwnedRunPage) {
    let encoded = serde_json::to_string(page).expect("sanitize Run summary page");
    for secret in [
        "foreign-run",
        "private prompt body",
        "private assistant delta",
        "private tool argument",
        "private tool output",
        "private error detail",
        "private final answer",
        "private system prompt",
        "private execution path",
        "private idempotency key",
        "execution",
        "idempotency_key",
        "events",
    ] {
        assert!(
            !encoded.contains(secret),
            "summary leaked {secret}: {encoded}"
        );
    }
}

#[test]
fn owner_timeline_pages_keep_dense_sequences_but_hide_intermediate_payloads() {
    let fixture = OwnedRunFixture::new();
    let run = fixture.add_run(&fixture.owner_a, &fixture.conversation_a, "run-a", 200);
    fixture.add_private_events(&run);
    let (first, second, final_page) = timeline_pages(&fixture);
    assert_timeline_sequences(&first, &second, &final_page);
    assert_timeline_pages_hide_private_data(&[first, second, final_page]);
}

fn timeline_pages(
    fixture: &OwnedRunFixture,
) -> (
    forge_runtime_domain::OwnedRunTimelinePage,
    forge_runtime_domain::OwnedRunTimelinePage,
    forge_runtime_domain::OwnedRunTimelinePage,
) {
    let first = fixture
        .store
        .owned_run_timeline_page(&fixture.owner_a, &fixture.conversation_a, "run-a", 0, 2)
        .expect("first timeline page");
    let second = fixture
        .store
        .owned_run_timeline_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            "run-a",
            first.scanned_through_sequence,
            2,
        )
        .expect("continued timeline page");
    let final_page = fixture
        .store
        .owned_run_timeline_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            "run-a",
            second.scanned_through_sequence,
            2,
        )
        .expect("final timeline page");
    (first, second, final_page)
}

fn assert_timeline_sequences(
    first: &forge_runtime_domain::OwnedRunTimelinePage,
    second: &forge_runtime_domain::OwnedRunTimelinePage,
    final_page: &forge_runtime_domain::OwnedRunTimelinePage,
) {
    assert_eq!(
        (first.after_sequence, first.scanned_through_sequence),
        (0, 2)
    );
    assert!(first.has_more);
    assert_eq!(
        first
            .events
            .iter()
            .map(|event| (event.seq, event.event_type))
            .collect::<Vec<_>>(),
        [
            (1, OwnedRunTimelineEventType::RunStarted),
            (2, OwnedRunTimelineEventType::Activity)
        ]
    );
    assert_eq!(second.scanned_through_sequence, 4);
    assert!(second.has_more);
    assert_eq!(
        second
            .events
            .iter()
            .map(|event| (event.seq, event.event_type))
            .collect::<Vec<_>>(),
        [
            (3, OwnedRunTimelineEventType::Activity),
            (4, OwnedRunTimelineEventType::Activity)
        ]
    );
    assert_eq!(final_page.scanned_through_sequence, 5);
    assert!(!final_page.has_more);
    assert_eq!(final_page.events.len(), 1);
    assert_eq!(
        final_page.events[0].event_type,
        OwnedRunTimelineEventType::RunFinished
    );
}

fn assert_timeline_pages_hide_private_data(pages: &[forge_runtime_domain::OwnedRunTimelinePage]) {
    let encoded = serde_json::to_string(pages).expect("encode sanitized timeline pages");
    for secret in [
        "private prompt body",
        "private assistant delta",
        "private tool name",
        "private tool argument",
        "private tool output",
        "private error detail",
        "private final answer",
        "private system prompt",
        "private execution path",
        "tool_started",
        "assistant_delta",
        "runtime_error",
    ] {
        assert!(
            !encoded.contains(secret),
            "timeline leaked {secret}: {encoded}"
        );
    }
}

#[test]
fn foreign_and_absent_conversation_or_run_are_indistinguishable() {
    let fixture = OwnedRunFixture::new();
    fixture.add_run(
        &fixture.owner_b,
        &fixture.conversation_b,
        "run-foreign",
        300,
    );
    assert_foreign_and_absent_conversations_are_hidden(&fixture);
    assert_foreign_and_absent_runs_are_hidden(&fixture);
}

fn assert_foreign_and_absent_conversations_are_hidden(fixture: &OwnedRunFixture) {
    let foreign_conversation = fixture
        .store
        .owned_run_page(&fixture.owner_a, &fixture.conversation_b, None, 1)
        .expect_err("foreign Conversation is hidden");
    let absent_conversation = fixture
        .store
        .owned_run_page(&fixture.owner_a, "conversation-absent", None, 1)
        .expect_err("absent Conversation is hidden");
    assert!(matches!(
        foreign_conversation,
        HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            ..
        }
    ));
    assert!(matches!(
        absent_conversation,
        HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            ..
        }
    ));
}

fn assert_foreign_and_absent_runs_are_hidden(fixture: &OwnedRunFixture) {
    let foreign_run = fixture
        .store
        .owned_run_timeline_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            "run-foreign",
            0,
            1,
        )
        .expect_err("Run outside Conversation is hidden");
    let absent_run = fixture
        .store
        .owned_run_timeline_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            "run-absent",
            0,
            1,
        )
        .expect_err("absent Run is hidden");
    assert_eq!(foreign_run, absent_run);
}

#[test]
fn owner_run_queries_reject_invalid_limits_and_malformed_event_rows() {
    let fixture = OwnedRunFixture::new();
    let run = fixture.add_run(&fixture.owner_a, &fixture.conversation_a, "run-a", 200);
    let gapped = fixture.add_run(&fixture.owner_a, &fixture.conversation_a, "run-gap", 100);
    assert_invalid_run_limits(&fixture);
    assert_malformed_run_events(&fixture, &run, &gapped);
}

fn assert_invalid_run_limits(fixture: &OwnedRunFixture) {
    for limit in [0, MAX_OWNED_RUN_PAGE_LIMIT + 1] {
        assert!(matches!(
            fixture
                .store
                .owned_run_page(&fixture.owner_a, &fixture.conversation_a, None, limit),
            Err(HubStoreError::Conflict { .. })
        ));
    }
    for limit in [0, MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT + 1] {
        assert!(matches!(
            fixture.store.owned_run_timeline_page(
                &fixture.owner_a,
                &fixture.conversation_a,
                "run-a",
                0,
                limit
            ),
            Err(HubStoreError::Conflict { .. })
        ));
    }
}

fn assert_malformed_run_events(
    fixture: &OwnedRunFixture,
    run: &fixture::RunFixture,
    gapped: &fixture::RunFixture,
) {
    assert!(matches!(
        fixture
            .store
            .owned_run_page(&fixture.owner_a, &fixture.conversation_a, None, 1),
        Err(HubStoreError::Corrupt { .. })
    ));
    fixture.insert_raw_event(&run, 1, "{not-json");
    assert!(matches!(
        fixture
            .store
            .owned_run_page(&fixture.owner_a, &fixture.conversation_a, None, 1),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        fixture.store.owned_run_timeline_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            "run-a",
            0,
            1
        ),
        Err(HubStoreError::Corrupt { .. })
    ));
    fixture.add_single_event(&gapped, 1, "run_started", "prompt", "gap prompt");
    fixture.add_single_event(&gapped, 3, "turn_started", "turn", "unused");
    assert!(matches!(
        fixture.store.owned_run_timeline_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            &gapped.run_id,
            0,
            2
        ),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn both_pages_enforce_aggregate_utf8_event_byte_budget_and_resume_without_gaps() {
    let fixture = OwnedRunFixture::new();
    let body = "é".repeat(400_000);
    assert_run_pages_resume_at_byte_budget(&fixture, &body);
    assert_timeline_pages_resume_at_byte_budget(&fixture, &body);
}

fn assert_run_pages_resume_at_byte_budget(fixture: &OwnedRunFixture, body: &str) {
    let mut runs = Vec::new();
    for (run_id, created_at_ms) in [("run-3", 300), ("run-2", 200), ("run-1", 100)] {
        let run = fixture.add_run(
            &fixture.owner_a,
            &fixture.conversation_a,
            run_id,
            created_at_ms,
        );
        fixture.add_single_event(&run, 1, "run_started", "prompt", &body);
        runs.push(run);
    }

    let first = fixture
        .store
        .owned_run_page(&fixture.owner_a, &fixture.conversation_a, None, 3)
        .expect("byte-bounded Run summaries");
    assert_eq!(first.runs.len(), 2);
    assert!(first.has_more);
    assert_eq!(first.runs[0].run_id, "run-3");
    assert_eq!(first.runs[1].run_id, "run-2");
    let first_run_bytes = fixture.event_bytes(&runs[0], 1);
    let second_run_bytes = fixture.event_bytes(&runs[1], 1);
    let third_run_bytes = fixture.event_bytes(&runs[2], 1);
    assert!(first_run_bytes + second_run_bytes <= MAX_RUN_EVENT_JSON_BYTES);
    assert!(first_run_bytes + second_run_bytes + third_run_bytes > MAX_RUN_EVENT_JSON_BYTES);

    let second = fixture
        .store
        .owned_run_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            first.next_cursor.as_ref(),
            3,
        )
        .expect("resume byte-bounded Run summaries");
    assert_eq!(second.runs.len(), 1);
    assert_eq!(second.runs[0].run_id, "run-1");
    assert!(!second.has_more);
}

fn assert_timeline_pages_resume_at_byte_budget(fixture: &OwnedRunFixture, body: &str) {
    let timeline_run = fixture.add_run(
        &fixture.owner_a,
        &fixture.conversation_a,
        "run-large-timeline",
        50,
    );
    for sequence in 1..=3 {
        fixture.add_single_event(&timeline_run, sequence, "assistant_delta", "delta", &body);
    }
    let first_timeline = fixture
        .store
        .owned_run_timeline_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            &timeline_run.run_id,
            0,
            3,
        )
        .expect("byte-bounded timeline page");
    assert_eq!(first_timeline.events.len(), 2);
    assert_eq!(first_timeline.scanned_through_sequence, 2);
    assert!(first_timeline.has_more);
    let timeline_bytes = (1..=3)
        .map(|sequence| fixture.event_bytes(&timeline_run, sequence))
        .sum::<usize>();
    assert!(timeline_bytes > MAX_RUN_EVENT_JSON_BYTES);

    let second_timeline = fixture
        .store
        .owned_run_timeline_page(
            &fixture.owner_a,
            &fixture.conversation_a,
            &timeline_run.run_id,
            first_timeline.scanned_through_sequence,
            3,
        )
        .expect("resume byte-bounded timeline page");
    assert_eq!(second_timeline.events.len(), 1);
    assert_eq!(second_timeline.events[0].seq, 3);
    assert_eq!(second_timeline.scanned_through_sequence, 3);
    assert!(!second_timeline.has_more);
}
