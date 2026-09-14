use super::*;

#[test]
fn hub_service_submits_atomic_pending_intent_and_replays_original_receipt_after_revocation() {
    let fixture = ConsentFixture::new();
    let owner = owner("issuer", "subject", "tenant");
    let profile_id = "profile-server-owned-v1";
    let profile_sha256 = [0x61; 32];
    let expiry = unix_time_ms() + 60_000;
    let conversation = create_project_conversation(&fixture, &owner);
    let grant = grant_profile_consent(&fixture, &owner, profile_id, &profile_sha256, expiry);
    let submitted = submit_initial_intent(
        &fixture,
        &owner,
        &conversation.id,
        profile_id,
        &profile_sha256,
    );
    assert_initial_intent(&submitted, profile_id);

    advance_conversation_after_submission(
        &fixture,
        &owner,
        &conversation.id,
        submitted.intent.aggregate_version,
    );
    revoke_intent_consent(&fixture, &owner, &grant.grant.grant_id);
    assert_original_receipt_replays(&fixture, &owner, &conversation.id, &submitted);
    assert_pending_intent_projection(&fixture, &owner, &conversation.id, &submitted);
}

fn create_project_conversation(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
) -> Conversation {
    fixture
        .service
        .create_owned_conversation(
            owner,
            &ConversationScope::Project(fixture.project_id.clone()),
            "Project conversation",
            "project-conversation",
        )
        .expect("create Project-scoped Conversation")
}

fn grant_profile_consent(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    profile_id: &str,
    profile_sha256: &[u8; 32],
    expiry: u64,
) -> ProjectExecutionConsentGrantResult {
    fixture
        .service
        .grant_project_execution_consent(
            owner,
            &fixture.project_id,
            profile_id,
            profile_sha256,
            expiry,
            "project-intent-grant",
        )
        .expect("grant exact server profile")
}

fn submit_initial_intent(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    conversation_id: &str,
    profile_id: &str,
    profile_sha256: &[u8; 32],
) -> PendingRunIntentSubmissionResult {
    fixture
        .service
        .submit_owned_prompt_run_intent(
            owner,
            SubmitPendingRunIntent {
                conversation_id,
                content: "first line\nsecond line",
                idempotency_key: "same-intent-key",
                expected_version: 1,
                profile_id,
                profile_sha256,
            },
        )
        .expect("submit Prompt and pending intent")
}

fn assert_initial_intent(submitted: &PendingRunIntentSubmissionResult, profile_id: &str) {
    assert!(!submitted.replayed);
    assert_eq!(submitted.intent.status, PendingRunIntentStatus::Pending);
    assert_eq!(submitted.intent.profile_id, profile_id);
    assert_eq!(submitted.prompt.content, "first line\nsecond line");
    assert_eq!(
        submitted.initial_event.event_type,
        PendingRunIntentTimelineEventType::Submitted
    );
}

fn advance_conversation_after_submission(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    conversation_id: &str,
    expected_version: u64,
) {
    fixture
        .service
        .append_owned_prompt(
            owner,
            conversation_id,
            "later prompt",
            "later-prompt",
            expected_version,
        )
        .expect("advance aggregate version");
}

fn revoke_intent_consent(fixture: &ConsentFixture, owner: &ConversationOwner, grant_id: &str) {
    fixture
        .service
        .revoke_project_execution_consent(owner, grant_id, "revoke-consent")
        .expect("revoke the current grant");
}

fn assert_original_receipt_replays(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    conversation_id: &str,
    submitted: &PendingRunIntentSubmissionResult,
) {
    let replay = fixture
        .service
        .submit_owned_prompt_run_intent(
            owner,
            SubmitPendingRunIntent {
                conversation_id,
                content: "first line\nsecond line",
                idempotency_key: "same-intent-key",
                expected_version: 1,
                profile_id: "changed-server-profile",
                profile_sha256: &[0x62; 32],
            },
        )
        .expect("replay returns only the original receipt after current state changes");
    assert!(replay.replayed);
    assert_eq!(replay.prompt, submitted.prompt);
    assert_eq!(replay.intent, submitted.intent);
    assert_eq!(replay.initial_event, submitted.initial_event);
}

fn assert_pending_intent_projection(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    conversation_id: &str,
    submitted: &PendingRunIntentSubmissionResult,
) {
    let page = fixture
        .service
        .owned_pending_run_intent_page(owner, conversation_id, None, 10)
        .expect("list pending-intent projection");
    assert_eq!(
        page.intents.as_slice(),
        std::slice::from_ref(&submitted.intent)
    );
    assert_eq!(page.intents[0].intent_id, submitted.intent.intent_id);
    let timeline = fixture
        .service
        .owned_pending_run_intent_timeline_page(
            owner,
            conversation_id,
            &submitted.intent.intent_id,
            0,
            10,
        )
        .expect("read payload-free intent timeline");
    assert_eq!(
        timeline.events.as_slice(),
        std::slice::from_ref(&submitted.initial_event)
    );
}
