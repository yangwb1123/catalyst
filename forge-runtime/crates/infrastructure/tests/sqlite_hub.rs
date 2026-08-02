use std::{collections::BTreeMap, fs, path::Path};

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

#[test]
fn effect_free_open_requires_an_existing_database_without_creating_state() {
    let root = TempDir::new().expect("temporary root");
    let state = root.path().join("private-state");
    fs::create_dir(&state).expect("private state directory");
    make_private_directory(&state);
    let database = state.join("hub.sqlite3");
    let before = state_files(&state);

    let error = SqliteHubStore::open_existing_current_read_only(&database)
        .expect_err("missing Hub is rejected");

    assert!(matches!(error, HubStoreError::Unavailable { .. }));
    assert_eq!(state_files(&state), before);
    for name in [
        "hub.sqlite3",
        "hub.sqlite3-wal",
        "hub.sqlite3-shm",
        "hub.sqlite3-journal",
    ] {
        assert!(!state.join(name).exists(), "created {name}");
    }
}

#[cfg(unix)]
#[test]
fn effect_free_open_does_not_chmod_an_empty_shared_state_directory() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().expect("temporary root");
    let state = root.path().join("shared-state");
    fs::create_dir(&state).expect("shared state directory");
    fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).expect("shared mode");

    let error = SqliteHubStore::open_existing_current_read_only(state.join("hub.sqlite3"))
        .expect_err("shared state is rejected without chmod");

    assert!(matches!(error, HubStoreError::Unavailable { .. }));
    assert_eq!(
        fs::metadata(&state).expect("metadata").permissions().mode() & 0o777,
        0o755
    );
    assert!(state_files(&state).is_empty());
}

#[test]
fn effect_free_open_reads_current_state_and_cannot_write_or_create_sidecars() {
    let (root, store) = fixture();
    store
        .create_group("read-only fixture", "read-only-group")
        .expect("create group");
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let before = state_files(root.path());

    let read_only = SqliteHubStore::open_existing_current_read_only(&database)
        .expect("open current Hub without effects");
    assert_eq!(
        read_only.list_groups().expect("read groups")[0].name,
        "read-only fixture"
    );
    let error = read_only
        .create_group("forbidden", "forbidden-write")
        .expect_err("read-only store rejects writes");
    assert!(matches!(error, HubStoreError::Unavailable { .. }));
    drop(read_only);

    assert_eq!(state_files(root.path()), before);
    assert!(!root.path().join("hub.sqlite3-wal").exists());
    assert!(!root.path().join("hub.sqlite3-shm").exists());
    assert!(!root.path().join("hub.sqlite3-journal").exists());
}

#[test]
fn effect_free_open_rejects_non_current_and_corrupt_databases_without_changes() {
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw Hub");
    connection
        .pragma_update(None, "user_version", 10)
        .expect("make schema non-current");
    drop(connection);
    let before = state_files(root.path());

    let error = SqliteHubStore::open_existing_current_read_only(&database)
        .expect_err("non-current Hub is rejected");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert!(
        error
            .to_string()
            .contains("current schema version 14; found 10")
    );
    assert_eq!(state_files(root.path()), before);

    let corrupt_root = TempDir::new().expect("corrupt root");
    let corrupt = corrupt_root.path().join("hub.sqlite3");
    drop(SqliteHubStore::open(&corrupt).expect("initialize private corrupt fixture"));
    fs::write(&corrupt, b"not a SQLite database").expect("corrupt database fixture");
    let before = state_files(corrupt_root.path());
    let error = SqliteHubStore::open_existing_current_read_only(&corrupt)
        .expect_err("corrupt Hub is rejected");
    assert!(
        matches!(error, HubStoreError::Corrupt { .. }),
        "unexpected error: {error:?}"
    );
    assert_eq!(state_files(corrupt_root.path()), before);
}

#[test]
fn effect_free_open_rejects_live_uncheckpointed_wal_without_reading_stale_main() {
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let writer = rusqlite::Connection::open(&database).expect("open WAL writer");
    writer
        .pragma_update(None, "wal_autocheckpoint", 0)
        .expect("disable automatic checkpoint");
    writer
        .execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE uncheckpointed_security_state(value TEXT NOT NULL);
             INSERT INTO uncheckpointed_security_state(value) VALUES ('must-not-be-ignored');
             COMMIT;",
        )
        .expect("commit only to live WAL");
    assert!(root.path().join("hub.sqlite3-wal").exists());
    assert!(root.path().join("hub.sqlite3-shm").exists());
    let before = state_files(root.path());

    let error = SqliteHubStore::open_existing_current_read_only(&database)
        .expect_err("live WAL is rejected before immutable read");

    assert!(matches!(error, HubStoreError::Unavailable { .. }));
    assert_eq!(state_files(root.path()), before);
    drop(writer);
}

fn state_files(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .expect("read state directory")
        .map(|entry| {
            let entry = entry.expect("state entry");
            let name = entry
                .file_name()
                .into_string()
                .expect("UTF-8 state filename");
            let bytes = fs::read(entry.path()).expect("read state file");
            (name, bytes)
        })
        .collect()
}

#[cfg(unix)]
fn make_private_directory(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("private fixture");
}

#[cfg(not(unix))]
fn make_private_directory(_path: &Path) {}
