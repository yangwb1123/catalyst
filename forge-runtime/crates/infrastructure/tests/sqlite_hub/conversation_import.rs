use forge_runtime_domain::{
    ConversationImportPrompt, ConversationOwner, ConversationScope, HubEntity, HubStore,
    HubStoreError,
};
use rusqlite::Connection;

use super::fixture;

#[test]
fn import_is_atomic_owner_bound_and_idempotent_after_later_prompts() {
    let (_root, store) = fixture();
    let account = owner("issuer", "subject", "tenant");
    let prompts = [
        import_prompt("user", "please review this change"),
        import_prompt("assistant", "I will inspect the request."),
    ];
    let imported = store
        .import_owned_conversation(&account, "Review", &prompts, "stable-import-key")
        .expect("import transcript atomically");
    assert!(!imported.replayed);
    assert_eq!(imported.imported_prompt_count, 2);
    assert_eq!(imported.aggregate_version, 3);
    assert_eq!(imported.conversation.scope, ConversationScope::Global);
    assert_replay_and_owner_feed(&store, &account, &prompts, &imported);
}

fn assert_replay_and_owner_feed(
    store: &forge_runtime_infrastructure::SqliteHubStore,
    account: &ConversationOwner,
    prompts: &[ConversationImportPrompt],
    imported: &forge_runtime_domain::OwnedConversationImportResult,
) {
    let retry = store
        .import_owned_conversation(account, "Review", prompts, "stable-import-key")
        .expect("replay same import");
    assert!(retry.replayed);
    assert_eq!(retry.conversation, imported.conversation);

    store
        .append_owned_prompt(
            account,
            &imported.conversation.id,
            "follow-up",
            "later-prompt",
            imported.aggregate_version,
        )
        .expect("continue imported conversation");
    let retry_after_follow_up = store
        .import_owned_conversation(account, "Review", prompts, "stable-import-key")
        .expect("a later append does not change import replay identity");
    assert!(retry_after_follow_up.replayed);
    assert_eq!(
        retry_after_follow_up.conversation.id,
        imported.conversation.id
    );
    assert_eq!(retry_after_follow_up.imported_prompt_count, prompts.len());
    assert_import_feed_and_owner_filter(store, account, imported);
}

fn assert_import_feed_and_owner_filter(
    store: &forge_runtime_infrastructure::SqliteHubStore,
    account: &ConversationOwner,
    imported: &forge_runtime_domain::OwnedConversationImportResult,
) {
    let feed = store
        .owned_conversation_changes_after(account, 0, 8)
        .expect("read imported owner feed");
    assert_eq!(feed.changes.len(), 4);
    assert_eq!(
        feed.changes
            .iter()
            .map(|change| change.cursor)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    let foreign_history = store.owned_conversation_prompt_page(
        &owner("issuer", "other", "tenant"),
        &imported.conversation.id,
        None,
        8,
    );
    assert!(matches!(
        foreign_history,
        Err(HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            ..
        })
    ));
}

#[test]
fn conflicting_replay_does_not_add_partial_import_rows() {
    let (_root, store) = fixture();
    let account = owner("issuer", "subject", "tenant");
    let original = [import_prompt("user", "source")];
    let imported = store
        .import_owned_conversation(&account, "Review", &original, "stable-import-key")
        .expect("initial import");
    let changed = [import_prompt("user", "changed source")];
    assert!(matches!(
        store.import_owned_conversation(&account, "Review", &changed, "stable-import-key"),
        Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            ..
        })
    ));
    let history = store
        .owned_conversation_prompt_page(&account, &imported.conversation.id, None, 8)
        .expect("read original import");
    assert_eq!(history.prompts.len(), 1);
    assert_eq!(history.prompts[0].content, "source");
}

#[test]
fn failed_prompt_insert_rolls_back_the_conversation_and_all_feed_rows() {
    let (root, store) = fixture();
    let database = root.path().join("hub.sqlite3");
    let connection = Connection::open(&database).expect("open Hub for fault trigger");
    connection
        .execute_batch(
            "CREATE TRIGGER fail_second_import_prompt
             BEFORE INSERT ON prompts
             WHEN (SELECT COUNT(*) FROM prompts WHERE conversation_id = NEW.conversation_id) = 1
             BEGIN SELECT RAISE(ABORT, 'injected import failure'); END;",
        )
        .expect("create fault trigger");
    drop(connection);

    let account = owner("issuer", "subject", "tenant");
    let prompts = [
        import_prompt("user", "first"),
        import_prompt("assistant", "second"),
    ];
    assert!(
        store
            .import_owned_conversation(&account, "Review", &prompts, "rollback-key")
            .is_err()
    );

    Connection::open(&database)
        .expect("open Hub to remove fault trigger")
        .execute_batch("DROP TRIGGER fail_second_import_prompt;")
        .expect("remove fault trigger");

    let conversations = store
        .list_owned_conversations(&account, None, 8)
        .expect("read owner page after rollback");
    assert!(conversations.conversations.is_empty());
    let feed = store
        .owned_conversation_changes_after(&account, 0, 8)
        .expect("read owner feed after rollback");
    assert!(feed.changes.is_empty());
    assert_eq!(feed.scanned_through_cursor, 0);
}

