use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
};

use forge_runtime_domain::{
    Conversation, ConversationScope, GROUP_RUN_VERSION, GroupContextPolicy, GroupRunStore,
    HubEntity, HubStore, HubStoreError, PrepareGroupRun, PrepareGroupRunDisposition, SessionGroup,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use tempfile::TempDir;

mod sqlite_group_run_support;

use sqlite_group_run_support::{snapshot_hash, tamper_rows};

struct Fixture {
    root: TempDir,
    database: PathBuf,
    store: SqliteHubStore,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("Group Run root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        Self {
            root,
            database,
            store,
        }
    }

    fn group(&self, name: &str, key: &str) -> SessionGroup {
        self.store.create_group(name, key).expect("Group")
    }

    fn project_conversation(&self, group: &SessionGroup, name: &str, role: &str) -> Conversation {
        let path = self.root.path().join(name);
        fs::create_dir(&path).expect("project directory");
        let project = self
            .store
            .open_project(&path.canonicalize().expect("canonical path"))
            .expect("Project");
        self.store
            .add_project_to_group(&group.id, &project.id, role, &format!("link-{name}"))
            .expect("link member");
        self.store
            .create_conversation(
                &ConversationScope::Project(project.id),
                name,
                &format!("conversation-{name}"),
            )
            .expect("Conversation")
    }

    fn prompt(&self, conversation: &Conversation, role: &str, content: &str, key: &str) {
        self.store
            .append_prompt(&conversation.id, role, content, key)
            .expect("Prompt");
    }
}

#[test]
fn prepare_freezes_exact_context_and_replay_ignores_new_history() {
    let fixture = Fixture::new();
    let group = fixture.group("SSO delivery", "group-key");
    let conversation = fixture.project_conversation(&group, "frontend", "frontend");
    let secret_path = fixture.root.path().display().to_string();
    fixture.prompt(
        &conversation,
        "user",
        "wire \"SSO\"\\callback\nwithout tokens 🦀",
        "prompt-secret-key",
    );
    let first = fixture
        .store
        .prepare_group_run(&request(&group.id, "group-run-1", "prepare-key", 10))
        .expect("prepare Group Run");
    assert_eq!(first.disposition, PrepareGroupRunDisposition::Created);
    assert_snapshot_contract(&first.snapshot);
    assert!(!first.snapshot.context_json.contains(&secret_path));
    assert!(!first.snapshot.context_json.contains("prompt-secret-key"));

    fixture.prompt(&conversation, "user", "new history", "new-prompt-key");
    let replay = fixture
        .store
        .prepare_group_run(&request(&group.id, "ignored-id", "prepare-key", 99))
        .expect("idempotent replay");
    assert_eq!(replay.disposition, PrepareGroupRunDisposition::Replayed);
    assert_eq!(replay.snapshot, first.snapshot);

    let second = fixture
        .store
        .prepare_group_run(&request(&group.id, "group-run-2", "second-key", 11))
        .expect("fresh snapshot");
    assert_ne!(
        second.snapshot.run.context_slice_sha256,
        first.snapshot.run.context_slice_sha256
    );
    assert_snapshot_remains_durable(&fixture, &group.id, &first.snapshot);
}

#[test]
fn snapshot_digest_identifies_content_not_local_request_metadata() {
    let fixture = Fixture::new();
    let group = fixture.group("Stable content", "group-key");
    let first = fixture
        .store
        .prepare_group_run(&request(&group.id, "first-run", "first-key", 1))
        .expect("first snapshot");
    let second = fixture
        .store
        .prepare_group_run(&request(&group.id, "second-run", "second-key", 2))
        .expect("second snapshot");

    assert_eq!(
        first.snapshot.run.snapshot_sha256,
        second.snapshot.run.snapshot_sha256
    );
    assert_eq!(first.snapshot.context_json, second.snapshot.context_json);
}

#[test]
fn idempotency_and_identifier_conflicts_fail_closed() {
    let fixture = Fixture::new();
    let first = fixture.group("First", "first-group-key");
    let second = fixture.group("Second", "second-group-key");
    fixture
        .store
        .prepare_group_run(&request(&first.id, "run-1", "same-key", 1))
        .expect("initial prepare");

    let mut changed_policy = request(&first.id, "ignored", "same-key", 2);
    changed_policy.policy.max_total_content_bytes -= 1;
    assert_group_run_conflict(
        &fixture.store.prepare_group_run(&changed_policy),
        "changed policy",
    );
    let mut invalid_policy = request(&first.id, "ignored", "same-key", 2);
    invalid_policy.policy.max_members = 0;
    assert!(matches!(
        fixture.store.prepare_group_run(&invalid_policy),
        Err(HubStoreError::Conflict {
            entity: HubEntity::Group,
            ..
        })
    ));
    assert_group_run_conflict(
        &fixture
            .store
            .prepare_group_run(&request(&second.id, "ignored", "same-key", 3)),
        "changed Group",
    );
    assert_group_run_conflict(
        &fixture
            .store
            .prepare_group_run(&request(&first.id, "run-1", "new-key", 4)),
        "reused ID",
    );
    assert_missing_and_limit_errors(&fixture.store);
    assert_management_side_effects(&fixture.database, 1);
}

