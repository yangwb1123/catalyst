use crate::runtime_domain::{
    ConversationOwner, ConversationScope, HubEntity, HubStore, HubStoreError,
    PendingRunIntentStatus, PendingRunIntentTimelineEventType, SubmitPendingRunIntent,
};
use crate::sqlite_hub::rows;

#[path = "pending_run_intent_atomicity/fixture.rs"]
mod fixture;
use fixture::Fixture;

use super::{SubmitInput, SubmitStage, submit, submit_with_hooks};

const PROFILE_ID: &str = "profile-server-fixed";
const PROFILE_DIGEST: [u8; 32] = [0x91; 32];

#[test]
fn submission_is_atomic_and_projects_pending_intent_separately_from_run() {
    let fixture = Fixture::new(true);
    let before_runs = fixture.count("runs");
    let before_run_events = fixture.count("run_events");
    let result = submit(
        &mut fixture.connection(),
        fixture.input("submit-key", 1, "first\nsecond"),
    )
    .expect("submit Prompt and pending intent");

    assert!(!result.replayed);
    assert_eq!(result.prompt.role, "user");
    assert_eq!(result.prompt.content, "first\nsecond");
    assert_eq!(result.intent.status, PendingRunIntentStatus::Pending);
    assert_eq!(result.intent.aggregate_version, 2);
    assert_eq!(result.intent.latest_sequence, 1);
    assert_eq!(result.initial_event.seq, 1);
    assert_eq!(
        result.initial_event.event_type,
        PendingRunIntentTimelineEventType::Submitted
    );
    assert_eq!(fixture.count("prompts"), 1);
    assert_eq!(fixture.count("pending_run_intents"), 1);
    assert_eq!(fixture.count("pending_run_intent_events"), 1);
    assert_eq!(fixture.count("runs"), before_runs);
    assert_eq!(fixture.count("run_events"), before_run_events);

    let page = fixture
        .store
        .owned_pending_run_intent_page(&fixture.owner, &fixture.conversation_id, None, 10)
        .expect("list pending intent projection");
    assert_eq!(
        page.intents.as_slice(),
        std::slice::from_ref(&result.intent)
    );
    assert!(!page.has_more);
    let timeline = fixture
        .store
        .owned_pending_run_intent_timeline_page(
            &fixture.owner,
            &fixture.conversation_id,
            &result.intent.intent_id,
            0,
            10,
        )
        .expect("read pending intent timeline");
    assert_eq!(timeline.events, [result.initial_event]);
    assert_eq!(timeline.scanned_through_sequence, 1);
}

#[test]
fn exact_replay_precedes_current_version_profile_and_consent_checks() {
    let fixture = Fixture::new(true);
    let first = submit(
        &mut fixture.connection(),
        fixture.input("replay-key", 1, "same prompt"),
    )
    .expect("first submit");
    fixture
        .store
        .append_owned_prompt(
            &fixture.owner,
            &fixture.conversation_id,
            "later prompt",
            "later-prompt-key",
            2,
        )
        .expect("advance Conversation version");
    fixture
        .store
        .revoke_project_execution_consent(&fixture.owner, &fixture.grant_id, "revoke-key")
        .expect("revoke grant after original submission");
    assert_original_receipt_replays(&fixture, &first);
    assert_reused_keys_reject_changed_input(&fixture);
}

fn assert_original_receipt_replays(
    fixture: &Fixture,
    first: &forge_runtime_domain::PendingRunIntentSubmissionResult,
) {
    let replay = submit(
        &mut fixture.connection(),
        SubmitInput {
            expected_version: 0,
            profile_id: "new-server-profile",
            profile_sha256: &[0x42; 32],
            ..fixture.input("replay-key", 1, "same prompt")
        },
    )
    .expect("exact replay returns original receipt after current consent changes");
    assert!(replay.replayed);
    assert_eq!(replay.prompt, first.prompt);
    assert_eq!(replay.intent, first.intent);
    assert_eq!(replay.initial_event, first.initial_event);
    assert_eq!(replay.intent.profile_id, PROFILE_ID);
    assert_eq!(fixture.count("prompts"), 2, "replay inserts no Prompt");
}

fn assert_reused_keys_reject_changed_input(fixture: &Fixture) {
    let changed_content = submit(
        &mut fixture.connection(),
        fixture.input("replay-key", 3, "different prompt"),
    )
    .expect_err("same key with changed body conflicts");
    assert!(matches!(changed_content, HubStoreError::Conflict { .. }));

    let other = fixture.create_owned_conversation("other-conversation");
    let changed_conversation = submit(
        &mut fixture.connection(),
        SubmitInput {
            conversation_id: &other,
            ..fixture.input("replay-key", 1, "same prompt")
        },
    )
    .expect_err("same key with another Conversation conflicts");
    assert!(matches!(
        changed_conversation,
        HubStoreError::Conflict { .. }
    ));
}

