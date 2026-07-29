use std::{
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
};

use forge_runtime_domain::{
    BeginGroupExecution, BeginGroupExecutionDisposition, GROUP_EXECUTION_PROTOCOL_VERSION,
    GROUP_EXECUTION_VERSION, GROUP_RUN_VERSION, GroupContextPolicy, GroupExecutionEvent,
    GroupExecutionEventKind, GroupExecutionMode, GroupExecutionOutcome, GroupExecutionReceipt,
    GroupExecutionRecovery, GroupExecutionStatus, GroupExecutionStore, GroupRunSnapshot,
    GroupRunStore, HubEntity, HubStore, HubStoreError, PrepareGroupRun, SessionGroup,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    database: PathBuf,
    store: SqliteHubStore,
    group: SessionGroup,
    source: GroupRunSnapshot,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("Group Execution root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        let group = store
            .create_group("private frozen dossier", "group-key")
            .expect("Group");
        let source = prepare_source(&store, &group.id, "group-run-1", "prepare-key", 1);
        Self {
            _root: root,
            database,
            store,
            group,
            source,
        }
    }

    fn begin(&self, execution_id: &str, key: &str, time: u64) -> BeginGroupExecution {
        begin_request(&self.source.run.run_id, execution_id, key, time)
    }
}

#[test]
fn durable_prefix_resumes_and_the_third_event_completes_atomically() {
    let fixture = Fixture::new();
    let request = fixture.begin("execution-1", "execution-key", 10);
    let begun = fixture
        .store
        .begin_group_execution(&request)
        .expect("begin");
    assert_eq!(begun.disposition, BeginGroupExecutionDisposition::Created);
    assert_eq!(begun.snapshot, fixture.source);
    let events = expected_events(&begun.execution, &begun.snapshot);

    append_start_and_reject_gap(&fixture.store, &events);
    fixture
        .store
        .append_group_execution_event(&events[1])
        .expect("append verification");
    assert_prefix(&fixture.store, "execution-1", 2, true);
    fixture
        .store
        .append_group_execution_event(&events[2])
        .expect("append finish");
    assert_terminal_and_replay(&fixture.store, &events);
    assert_terminal_storage(&fixture.database);
    assert_zero_project_run_side_effects(&fixture.database);
}

#[test]
fn replay_is_key_first_and_one_source_accepts_multiple_execution_keys() {
    let fixture = Fixture::new();
    let first = fixture
        .store
        .begin_group_execution(&fixture.begin("execution-1", "stable-key", 10))
        .expect("first begin");
    let replay = fixture
        .store
        .begin_group_execution(&fixture.begin("", "stable-key", u64::MAX))
        .expect("candidate identity and time are ignored");
    assert_eq!(replay.disposition, BeginGroupExecutionDisposition::Replayed);
    assert_eq!(replay.execution, first.execution);
    assert_eq!(replay.snapshot, first.snapshot);

    assert_multiple_keys_and_id_collision(&fixture, &first.execution.source_snapshot_sha256);
    assert_divergent_source_identity_conflicts(&fixture);
    assert_zero_project_run_side_effects(&fixture.database);
}

fn assert_multiple_keys_and_id_collision(fixture: &Fixture, source_digest: &str) {
    let second = fixture
        .store
        .begin_group_execution(&fixture.begin("execution-2", "second-key", 11))
        .expect("same source with another key");
    assert_eq!(second.disposition, BeginGroupExecutionDisposition::Created);
    assert_eq!(second.execution.source_snapshot_sha256, source_digest);
    assert_eq!(
        fixture
            .store
            .list_group_executions(Some(&fixture.source.run.run_id), 10)
            .expect("list")
            .len(),
        2
    );
    assert!(matches!(
        fixture
            .store
            .begin_group_execution(&fixture.begin("execution-1", "third-key", 12)),
        Err(HubStoreError::Conflict { .. })
    ));
}

fn assert_divergent_source_identity_conflicts(fixture: &Fixture) {
    let other = prepare_source(
        &fixture.store,
        &fixture.group.id,
        "group-run-2",
        "prepare-key-2",
        2,
    );
    assert_eq!(
        other.run.snapshot_sha256,
        fixture.source.run.snapshot_sha256
    );
    assert!(matches!(
        fixture.store.begin_group_execution(&begin_request(
            &other.run.run_id,
            "ignored",
            "stable-key",
            99,
        )),
        Err(HubStoreError::Conflict { .. })
    ));
}

