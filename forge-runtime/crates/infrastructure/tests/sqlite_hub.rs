use std::fs;

use forge_runtime_domain::{ConversationScope, HubEntity, HubSnapshot, HubStore, HubStoreError};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

fn fixture() -> (TempDir, SqliteHubStore) {
    let root = TempDir::new().expect("temporary Hub root");
    let store = SqliteHubStore::open(root.path().join("hub.sqlite3")).expect("open Hub");
    (root, store)
}

fn project_directory(root: &TempDir, name: &str) -> std::path::PathBuf {
    let path = root.path().join(name);
    fs::create_dir(&path).expect("project directory");
    path.canonicalize().expect("canonical project path")
}

fn snapshot_has_session(snapshot: &HubSnapshot, id: &str) -> bool {
    snapshot
        .conversations
        .iter()
        .any(|conversation| conversation.id == id)
}

#[test]
fn project_sessions_and_prompts_survive_reopen() {
    let (root, store) = fixture();
    let path = project_directory(&root, "frontend");
    let project = store.open_project(&path).expect("register project");
    assert_eq!(
        store.open_project(&path).expect("idempotent project"),
        project
    );

    let global = store
        .create_conversation(&ConversationScope::Global, "General", "session-global")
        .expect("global conversation");
    let scoped = store
        .create_conversation(
            &ConversationScope::Project(project.id.clone()),
            "Frontend",
            "session-frontend",
        )
        .expect("project conversation");
    let prompt = store
        .append_prompt(
            &scoped.id,
            "user",
            "connect the SSO\nwithout leaking tokens",
            "prompt-1",
        )
        .expect("append prompt");
    assert_eq!(
        store
            .append_prompt(&scoped.id, "user", &prompt.content, "prompt-1")
            .expect("idempotent append"),
        prompt
    );

    drop(store);
    let reopened = SqliteHubStore::open(root.path().join("hub.sqlite3")).expect("reopen Hub");
    let snapshot = reopened
        .snapshot(&ConversationScope::Global)
        .expect("global snapshot");
    assert_eq!(snapshot.projects, [project]);
    assert_eq!(snapshot.conversations.len(), 2);
    assert!(snapshot_has_session(&snapshot, &global.id));
    assert!(snapshot_has_session(&snapshot, &scoped.id));
    assert_eq!(
        reopened.list_prompts(None, 50).expect("global prompts"),
        [prompt]
    );
}

#[test]
fn local_group_links_projects_with_roles() {
    let (root, store) = fixture();
    let frontend = store
        .open_project(&project_directory(&root, "frontend"))
        .expect("frontend project");
    let backend = store
        .open_project(&project_directory(&root, "backend"))
        .expect("backend project");
    let group = store
        .create_group("SSO integration", "group-create")
        .expect("group");
    let front_member = store
        .add_project_to_group(&group.id, &frontend.id, "frontend", "group-front")
        .expect("frontend member");
    let back_member = store
        .add_project_to_group(&group.id, &backend.id, "backend", "group-back")
        .expect("backend member");
    let alias_error = store
        .add_project_to_group(&group.id, &backend.id, "backend", "group-back-alias")
        .expect_err("new key cannot alias an existing link");

    let snapshot = store
        .snapshot(&ConversationScope::Group(group.id.clone()))
        .expect("group snapshot");
    assert_eq!(snapshot.groups, [group]);
    assert_eq!(snapshot.projects.len(), 2);
    assert_eq!(snapshot.group_project_members, [back_member, front_member]);
    assert!(matches!(
        alias_error,
        HubStoreError::Conflict {
            entity: HubEntity::GroupProjectMember,
            ..
        }
    ));
}

#[test]
fn idempotency_key_reuse_with_different_prompt_is_rejected() {
    let (_root, store) = fixture();
    let conversation = store
        .create_conversation(&ConversationScope::Global, "General", "session-1")
        .expect("conversation");
    store
        .append_prompt(&conversation.id, "user", "first", "prompt-key")
        .expect("first prompt");

    let error = store
        .append_prompt(&conversation.id, "user", "different", "prompt-key")
        .expect_err("mismatched retry must fail");
    assert!(matches!(
        error,
        HubStoreError::Conflict {
            entity: HubEntity::Prompt,
            ..
        }
    ));
}

