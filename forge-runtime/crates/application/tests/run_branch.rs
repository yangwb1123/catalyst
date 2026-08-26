use std::{
    fs,
    sync::{Arc, Barrier},
};

use forge_runtime_application::{PrepareRunBranch, PrepareRunRestart, RunError, RunService};
use forge_runtime_domain::{
    BeginRun, BeginRunDisposition, ConversationScope, HubStore, Message, PROTOCOL_VERSION,
    RUN_STORE_VERSION, RunBranchMode, RunExecution, RunInspection, RunLimits, RunLineageRecord,
    RunOutcome, RunProvider, RunResumePoint, RunStore, RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    store: Arc<SqliteHubStore>,
    source: BeginRun,
}

#[test]
fn terminal_root_branch_is_atomic_queryable_and_exactly_replayable() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let request = branch_request("branch-key", 50);

    let created = service.prepare_branch(&request).expect("prepare branch");
    assert_eq!(created.disposition, BeginRunDisposition::Created);
    assert!(created.inspection.run.run_id.starts_with("run-branch-"));
    assert_eq!(created.inspection.run.execution, fixture.source.execution);
    assert_eq!(created.inspection.events.len(), 1);
    assert!(matches!(
        created.inspection.resume_point(),
        Ok(RunResumePoint::CommitUser { prompt }) if prompt == "branch this run"
    ));
    assert_eq!(created.lineage.parent_run_id, fixture.source.run_id);
    assert_eq!(created.lineage.source_event_seq, 1);
    assert_eq!(created.lineage.branch_mode, RunBranchMode::RootInput);
    assert_eq!(created.lineage.source_event_sha256.len(), 64);

    let replayed = service
        .prepare_branch(&PrepareRunBranch {
            created_at_ms: 999,
            ..request
        })
        .expect("replay branch");
    assert_eq!(replayed.disposition, BeginRunDisposition::Replayed);
    assert_eq!(
        replayed.inspection.run.run_id,
        created.inspection.run.run_id
    );
    assert_eq!(replayed.inspection.events, created.inspection.events);
    assert_eq!(replayed.lineage, created.lineage);
    assert_eq!(
        service
            .run_lineage(&created.inspection.run.run_id)
            .expect("query lineage"),
        Some(created.lineage)
    );
}

#[test]
fn branch_key_is_bound_to_parent_and_operation_namespace() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let created = service
        .prepare_branch(&branch_request("branch-key", 50))
        .expect("prepare branch");
    let second = equivalent_terminal_source(&fixture, &service);
    let before_parent_conflict = durable_state(&service);

    let error = service
        .prepare_branch(&PrepareRunBranch {
            parent_run_id: second.run_id,
            idempotency_key: "branch-key".into(),
            created_at_ms: 60,
        })
        .expect_err("same key cannot select another parent");
    assert!(matches!(error, RunError::BranchIdempotencyConflict));
    assert_eq!(durable_state(&service), before_parent_conflict);

    let mut standard = fixture.source.clone();
    standard.run_id = "ordinary-retry".into();
    standard.idempotency_key = "branch-key".into();
    let before_standard_conflict = durable_state(&service);
    let reverse = service
        .begin_run(&standard)
        .expect_err("ordinary begin cannot replay branch identity");
    assert!(matches!(reverse, RunError::BranchIdempotencyConflict));
    assert_eq!(durable_state(&service), before_standard_conflict);
    assert_eq!(service.list_runs(None, 10).expect("list Runs").len(), 3);
    assert_eq!(created.inspection.events.len(), 1);
}

#[test]
fn branch_owned_key_rejects_restart_without_mutation() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    service
        .prepare_branch(&branch_request("shared-key", 50))
        .expect("prepare branch");
    let before = durable_state(&service);

    let error = service
        .prepare_restart(&PrepareRunRestart {
            source_run_id: fixture.source.run_id.clone(),
            idempotency_key: "shared-key".into(),
            created_at_ms: 60,
        })
        .expect_err("branch-owned key must reject restart");

    assert!(matches!(error, RunError::RestartIdempotencyConflict));
    assert_eq!(durable_state(&service), before);
}

#[test]
fn standard_owned_key_rejects_branch_without_mutation() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let mut standard = fixture.source.clone();
    standard.run_id = "standard-owner".into();
    standard.idempotency_key = "shared-key".into();
    standard.created_at_ms = 40;
    service.begin_run(&standard).expect("prepare standard Run");
    let before = durable_state(&service);

    let error = service
        .prepare_branch(&branch_request("shared-key", 50))
        .expect_err("standard-owned key must reject branch");

    assert!(matches!(error, RunError::BranchIdempotencyConflict));
    assert_eq!(durable_state(&service), before);
}

#[test]
fn branch_namespace_is_reserved_from_standard_begin() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let mut standard = fixture.source.clone();
    standard.run_id = "run-branch-caller-selected".into();
    standard.idempotency_key = "standard-key".into();

    let error = service
        .begin_run(&standard)
        .expect_err("standard begin cannot claim branch namespace");

    assert!(matches!(error, RunError::ReservedBranchRunId));
    assert_eq!(service.list_runs(None, 10).expect("list Runs").len(), 1);
}

#[test]
fn branch_refuses_nonterminal_parent_without_creating_a_child() {
    let fixture = fixture(false);
    let service = RunService::new(fixture.store.clone());

    let error = service
        .prepare_branch(&branch_request("branch-key", 50))
        .expect_err("incomplete parent must fail");

    assert!(matches!(error, RunError::BranchParentNotTerminal));
    assert_eq!(service.list_runs(None, 10).expect("list Runs").len(), 1);
}