#[test]
fn concurrent_same_key_creates_exactly_one_execution() {
    const WORKERS: usize = 8;
    let fixture = Fixture::new();
    let barrier = Arc::new(Barrier::new(WORKERS));
    let workers = (0..WORKERS)
        .map(|index| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            let source_id = fixture.source.run.run_id.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.begin_group_execution(&begin_request(
                    &source_id,
                    &format!("candidate-{index}"),
                    "concurrent-key",
                    u64::try_from(index).expect("time"),
                ))
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker").expect("begin"))
        .collect::<Vec<_>>();

    assert_eq!(
        results
            .iter()
            .filter(|result| result.disposition == BeginGroupExecutionDisposition::Created)
            .count(),
        1
    );
    assert!(
        results
            .windows(2)
            .all(|pair| pair[0].execution == pair[1].execution)
    );
    assert_eq!(row_count(&fixture.database, "group_executions"), 1);
    assert_zero_project_run_side_effects(&fixture.database);
}

#[test]
fn corrupt_bodies_remain_listable_but_fail_every_validating_path() {
    let fixture = Fixture::new();
    let request = fixture.begin("execution-1", "secret-execution-key", 10);
    let begun = fixture
        .store
        .begin_group_execution(&request)
        .expect("begin");
    let events = expected_events(&begun.execution, &begun.snapshot);
    fixture
        .store
        .append_group_execution_event(&events[0])
        .expect("append");
    let connection = Connection::open(&fixture.database).expect("raw SQLite");
    connection
        .execute(
            "UPDATE group_execution_events SET event_json='{\"broken\":true}'
             WHERE execution_id='execution-1' AND seq=1",
            [],
        )
        .expect("tamper event body");

    let listed = fixture
        .store
        .list_group_executions(Some(&fixture.source.run.run_id), 10)
        .expect("metadata-only list");
    assert_eq!(listed.len(), 1);
    let output = serde_json::to_string(&listed).expect("record JSON");
    for private in [
        "secret-execution-key",
        "private frozen dossier",
        "execution_started",
        "cursor_json",
    ] {
        assert!(!output.contains(private), "list leaked {private}");
    }
    assert_validating_paths_corrupt(&fixture, &request, &events[0]);
    assert_zero_project_run_side_effects(&fixture.database);
}

#[test]
fn corrupt_frozen_source_remains_listable_but_invalidates_execution_paths() {
    let fixture = Fixture::new();
    let request = fixture.begin("execution-1", "execution-key", 10);
    let begun = fixture
        .store
        .begin_group_execution(&request)
        .expect("begin");
    let event = expected_events(&begun.execution, &begun.snapshot)[0].clone();
    Connection::open(&fixture.database)
        .expect("raw SQLite")
        .execute(
            "UPDATE group_runs SET context_blob=X'7B7D' WHERE id='group-run-1'",
            [],
        )
        .expect("tamper frozen source");

    let listed = fixture
        .store
        .list_group_executions(Some("group-run-1"), 10)
        .expect("metadata remains listable");
    assert_eq!(listed.len(), 1);
    assert_validating_paths_corrupt(&fixture, &request, &event);
}

fn append_start_and_reject_gap(store: &SqliteHubStore, events: &[GroupExecutionEvent; 3]) {
    assert_prefix(store, "execution-1", 0, false);
    store
        .append_group_execution_event(&events[0])
        .expect("append start");
    store
        .append_group_execution_event(&events[0])
        .expect("exact start replay");
    assert_prefix(store, "execution-1", 1, false);
    assert!(matches!(
        store.append_group_execution_event(&events[2]),
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupExecution,
            ..
        })
    ));
}

fn assert_terminal_and_replay(store: &SqliteHubStore, events: &[GroupExecutionEvent; 3]) {
    let terminal = store
        .inspect_group_execution("execution-1")
        .expect("inspect terminal");
    assert_eq!(terminal.execution.status, GroupExecutionStatus::Completed);
    assert!(matches!(
        terminal.recovery,
        GroupExecutionRecovery::Terminal {
            outcome: GroupExecutionOutcome::SnapshotValidated
        }
    ));
    store
        .append_group_execution_event(&events[2])
        .expect("exact terminal replay");
    let mut after_terminal = events[2].clone();
    after_terminal.seq = 4;
    assert!(matches!(
        store.append_group_execution_event(&after_terminal),
        Err(HubStoreError::Conflict { .. })
    ));
}