#[test]
fn local_preview_returns_only_ownerless_user_visible_text_without_paths() {
    let (root, store) = fixture();
    let project_dir = root.path().join("source-project");
    let project = store
        .open_project(&create_dir(&project_dir))
        .expect("register source project");
    let source = create_local_preview_source(&store, &project.id);
    let preview = store
        .local_conversation_import_source(&source.id)
        .expect("read ownerless source preview");
    assert_preview_is_safe(&preview, &project.id, &project_dir);

    let account = owner("issuer", "subject", "tenant");
    assert_owned_conversation_is_not_importable(&store, &account);
}

fn create_dir(path: &std::path::Path) -> std::path::PathBuf {
    std::fs::create_dir(path).expect("create project path");
    path.canonicalize().expect("canonical path")
}

fn create_local_preview_source(
    store: &forge_runtime_infrastructure::SqliteHubStore,
    project_id: &str,
) -> forge_runtime_domain::Conversation {
    let source = store
        .create_conversation(
            &ConversationScope::Project(project_id.into()),
            "Local task",
            "local-task",
        )
        .expect("create ownerless local Conversation");
    store
        .append_prompt(&source.id, "user", "question", "local-user")
        .expect("append user text");
    store
        .append_prompt(&source.id, "tool", "private tool payload", "local-tool")
        .expect("append non-user-visible row");
    store
        .append_prompt(&source.id, "assistant", "answer", "local-assistant")
        .expect("append assistant text");
    source
}

fn assert_preview_is_safe(
    preview: &forge_runtime_domain::LocalConversationImportSource,
    project_id: &str,
    project_dir: &std::path::Path,
) {
    assert_eq!(preview.conversation.title, "Local task");
    assert_eq!(
        preview.conversation.scope,
        ConversationScope::Project(project_id.into())
    );
    assert_eq!(preview.content_bytes, "questionanswer".len());
    assert_eq!(
        preview.prompts,
        [
            import_prompt("user", "question"),
            import_prompt("assistant", "answer")
        ]
    );
    let serialized = serde_json::to_string(&preview.conversation).expect("serialize preview");
    assert!(!serialized.contains(&project_dir.to_string_lossy().to_string()));
}

fn assert_owned_conversation_is_not_importable(
    store: &forge_runtime_infrastructure::SqliteHubStore,
    account: &ConversationOwner,
) {
    let owned = store
        .create_owned_conversation(
            account,
            &ConversationScope::Global,
            "Already shared",
            "owned-source",
        )
        .expect("create owner-bound source");
    assert!(store.local_conversation_import_source(&owned.id).is_err());
}

#[test]
fn local_preview_rejects_legacy_blank_visible_prompts_before_confirmation() {
    let (root, store) = fixture();
    let source = store
        .create_conversation(&ConversationScope::Global, "Legacy blank", "legacy-blank")
        .expect("create ownerless Conversation");
    let prompt = store
        .append_prompt(
            &source.id,
            "user",
            "valid before legacy rewrite",
            "local-user",
        )
        .expect("append visible Prompt");
    Connection::open(root.path().join("hub.sqlite3"))
        .expect("open local database for legacy fixture")
        .execute(
            "UPDATE prompts SET content = '  ' WHERE id = ?1",
            [&prompt.id],
        )
        .expect("simulate a legacy blank Prompt");

    assert!(matches!(
        store.local_conversation_import_source(&source.id),
        Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            ..
        })
    ));
}

fn import_prompt(role: &str, content: &str) -> ConversationImportPrompt {
    ConversationImportPrompt {
        role: role.into(),
        content: content.into(),
    }
}

fn owner(issuer: &str, subject: &str, tenant_id: &str) -> ConversationOwner {
    ConversationOwner {
        issuer: issuer.into(),
        subject: subject.into(),
        tenant_id: tenant_id.into(),
    }
}
