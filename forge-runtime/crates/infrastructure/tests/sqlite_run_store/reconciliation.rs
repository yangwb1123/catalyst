use super::*;

#[test]
fn delayed_writeback_remains_causally_attached_to_its_user_prompt() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin P Run");
    append_event(&fixture, 1, run_started());
    commit_completed(&fixture);
    fixture
        .store
        .append_prompt(&fixture.conversation_id, "user", "Q", "prompt-q")
        .expect("append Q before recovering P");
    fixture
        .store
        .reconcile_completed_assistant("run-1")
        .expect("recover P assistant after Q");
    let current = fixture
        .store
        .append_prompt(&fixture.conversation_id, "user", "R", "prompt-r")
        .expect("append R");

    let mut history = fixture
        .store
        .list_prompts_before(&fixture.conversation_id, &current.id, 10)
        .expect("causal history before R");
    history.reverse();
    let messages: Vec<_> = history
        .iter()
        .map(|prompt| (prompt.role.as_str(), prompt.content.as_str()))
        .collect();

    assert_eq!(
        messages,
        [
            ("user", "inspect README"),
            ("assistant", "done"),
            ("user", "Q")
        ]
    );
}

#[test]
fn non_completed_terminal_outcome_cannot_create_an_assistant_prompt() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    append_event(&fixture, 1, run_started());
    append_event(&fixture, 2, current_user());
    append_event(
        &fixture,
        3,
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Cancelled,
        },
    );

    assert!(
        fixture
            .store
            .reconcile_completed_assistant("run-1")
            .is_err()
    );
    assert_eq!(
        fixture
            .store
            .list_prompts(Some(&fixture.conversation_id), 10)
            .expect("prompt list")
            .len(),
        1
    );
}

#[test]
fn durable_sink_commits_before_downstream_and_stops_on_store_failure() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let event = fixture.event(1, run_started());
    let observer_store = fixture.store.clone();
    let mut observer = ObservingFailSink {
        store: observer_store,
        observed_durable_event: false,
    };
    {
        let mut sink = DurableFirstEventSink::new(&fixture.store, &mut observer);
        sink.emit(&event).expect_err("downstream failure surfaces");
    }
    assert!(observer.observed_durable_event);

    let mut counter = CountingSink::default();
    let missing = RuntimeEvent {
        run_id: "missing".into(),
        ..event
    };
    {
        let mut sink = DurableFirstEventSink::new(&fixture.store, &mut counter);
        sink.emit(&missing)
            .expect_err("store failure surfaces first");
    }
    assert_eq!(counter.events, 0);
}

#[test]
fn concurrent_divergent_appends_cannot_fork_one_sequence() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for emitted_at_ms in [101, 102] {
        let store = fixture.store.clone();
        let barrier = Arc::clone(&barrier);
        let mut event = fixture.event(1, run_started());
        event.emitted_at_ms = emitted_at_ms;
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store.append_event(&event)
        }));
    }
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        fixture
            .store
            .inspect_run("run-1")
            .expect("inspect")
            .events
            .len(),
        1
    );
}