fn assert_validating_paths_corrupt(
    fixture: &Fixture,
    request: &BeginGroupExecution,
    event: &GroupExecutionEvent,
) {
    assert_corrupt(
        &fixture.store.inspect_group_execution("execution-1"),
        "inspect",
    );
    assert_corrupt(
        &fixture.store.append_group_execution_event(event),
        "append replay",
    );
    assert_corrupt(
        &fixture.store.begin_group_execution(request),
        "begin replay",
    );
}

fn prepare_source(
    store: &SqliteHubStore,
    group_id: &str,
    run_id: &str,
    key: &str,
    created_at_ms: u64,
) -> GroupRunSnapshot {
    store
        .prepare_group_run(&PrepareGroupRun {
            v: GROUP_RUN_VERSION,
            run_id: run_id.into(),
            group_id: group_id.into(),
            policy: GroupContextPolicy::default(),
            idempotency_key: key.into(),
            created_at_ms,
        })
        .expect("prepare source")
        .snapshot
}

fn begin_request(
    group_run_id: &str,
    execution_id: &str,
    key: &str,
    created_at_ms: u64,
) -> BeginGroupExecution {
    BeginGroupExecution {
        v: GROUP_EXECUTION_VERSION,
        execution_id: execution_id.into(),
        group_run_id: group_run_id.into(),
        mode: GroupExecutionMode::OfflineSnapshotValidation,
        idempotency_key: key.into(),
        created_at_ms,
    }
}

fn expected_events(
    execution: &forge_runtime_domain::GroupExecutionRecord,
    snapshot: &GroupRunSnapshot,
) -> [GroupExecutionEvent; 3] {
    let receipt = GroupExecutionReceipt {
        v: GROUP_EXECUTION_VERSION,
        execution_id: execution.execution_id.clone(),
        group_run_id: snapshot.run.run_id.clone(),
        group_id: snapshot.run.group_id.clone(),
        context_version: snapshot.run.context_version,
        context_slice_sha256: snapshot.run.context_slice_sha256.clone(),
        snapshot_sha256: snapshot.run.snapshot_sha256.clone(),
        snapshot_bytes: snapshot.run.snapshot_bytes,
        stats: snapshot.context.payload.stats.clone(),
    };
    [
        event(
            execution,
            1,
            GroupExecutionEventKind::ExecutionStarted {
                group_run_id: snapshot.run.run_id.clone(),
                snapshot_sha256: snapshot.run.snapshot_sha256.clone(),
            },
        ),
        event(
            execution,
            2,
            GroupExecutionEventKind::SnapshotVerified { receipt },
        ),
        event(
            execution,
            3,
            GroupExecutionEventKind::ExecutionFinished {
                outcome: GroupExecutionOutcome::SnapshotValidated,
            },
        ),
    ]
}

fn event(
    execution: &forge_runtime_domain::GroupExecutionRecord,
    seq: u64,
    kind: GroupExecutionEventKind,
) -> GroupExecutionEvent {
    GroupExecutionEvent {
        v: GROUP_EXECUTION_PROTOCOL_VERSION,
        execution_id: execution.execution_id.clone(),
        seq,
        kind,
    }
}

fn assert_prefix(store: &SqliteHubStore, id: &str, length: usize, has_receipt: bool) {
    let inspection = store.inspect_group_execution(id).expect("inspect prefix");
    assert_eq!(inspection.events.len(), length);
    assert_eq!(inspection.receipt.is_some(), has_receipt);
    assert_eq!(inspection.recovery, GroupExecutionRecovery::Incomplete);
}

fn assert_terminal_storage(database: &Path) {
    let connection = Connection::open(database).expect("raw SQLite");
    let stored: (String, i64, i64) = connection
        .query_row(
            "SELECT status,journal_bytes,
               (SELECT COUNT(*) FROM group_execution_events WHERE execution_id='execution-1')
             FROM group_executions WHERE id='execution-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("terminal row");
    assert_eq!(stored.0, "completed");
    assert!(stored.1 > 0);
    assert_eq!(stored.2, 3);
}

fn assert_corrupt<T>(result: &Result<T, HubStoreError>, subject: &str) {
    assert!(
        matches!(result, Err(HubStoreError::Corrupt { .. })),
        "{subject} must reject corruption"
    );
}

fn assert_zero_project_run_side_effects(database: &Path) {
    for table in ["projects", "runs", "run_events", "run_assistant_prompts"] {
        assert_eq!(row_count(database, table), 0, "unexpected row in {table}");
    }
}

fn row_count(database: &Path, table: &str) -> i64 {
    Connection::open(database)
        .expect("raw SQLite")
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