#[test]
fn concurrent_same_key_creates_one_snapshot_and_replays_it() {
    const WORKERS: usize = 8;
    let fixture = Fixture::new();
    let group = fixture.group("Concurrent", "group-key");
    let barrier = Arc::new(Barrier::new(WORKERS));
    let mut workers = Vec::new();
    for index in 0..WORKERS {
        let barrier = Arc::clone(&barrier);
        let store = fixture.store.clone();
        let group_id = group.id.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store.prepare_group_run(&request(
                &group_id,
                &format!("candidate-{index}"),
                "concurrent-key",
                u64::try_from(index).expect("time"),
            ))
        }));
    }
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker").expect("prepare"))
        .collect::<Vec<_>>();
    let created = results
        .iter()
        .filter(|result| result.disposition == PrepareGroupRunDisposition::Created)
        .count();
    assert_eq!(created, 1);
    assert!(
        results
            .windows(2)
            .all(|pair| pair[0].snapshot == pair[1].snapshot)
    );
    assert_management_side_effects(&fixture.database, 1);
}

#[test]
fn concurrent_divergent_key_reuse_allows_only_one_group() {
    let fixture = Fixture::new();
    let first = fixture.group("First", "first-key");
    let second = fixture.group("Second", "second-key");
    let barrier = Arc::new(Barrier::new(2));
    let workers = [first.id, second.id]
        .into_iter()
        .enumerate()
        .map(|(index, group_id)| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.prepare_group_run(&request(
                    &group_id,
                    &format!("divergent-{index}"),
                    "divergent-key",
                    u64::try_from(index).expect("time"),
                ))
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker"))
        .collect::<Vec<_>>();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(HubStoreError::Conflict { .. })))
            .count(),
        1
    );
    assert_management_side_effects(&fixture.database, 1);
}