#[test]
fn branch_key_owned_by_restart_fails_without_mutating_restart_target() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let restarted = service
        .prepare_restart(&PrepareRunRestart {
            source_run_id: fixture.source.run_id.clone(),
            idempotency_key: "shared-key".into(),
            created_at_ms: 40,
        })
        .expect("prepare restart");
    let before = durable_state(&service);

    let error = service
        .prepare_branch(&branch_request("shared-key", 50))
        .expect_err("restart-owned key must fail");

    assert!(matches!(error, RunError::BranchIdempotencyConflict));
    assert_eq!(durable_state(&service), before);
    assert_eq!(restarted.inspection.events.len(), 1);
    assert_eq!(
        service
            .inspect_run(&restarted.inspection.run.run_id)
            .expect("inspect restart")
            .events
            .len(),
        1
    );
}

#[test]
fn concurrent_parents_with_one_key_commit_exactly_one_branch() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let second = equivalent_terminal_source(&fixture, &service);
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for parent_run_id in [fixture.source.run_id.clone(), second.run_id] {
        let store = fixture.store.clone();
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            RunService::new(store).prepare_branch(&PrepareRunBranch {
                parent_run_id,
                idempotency_key: "concurrent-branch-key".into(),
                created_at_ms: 70,
            })
        }));
    }
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("branch worker"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(RunError::BranchIdempotencyConflict)))
            .count(),
        1
    );
    let durable = durable_state(&service);
    assert_eq!(durable.len(), 3);
    assert_eq!(
        durable
            .iter()
            .filter(|(_, lineage)| lineage.is_some())
            .count(),
        1
    );
}

fn durable_state(service: &RunService) -> Vec<(RunInspection, Option<RunLineageRecord>)> {
    service
        .list_runs(None, 20)
        .expect("list durable Runs")
        .into_iter()
        .map(|run| {
            let inspection = service
                .inspect_run(&run.run_id)
                .expect("inspect durable Run");
            let lineage = service.run_lineage(&run.run_id).expect("inspect lineage");
            (inspection, lineage)
        })
        .collect()
}

fn fixture(terminal: bool) -> Fixture {
    let root = TempDir::new().expect("temporary root");
    let project_path = root.path().join("project");
    fs::create_dir(&project_path).expect("project directory");
    let store = Arc::new(
        SqliteHubStore::open(root.path().join("state").join("hub.sqlite3"))
            .expect("open Hub store"),
    );
    let project = store
        .open_project(&project_path.canonicalize().expect("canonical project"))
        .expect("open Project");
    let conversation = store
        .create_conversation(
            &ConversationScope::Project(project.id.clone()),
            "Branch",
            "conversation-key",
        )
        .expect("create Conversation");
    let prompt = store
        .append_prompt(&conversation.id, "user", "branch this run", "prompt-key")
        .expect("append Prompt");
    let source = BeginRun {
        v: RUN_STORE_VERSION,
        run_id: "source-run".into(),
        conversation_id: conversation.id,
        prompt_id: prompt.id,
        project_id: project.id,
        execution: execution(),
        idempotency_key: "source-key".into(),
        created_at_ms: 10,
    };
    store.begin_run(&source).expect("begin source Run");
    if terminal {
        append_terminal(store.as_ref(), &source);
    }
    Fixture {
        _root: root,
        store,
        source,
    }
}

fn branch_request(key: &str, created_at_ms: u64) -> PrepareRunBranch {
    PrepareRunBranch {
        parent_run_id: "source-run".into(),
        idempotency_key: key.into(),
        created_at_ms,
    }
}

fn equivalent_terminal_source(fixture: &Fixture, service: &RunService) -> BeginRun {
    let mut source = fixture.source.clone();
    source.run_id = "source-run-two".into();
    source.idempotency_key = "source-key-two".into();
    source.created_at_ms = 20;
    service.begin_run(&source).expect("begin equivalent source");
    append_terminal(fixture.store.as_ref(), &source);
    source
}

fn execution() -> RunExecution {
    RunExecution {
        provider: RunProvider::DeterministicRead {
            path: "README.md".into(),
        },
        system_prompt: "read only".into(),
        allowed_read_paths: vec!["README.md".into()],
        limits: RunLimits {
            max_turns: 4,
            max_tool_calls: 4,
            max_tool_output_bytes: 1_024,
            max_model_output_bytes: 1_024,
            max_model_events: 128,
            max_output_tokens_per_turn: 256,
        },
    }
}

fn append_terminal(store: &impl RunStore, run: &BeginRun) {
    let answer = "terminal answer";
    let kinds = [
        RuntimeEventKind::RunStarted {
            prompt: "branch this run".into(),
        },
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: "branch this run".into(),
            },
        },
        RuntimeEventKind::TurnStarted { turn: 1 },
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant {
                text: answer.into(),
                tool_calls: Vec::new(),
            },
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: answer.into(),
            },
        },
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        store
            .append_event(&RuntimeEvent {
                v: PROTOCOL_VERSION,
                session_id: run.conversation_id.clone(),
                run_id: run.run_id.clone(),
                seq: u64::try_from(index + 1).expect("event sequence"),
                emitted_at_ms: u64::try_from(index + 1).expect("event time"),
                kind,
            })
            .expect("append source event");
    }
}
