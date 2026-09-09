use forge_runtime_domain::platform_core_contract::{
    AttemptState, PlatformCoreContractError, RejectionCode, validate_attempt_transition,
};
use std::{sync::Arc, thread};

type Lifecycle = forge_runtime_domain::execution::attempt_lifecycle::AttemptLifecycle;
type Request = forge_runtime_domain::execution::attempt_lifecycle::AttemptTransitionRequest;

const REQUESTED: Lifecycle = Lifecycle::requested();

fn lifecycles() -> [Lifecycle; 8] {
    let requested = Lifecycle::requested();
    let accepted = requested.reduce(&Request::Accept).expect("accept");
    let starting = accepted.reduce(&Request::BeginStarting).expect("start");
    let running = starting.reduce(&Request::ObserveRunning).expect("run");
    let interrupted = running
        .reduce(&Request::ObserveInterrupted)
        .expect("interrupt");
    let completed = running
        .reduce(&Request::ObserveCompleted)
        .expect("complete");
    let failed = running.reduce(&Request::ObserveFailed).expect("fail");
    let uncertain = running
        .reduce(&Request::ObserveEffectOutcomeUncertain)
        .expect("explicit effect uncertainty");
    [
        requested,
        accepted,
        starting,
        running,
        interrupted,
        completed,
        failed,
        uncertain,
    ]
}

fn requests() -> [(Request, AttemptState); 7] {
    [
        (Request::Accept, AttemptState::Accepted),
        (Request::BeginStarting, AttemptState::Starting),
        (Request::ObserveRunning, AttemptState::Running),
        (Request::ObserveInterrupted, AttemptState::Interrupted),
        (Request::ObserveCompleted, AttemptState::Completed),
        (Request::ObserveFailed, AttemptState::Failed),
        (
            Request::ObserveEffectOutcomeUncertain,
            AttemptState::Uncertain,
        ),
    ]
}

fn expected_edges() -> [(AttemptState, AttemptState); 13] {
    [
        (AttemptState::Requested, AttemptState::Accepted),
        (AttemptState::Accepted, AttemptState::Starting),
        (AttemptState::Accepted, AttemptState::Interrupted),
        (AttemptState::Accepted, AttemptState::Failed),
        (AttemptState::Accepted, AttemptState::Uncertain),
        (AttemptState::Starting, AttemptState::Running),
        (AttemptState::Starting, AttemptState::Interrupted),
        (AttemptState::Starting, AttemptState::Failed),
        (AttemptState::Starting, AttemptState::Uncertain),
        (AttemptState::Running, AttemptState::Interrupted),
        (AttemptState::Running, AttemptState::Completed),
        (AttemptState::Running, AttemptState::Failed),
        (AttemptState::Running, AttemptState::Uncertain),
    ]
}

fn is_terminal(state: &AttemptState) -> bool {
    matches!(
        state,
        AttemptState::Interrupted
            | AttemptState::Completed
            | AttemptState::Failed
            | AttemptState::Uncertain
    )
}

fn assert_uncertainty(source: &Lifecycle, request: Request, next: &Lifecycle) -> usize {
    if next.state() != &AttemptState::Uncertain {
        assert_ne!(request, Request::ObserveEffectOutcomeUncertain);
        return 0;
    }
    assert_eq!(request, Request::ObserveEffectOutcomeUncertain);
    assert!(matches!(
        source.state(),
        AttemptState::Accepted | AttemptState::Starting | AttemptState::Running
    ));
    1
}

#[test]
fn requested_seed_and_reachable_values_have_stable_clone_equality() {
    assert_eq!(REQUESTED.state(), &AttemptState::Requested);
    assert_eq!(REQUESTED, Lifecycle::requested());
    let states = lifecycles();
    let expected = [
        AttemptState::Requested,
        AttemptState::Accepted,
        AttemptState::Starting,
        AttemptState::Running,
        AttemptState::Interrupted,
        AttemptState::Completed,
        AttemptState::Failed,
        AttemptState::Uncertain,
    ];
    for (index, state) in states.iter().enumerate() {
        assert_eq!(state.state(), &expected[index]);
        assert_eq!(state.clone(), *state);
        for other in &states[index + 1..] {
            assert_ne!(state, other);
        }
    }
}

#[test]
fn complete_matrix_preserves_canonical_edges_terminals_uncertainty_and_source_values() {
    let edges = expected_edges();
    let (mut accepted, mut rejected, mut terminal_rejections, mut uncertain) = (0, 0, 0, 0);
    for source in lifecycles() {
        for (request, target) in requests() {
            let before = source.clone();
            let result = source.reduce(&request);
            let canonical = validate_attempt_transition(source.state(), &target);
            let expected = edges
                .iter()
                .any(|(from, to)| from == source.state() && to == &target);
            assert_eq!(source, before, "source changed for {request:?}");
            assert_eq!(
                result.as_ref().map(|_| ()),
                canonical.as_ref().copied(),
                "canonical parity: {:?} / {request:?}",
                source.state()
            );
            assert_eq!(
                result.is_ok(),
                expected,
                "{:?} / {request:?}",
                source.state()
            );
            match result {
                Ok(next) => {
                    accepted += 1;
                    assert_eq!(next.state(), &target);
                    assert_ne!(next, source);
                    assert!(!is_terminal(source.state()));
                    uncertain += assert_uncertainty(&source, request, &next);
                }
                Err(error) => {
                    rejected += 1;
                    assert_eq!(error.code, RejectionCode::TransitionInvalid);
                    terminal_rejections += usize::from(is_terminal(source.state()));
                }
            }
        }
    }
    assert_eq!(
        (accepted, rejected, terminal_rejections, uncertain),
        (13, 43, 28, 3)
    );
}

fn reductions(states: &[Lifecycle]) -> Vec<Result<Lifecycle, PlatformCoreContractError>> {
    states
        .iter()
        .flat_map(|source| {
            requests()
                .into_iter()
                .map(move |(request, _)| source.reduce(&request))
        })
        .collect()
}

#[test]
fn sixty_four_repeated_reductions_return_equal_values_and_errors() {
    let states = lifecycles();
    let before = states.clone();
    let expected = reductions(&states);
    assert_eq!(expected.len(), 56);
    for _ in 0..64 {
        assert_eq!(reductions(&states), expected);
        assert_eq!(states, before);
    }
}

#[test]
fn eight_parallel_reducers_share_immutable_sources_and_equal_results() {
    let states = Arc::new(lifecycles());
    let before = states.as_ref().clone();
    let expected = reductions(states.as_ref());
    let workers = (0..8)
        .map(|_| {
            let shared = Arc::clone(&states);
            thread::spawn(move || reductions(shared.as_ref()))
        })
        .collect::<Vec<_>>();
    for worker in workers {
        assert_eq!(worker.join().expect("parallel reduction"), expected);
    }
    assert_eq!(states.as_ref(), &before);
}