#[test]
fn invalid_context_rolls_back_the_prepared_row() {
    let fixture = Fixture::new();
    let group = fixture.group("Strict", "group-key");
    let conversation = fixture.project_conversation(&group, "backend", "backend");
    fixture.prompt(&conversation, "system", "not allowed", "invalid-role-key");

    assert!(matches!(
        fixture
            .store
            .prepare_group_run(&request(&group.id, "invalid-run", "invalid-key", 1)),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_management_side_effects(&fixture.database, 0);
}

#[test]
fn replay_is_key_first_when_current_group_history_becomes_invalid() {
    let fixture = Fixture::new();
    let group = fixture.group("Frozen", "group-key");
    let conversation = fixture.project_conversation(&group, "api", "backend");
    fixture.prompt(&conversation, "user", "valid", "valid-key");
    let first = fixture
        .store
        .prepare_group_run(&request(&group.id, "run-1", "stable-key", 1))
        .expect("initial snapshot");
    fixture.prompt(&conversation, "system", "invalid later role", "invalid-key");

    let replay = fixture
        .store
        .prepare_group_run(&request(&group.id, "ignored", "stable-key", 2))
        .expect("replay does not query current history");
    assert_eq!(replay.snapshot, first.snapshot);
    assert!(matches!(
        fixture
            .store
            .prepare_group_run(&request(&group.id, "run-2", "new-key", 3)),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_management_side_effects(&fixture.database, 1);
}

#[test]
fn missing_group_does_not_persist_a_prepared_run() {
    let fixture = Fixture::new();

    assert!(matches!(
        fixture
            .store
            .prepare_group_run(&request("missing", "run-1", "key-1", 1)),
        Err(HubStoreError::NotFound {
            entity: HubEntity::Group,
            ..
        })
    ));
    assert_management_side_effects(&fixture.database, 0);
}

#[test]
fn list_reads_bounded_metadata_while_inspect_verifies_the_snapshot_body() {
    let fixture = Fixture::new();
    let group = fixture.group("Metadata only", "group-key");
    fixture
        .store
        .prepare_group_run(&request(&group.id, "run-1", "key-1", 1))
        .expect("seed Group Run");
    let connection = Connection::open(&fixture.database).expect("raw SQLite");
    connection
        .execute(
            "UPDATE group_runs SET context_blob = X'7B7D' WHERE id = 'run-1'",
            [],
        )
        .expect("tamper body only");

    let listed = fixture
        .store
        .list_group_runs(Some(&group.id), 1)
        .expect("metadata list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].snapshot_bytes, 2);
    assert!(matches!(
        fixture.store.inspect_group_run("run-1"),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn inspect_and_replay_reject_tampered_snapshots_without_rebuilding() {
    let fixture = Fixture::new();
    let group = fixture.group("Integrity", "group-key");
    let other = fixture.group("Other", "other-key");
    for index in 0..7 {
        fixture
            .store
            .prepare_group_run(&request(
                &group.id,
                &format!("run-{index}"),
                &format!("key-{index}"),
                u64::try_from(index).expect("time"),
            ))
            .expect("seed Group Run");
    }
    tamper_rows(&fixture.database, &other.id);

    for index in 0..7 {
        assert!(matches!(
            fixture.store.inspect_group_run(&format!("run-{index}")),
            Err(HubStoreError::Corrupt { .. })
        ));
    }
    assert!(matches!(
        fixture
            .store
            .prepare_group_run(&request(&group.id, "ignored", "key-0", 99)),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        fixture.store.list_group_runs(Some(&group.id), 10),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_eq!(row_count(&fixture.database, "group_runs"), 7);
}

fn request(group_id: &str, run_id: &str, key: &str, created_at_ms: u64) -> PrepareGroupRun {
    PrepareGroupRun {
        v: GROUP_RUN_VERSION,
        run_id: run_id.into(),
        group_id: group_id.into(),
        policy: GroupContextPolicy::default(),
        idempotency_key: key.into(),
        created_at_ms,
    }
}

fn assert_snapshot_contract(snapshot: &forge_runtime_domain::GroupRunSnapshot) {
    let decoded: forge_runtime_domain::GroupContextSlice =
        serde_json::from_str(&snapshot.context_json).expect("snapshot JSON");
    assert_eq!(decoded, snapshot.context);
    assert_eq!(snapshot.run.snapshot_bytes, snapshot.context_json.len());
    assert_eq!(
        snapshot.run.snapshot_sha256,
        snapshot_hash(snapshot.context_json.as_bytes())
    );
    assert_eq!(
        snapshot.run.context_slice_sha256,
        snapshot.context.slice_sha256
    );
}

fn assert_snapshot_remains_durable(
    fixture: &Fixture,
    group_id: &str,
    snapshot: &forge_runtime_domain::GroupRunSnapshot,
) {
    assert_eq!(
        fixture
            .store
            .inspect_group_run(&snapshot.run.run_id)
            .expect("inspect"),
        *snapshot
    );
    assert_listed_runs(&fixture.store, group_id, &["group-run-2", "group-run-1"]);
    let reopened = SqliteHubStore::open(&fixture.database).expect("reopen Hub");
    assert_eq!(
        reopened
            .inspect_group_run(&snapshot.run.run_id)
            .expect("reopened snapshot"),
        *snapshot
    );
    assert_management_side_effects(&fixture.database, 2);
}

fn assert_group_run_conflict<T>(result: &Result<T, HubStoreError>, subject: &str) {
    assert!(
        matches!(
            result,
            Err(HubStoreError::Conflict {
                entity: HubEntity::GroupRun,
                ..
            })
        ),
        "{subject} should conflict"
    );
}

fn assert_listed_runs(store: &SqliteHubStore, group_id: &str, expected: &[&str]) {
    let listed = store
        .list_group_runs(Some(group_id), 10)
        .expect("list Group Runs");
    let actual = listed
        .iter()
        .map(|run| run.run_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn assert_missing_and_limit_errors(store: &SqliteHubStore) {
    assert!(matches!(
        store.inspect_group_run("missing"),
        Err(HubStoreError::NotFound {
            entity: HubEntity::GroupRun,
            ..
        })
    ));
    assert!(matches!(
        store.list_group_runs(Some("missing"), 10),
        Err(HubStoreError::NotFound {
            entity: HubEntity::Group,
            ..
        })
    ));
    for limit in [0, 101] {
        assert_group_run_conflict(&store.list_group_runs(None, limit), "bad limit");
    }
}

fn assert_management_side_effects(database: &Path, expected_group_runs: i64) {
    assert_eq!(row_count(database, "group_runs"), expected_group_runs);
    assert_eq!(row_count(database, "runs"), 0);
    assert_eq!(row_count(database, "run_events"), 0);
    assert_eq!(row_count(database, "run_assistant_prompts"), 0);
}

fn row_count(database: &Path, table: &str) -> i64 {
    let connection = Connection::open(database).expect("raw SQLite");
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
