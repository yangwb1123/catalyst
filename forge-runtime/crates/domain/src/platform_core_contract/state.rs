use std::collections::HashSet;
use std::hash::Hash;

use super::{
    ActionState, AttemptState, PlatformCoreContractError, RejectionCode, WorkItemState, reject,
};

/// Validates one declared `WorkItem` edge without advancing product state.
///
/// # Errors
/// Returns `pc_transition_invalid` when the edge is not in v1.
pub fn validate_work_item_transition(
    from: &WorkItemState,
    to: &WorkItemState,
) -> Result<(), PlatformCoreContractError> {
    if matches!(from, WorkItemState::Unknown(_)) || matches!(to, WorkItemState::Unknown(_)) {
        return Err(unsupported_state("WorkItem"));
    }
    validate_edge(from, to, &work_item_edges())
}

/// Validates one declared `Attempt` edge without advancing runtime state.
///
/// # Errors
/// Returns `pc_transition_invalid` when the edge is not in v1.
pub fn validate_attempt_transition(
    from: &AttemptState,
    to: &AttemptState,
) -> Result<(), PlatformCoreContractError> {
    if matches!(from, AttemptState::Unknown(_)) || matches!(to, AttemptState::Unknown(_)) {
        return Err(unsupported_state("Attempt"));
    }
    validate_edge(from, to, &attempt_edges())
}

/// Validates one declared `Action` edge without advancing runtime state.
///
/// # Errors
/// Returns `pc_transition_invalid` when the edge is not in v1.
pub fn validate_action_transition(
    from: &ActionState,
    to: &ActionState,
) -> Result<(), PlatformCoreContractError> {
    if matches!(from, ActionState::Unknown(_)) || matches!(to, ActionState::Unknown(_)) {
        return Err(unsupported_state("Action"));
    }
    validate_edge(from, to, &action_edges())
}

fn unsupported_state(machine: &str) -> PlatformCoreContractError {
    reject(
        RejectionCode::StateInvalid,
        format!("{machine} transition contains an unsupported state"),
    )
}

fn validate_edge<T>(
    from: &T,
    to: &T,
    edges: &HashSet<(T, T)>,
) -> Result<(), PlatformCoreContractError>
where
    T: Clone + Eq + Hash + std::fmt::Debug,
{
    if edges
        .iter()
        .any(|(source, target)| source == from && target == to)
    {
        Ok(())
    } else {
        Err(reject(
            RejectionCode::TransitionInvalid,
            format!("transition {from:?} -> {to:?} is not declared"),
        ))
    }
}

fn work_item_edges() -> HashSet<(WorkItemState, WorkItemState)> {
    use WorkItemState as S;
    edge_set(&[
        (S::Draft, &[S::Planned, S::Cancelled]),
        (S::Planned, &[S::AwaitingApproval, S::Ready, S::Cancelled]),
        (S::AwaitingApproval, &[S::Ready, S::Blocked, S::Cancelled]),
        (S::Ready, &[S::Dispatched, S::Blocked, S::Cancelled]),
        (
            S::Dispatched,
            &[
                S::Running,
                S::Blocked,
                S::Failed,
                S::Uncertain,
                S::Cancelled,
            ],
        ),
        (
            S::Running,
            &[
                S::Verifying,
                S::Blocked,
                S::Failed,
                S::Uncertain,
                S::Cancelled,
            ],
        ),
        (
            S::Verifying,
            &[
                S::Completed,
                S::Ready,
                S::Blocked,
                S::Failed,
                S::Uncertain,
                S::Cancelled,
            ],
        ),
        (S::Blocked, &[S::Ready, S::Failed, S::Cancelled]),
        (S::Failed, &[S::Ready, S::Cancelled]),
        (S::Uncertain, &[S::Ready, S::Failed, S::Cancelled]),
    ])
}

fn attempt_edges() -> HashSet<(AttemptState, AttemptState)> {
    use AttemptState as S;
    edge_set(&[
        (S::Requested, &[S::Accepted]),
        (
            S::Accepted,
            &[S::Starting, S::Interrupted, S::Failed, S::Uncertain],
        ),
        (
            S::Starting,
            &[S::Running, S::Interrupted, S::Failed, S::Uncertain],
        ),
        (
            S::Running,
            &[S::Interrupted, S::Completed, S::Failed, S::Uncertain],
        ),
    ])
}

fn action_edges() -> HashSet<(ActionState, ActionState)> {
    use ActionState as S;
    edge_set(&[
        (
            S::Requested,
            &[S::AwaitingApproval, S::Approved, S::Rejected, S::Cancelled],
        ),
        (
            S::AwaitingApproval,
            &[S::Approved, S::Rejected, S::Cancelled],
        ),
        (S::Approved, &[S::Started, S::Cancelled]),
        (
            S::Started,
            &[S::Finished, S::Failed, S::Cancelled, S::Uncertain],
        ),
    ])
}

fn edge_set<T>(rows: &[(T, &[T])]) -> HashSet<(T, T)>
where
    T: Clone + Eq + Hash,
{
    let mut result = HashSet::new();
    for (from, targets) in rows {
        for to in *targets {
            result.insert((from.clone(), to.clone()));
        }
    }
    result
}