#[test]
fn missing_entities_fail_closed() {
    let (root, store) = fixture();
    let project = store
        .open_project(&project_directory(&root, "identity"))
        .expect("project");
    let error = store
        .add_project_to_group("missing", &project.id, "sso", "link-1")
        .expect_err("unknown group denied");
    assert!(matches!(
        error,
        HubStoreError::NotFound {
            entity: HubEntity::Group,
            ..
        }
    ));
    let error = store
        .list_conversations(&ConversationScope::Group("missing".into()))
        .expect_err("unknown group list denied");
    assert!(matches!(
        error,
        HubStoreError::NotFound {
            entity: HubEntity::Group,
            ..
        }
    ));
    let error = store
        .list_prompts(Some("missing"), 10)
        .expect_err("unknown conversation prompt list denied");
    assert!(matches!(
        error,
        HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            ..
        }
    ));
}

#[test]
fn unsupported_schema_version_is_rejected() {
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw SQLite");
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .expect("set baseline journal mode");
    connection
        .pragma_update(None, "user_version", 99)
        .expect("tamper schema version");
    drop(connection);

    let error = SqliteHubStore::open(&database).expect_err("unknown schema denied");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = rusqlite::Connection::open(database).expect("reopen raw SQLite");
    let version: i64 = unchanged
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    let journal: String = unchanged
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("read journal mode");
    assert_eq!(version, 99);
    assert_eq!(journal, "delete");
}

#[test]
fn concurrent_first_open_serializes_schema_creation() {
    use std::sync::{Arc, Barrier};

    const ROUNDS: usize = 8;
    const WORKERS: usize = 16;

    let root = TempDir::new().expect("state directory");
    for round in 0..ROUNDS {
        let database = root.path().join(format!("concurrent-{round}.sqlite3"));
        let barrier = Arc::new(Barrier::new(WORKERS));
        let mut workers = Vec::with_capacity(WORKERS);
        for _ in 0..WORKERS {
            let barrier = Arc::clone(&barrier);
            let database = database.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                SqliteHubStore::open(database)
            }));
        }
        for worker in workers {
            worker
                .join()
                .expect("worker does not panic")
                .expect("concurrent open succeeds");
        }
    }
}

#[test]
fn locked_first_open_retries_the_complete_initialization() {
    use std::{
        sync::{Arc, Barrier},
        time::Duration,
    };

    let root = TempDir::new().expect("state directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("private state permissions");
    }
    let database = root.path().join("locked-first-open.sqlite3");
    let blocker = rusqlite::Connection::open(&database).expect("open lock holder");
    blocker
        .execute_batch("BEGIN EXCLUSIVE")
        .expect("hold exclusive initialization lock");
    let barrier = Arc::new(Barrier::new(2));
    let worker_barrier = Arc::clone(&barrier);
    let worker_database = database.clone();
    let worker = std::thread::spawn(move || {
        worker_barrier.wait();
        SqliteHubStore::open(worker_database)
    });

    barrier.wait();
    std::thread::sleep(Duration::from_millis(2_300));
    blocker.execute_batch("COMMIT").expect("release lock");
    worker
        .join()
        .expect("worker does not panic")
        .expect("first open retries after lock release");
}

#[cfg(unix)]
#[test]
fn database_permissions_are_private_and_symlinks_are_denied() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    assert_eq!(
        fs::metadata(root.path())
            .expect("directory metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&database)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let link = root.path().join("linked.sqlite3");
    symlink(&database, &link).expect("database link");
    let error = SqliteHubStore::open(link).expect_err("symlink denied");
    assert!(matches!(error, HubStoreError::Unavailable { .. }));
}

#[cfg(unix)]
#[test]
fn existing_shared_state_directory_is_rejected_without_chmod() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().expect("temporary root");
    let shared = root.path().join("shared");
    fs::create_dir(&shared).expect("shared directory");
    fs::write(shared.join("unrelated"), "must remain untouched").expect("unrelated file");
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o755)).expect("shared permissions");

    let error =
        SqliteHubStore::open(shared.join("hub.sqlite3")).expect_err("shared directory denied");
    assert!(matches!(error, HubStoreError::Unavailable { .. }));
    assert_eq!(
        fs::metadata(&shared)
            .expect("shared metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}