#[test]
fn new_submissions_require_owner_project_scope_exact_cas_and_matching_live_grant() {
    let fixture = Fixture::new(true);
    assert_stale_version_is_rejected(&fixture);
    assert_profile_mismatches_are_rejected(&fixture);
    assert_expired_grant_is_rejected();
    assert_revoked_and_missing_grants_are_rejected();
}

fn assert_stale_version_is_rejected(fixture: &Fixture) {
    assert!(matches!(
        submit(
            &mut fixture.connection(),
            fixture.input("stale-key", 0, "stale")
        ),
        Err(HubStoreError::Conflict {
            entity: HubEntity::PendingRunIntent,
            ..
        })
    ));
    assert_eq!(fixture.count("prompts"), 0);
}

fn assert_profile_mismatches_are_rejected(fixture: &Fixture) {
    assert_conflict(&submit(
        &mut fixture.connection(),
        SubmitInput {
            profile_id: "wrong-profile-id",
            ..fixture.input("wrong-id-key", 1, "x")
        },
    ));
    assert_conflict(&submit(
        &mut fixture.connection(),
        SubmitInput {
            profile_sha256: &[0x33; 32],
            ..fixture.input("wrong-hash-key", 1, "x")
        },
    ));
}

fn assert_expired_grant_is_rejected() {
    let expired = Fixture::new(true);
    let expired_at = expired
        .store
        .grant_project_execution_consent(
            &expired.owner,
            &expired.project_id,
            PROFILE_ID,
            &PROFILE_DIGEST,
            expired.grant_expiry,
            "unused-extra-grant",
        )
        .expect_err("only one active grant is allowed");
    assert!(matches!(expired_at, HubStoreError::Conflict { .. }));
    let clock = expired.grant_expiry;
    let error = submit_with_hooks(
        &mut expired.connection(),
        expired.input("expired-key", 1, "x"),
        || Ok(clock),
        |_| Ok(()),
    )
    .expect_err("grant expiring at the evaluated instant is inactive");
    assert!(matches!(error, HubStoreError::Conflict { .. }));
}

fn assert_revoked_and_missing_grants_are_rejected() {
    let revoked = Fixture::new(true);
    revoked
        .store
        .revoke_project_execution_consent(&revoked.owner, &revoked.grant_id, "revoke")
        .expect("revoke grant");
    assert_conflict(&submit(
        &mut revoked.connection(),
        revoked.input("revoked-key", 1, "x"),
    ));

    let absent = Fixture::new(false);
    assert_conflict(&submit(
        &mut absent.connection(),
        absent.input("missing-consent-key", 1, "x"),
    ));
}

#[test]
fn foreign_missing_legacy_and_non_project_conversations_are_uniformly_hidden_or_rejected() {
    let fixture = Fixture::new(true);
    assert_foreign_and_missing_conversations_are_hidden(&fixture);
    assert_ownerless_legacy_conversation_is_hidden(&fixture);
    assert_non_project_scopes_are_rejected(&fixture);
}

fn assert_foreign_and_missing_conversations_are_hidden(fixture: &Fixture) {
    let foreign = ConversationOwner {
        tenant_id: "foreign-tenant".into(),
        ..fixture.owner.clone()
    };
    let error = fixture
        .submit(
            &foreign,
            SubmitPendingRunIntent {
                conversation_id: &fixture.conversation_id,
                content: "secret",
                idempotency_key: "foreign-key",
                expected_version: 1,
                profile_id: PROFILE_ID,
                profile_sha256: &PROFILE_DIGEST,
            },
        )
        .expect_err("foreign owner is not found");
    assert_not_found_conversation(&error);

    let missing = fixture
        .submit(
            &fixture.owner,
            SubmitPendingRunIntent {
                conversation_id: "missing-conversation",
                content: "secret",
                idempotency_key: "missing-key",
                expected_version: 1,
                profile_id: PROFILE_ID,
                profile_sha256: &PROFILE_DIGEST,
            },
        )
        .expect_err("missing Conversation is not found");
    assert_not_found_conversation(&missing);
}

fn assert_ownerless_legacy_conversation_is_hidden(fixture: &Fixture) {
    let legacy = fixture
        .store
        .create_conversation(
            &ConversationScope::Project(fixture.project_id.clone()),
            "legacy",
            "legacy",
        )
        .expect("create ownerless legacy Conversation");
    let legacy_error = fixture
        .submit(
            &fixture.owner,
            SubmitPendingRunIntent {
                conversation_id: &legacy.id,
                content: "legacy prompt",
                idempotency_key: "legacy-key",
                expected_version: 1,
                profile_id: PROFILE_ID,
                profile_sha256: &PROFILE_DIGEST,
            },
        )
        .expect_err("legacy ownerless Conversation is not found");
    assert_not_found_conversation(&legacy_error);
}

fn assert_non_project_scopes_are_rejected(fixture: &Fixture) {
    let global = fixture.create_owned_conversation_with_scope("global", &ConversationScope::Global);
    assert!(matches!(
        fixture.submit(
            &fixture.owner,
            SubmitPendingRunIntent {
                conversation_id: &global,
                content: "global prompt",
                idempotency_key: "global-key",
                expected_version: 1,
                profile_id: PROFILE_ID,
                profile_sha256: &PROFILE_DIGEST,
            },
        ),
        Err(HubStoreError::Conflict { .. })
    ));
    let group = fixture
        .store
        .create_group("Intent Group", "intent-group")
        .expect("create Group for scope check");
    let grouped = fixture
        .create_owned_conversation_with_scope("grouped", &ConversationScope::Group(group.id));
    assert!(matches!(
        fixture.submit(
            &fixture.owner,
            SubmitPendingRunIntent {
                conversation_id: &grouped,
                content: "group prompt",
                idempotency_key: "group-key",
                expected_version: 1,
                profile_id: PROFILE_ID,
                profile_sha256: &PROFILE_DIGEST,
            },
        ),
        Err(HubStoreError::Conflict { .. })
    ));
}

#[test]
fn owner_filtered_list_and_timeline_use_uniform_not_found() {
    let fixture = Fixture::new(true);
    let result = submit(
        &mut fixture.connection(),
        fixture.input("read-key", 1, "private"),
    )
    .expect("submit pending intent");
    let foreign = ConversationOwner {
        subject: "another-subject".into(),
        ..fixture.owner.clone()
    };
    let foreign_list_error = fixture
        .store
        .owned_pending_run_intent_page(&foreign, &fixture.conversation_id, None, 10)
        .expect_err("foreign Conversation list is not found");
    assert_not_found_conversation(&foreign_list_error);
    let missing = fixture
        .store
        .owned_pending_run_intent_timeline_page(
            &fixture.owner,
            &fixture.conversation_id,
            "missing-intent",
            0,
            10,
        )
        .expect_err("missing intent timeline is not found");
    assert!(matches!(
        missing,
        HubStoreError::NotFound {
            entity: HubEntity::PendingRunIntent,
            ..
        }
    ));
    let foreign_conversation = fixture.create_owned_conversation("foreign-conversation");
    let foreign_owner = ConversationOwner {
        tenant_id: "foreign".into(),
        ..fixture.owner.clone()
    };
    let hidden = fixture
        .store
        .owned_pending_run_intent_timeline_page(
            &foreign_owner,
            &foreign_conversation,
            &result.intent.intent_id,
            0,
            10,
        )
        .expect_err("foreign timeline lookup is uniformly not found");
    assert!(matches!(hidden, HubStoreError::NotFound { .. }));
}

#[test]
fn every_prompt_intent_and_timeline_fault_point_rolls_back_change_feed_too() {
    for failing_stage in [
        SubmitStage::PromptRow,
        SubmitStage::ConversationTimestamp,
        SubmitStage::ChangeJournal,
        SubmitStage::IntentRow,
        SubmitStage::TimelineEvent,
    ] {
        let fixture = Fixture::new(true);
        let mut connection = fixture.connection();
        let error = submit_with_hooks(
            &mut connection,
            fixture.input("rollback-key", 1, "rollback"),
            rows::now_ms,
            |stage| {
                if stage == failing_stage {
                    Err(HubStoreError::Conflict {
                        entity: HubEntity::PendingRunIntent,
                        message: "injected atomicity fault".into(),
                    })
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("fault aborts all submission writes");
        assert!(matches!(error, HubStoreError::Conflict { .. }));
        assert_eq!(count(&connection, "prompts"), 0, "stage {failing_stage:?}");
        assert_eq!(count(&connection, "pending_run_intents"), 0);
        assert_eq!(count(&connection, "pending_run_intent_events"), 0);
        assert_eq!(count(&connection, "runs"), 0);
        assert_eq!(count(&connection, "run_events"), 0);
        assert_eq!(
            connection
                .query_row(
                    "SELECT last_version FROM conversation_change_heads WHERE conversation_id = ?1",
                    [&fixture.conversation_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("read unchanged Conversation head"),
            1
        );
        assert_eq!(count(&connection, "conversation_changes"), 1);
    }
}

fn count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count fixture rows")
}

fn assert_conflict<T>(result: &Result<T, HubStoreError>) {
    assert!(matches!(result, Err(HubStoreError::Conflict { .. })));
}

fn assert_not_found_conversation(error: &HubStoreError) {
    assert!(matches!(
        error,
        HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            ..
        }
    ));
}
