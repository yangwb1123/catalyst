use super::{
    CANONICALIZATION, TRANSITION_VOCABULARY_SHA256, TransitionEdge, TransitionState,
    TransitionStateVocabulary, VOCABULARY_API_VERSION, VOCABULARY_KIND,
};

use TransitionState::{
    Assessed, Authorized, Baselined, Blocked, ChangesRequested, Closed, DesignDrafted, Designed,
    Draft, Implementing, Learning, NeedsEvidence, NeedsInfo, Observing, Planned, Quarantined,
    Reflecting, Rejected, ReleaseReady, Releasing, Reviewing, Superseded, Verifying,
};

pub(super) const STATES: [TransitionState; 23] = [
    Draft,
    NeedsEvidence,
    Baselined,
    DesignDrafted,
    Assessed,
    Designed,
    Planned,
    Authorized,
    Implementing,
    Verifying,
    Reviewing,
    ChangesRequested,
    ReleaseReady,
    Releasing,
    Observing,
    Reflecting,
    Learning,
    Closed,
    NeedsInfo,
    Blocked,
    Quarantined,
    Rejected,
    Superseded,
];
pub(super) const TERMINAL_STATES: [TransitionState; 3] = [Closed, Rejected, Superseded];
pub(super) const REWORK_TARGETS: [TransitionState; 6] = [
    DesignDrafted,
    Assessed,
    Designed,
    Planned,
    Implementing,
    Verifying,
];

/// Returns the exact frozen authored Transition state vocabulary.
#[must_use]
pub fn transition_vocabulary() -> TransitionStateVocabulary {
    let mut value = unsealed_vocabulary();
    value.vocabulary_sha256 = TRANSITION_VOCABULARY_SHA256.into();
    value
}

pub(super) fn unsealed_vocabulary() -> TransitionStateVocabulary {
    TransitionStateVocabulary {
        api_version: VOCABULARY_API_VERSION.into(),
        canonicalization: CANONICALIZATION.into(),
        edges: authored_edges(),
        kind: VOCABULARY_KIND.into(),
        rework_targets: REWORK_TARGETS.to_vec(),
        states: STATES.to_vec(),
        terminal_states: TERMINAL_STATES.to_vec(),
        vocabulary_sha256: String::new(),
    }
}

fn edge(from_state: TransitionState, allowed: &[TransitionState]) -> TransitionEdge {
    TransitionEdge {
        allowed_to_states: allowed.to_vec(),
        from_state,
    }
}

pub(super) fn authored_edges() -> Vec<TransitionEdge> {
    let mut edges = early_edges();
    edges.extend(execution_edges());
    edges.extend(delivery_edges());
    edges.extend(closing_edges());
    edges
}

fn early_edges() -> Vec<TransitionEdge> {
    vec![
        edge(Draft, &[NeedsEvidence, NeedsInfo, Rejected, Superseded]),
        edge(
            NeedsEvidence,
            &[Baselined, NeedsInfo, Blocked, Rejected, Superseded],
        ),
        edge(
            Baselined,
            &[DesignDrafted, NeedsInfo, Blocked, Rejected, Superseded],
        ),
        edge(
            DesignDrafted,
            &[Assessed, NeedsInfo, Blocked, Rejected, Superseded],
        ),
        edge(
            Assessed,
            &[
                DesignDrafted,
                Designed,
                NeedsInfo,
                Blocked,
                Rejected,
                Superseded,
            ],
        ),
        edge(
            Designed,
            &[Planned, NeedsInfo, Blocked, Rejected, Superseded],
        ),
        edge(
            Planned,
            &[
                Designed, Authorized, NeedsInfo, Blocked, Rejected, Superseded,
            ],
        ),
    ]
}

fn execution_edges() -> Vec<TransitionEdge> {
    vec![
        edge(
            Authorized,
            &[Implementing, Blocked, Quarantined, Superseded],
        ),
        edge(
            Implementing,
            &[
                Verifying,
                ChangesRequested,
                Blocked,
                Quarantined,
                Superseded,
            ],
        ),
        edge(
            Verifying,
            &[
                Reviewing,
                ChangesRequested,
                Blocked,
                Quarantined,
                Superseded,
            ],
        ),
        edge(
            Reviewing,
            &[
                ReleaseReady,
                ChangesRequested,
                Blocked,
                Rejected,
                Quarantined,
                Superseded,
            ],
        ),
    ]
}

fn delivery_edges() -> Vec<TransitionEdge> {
    vec![
        edge(
            ChangesRequested,
            &[
                DesignDrafted,
                Assessed,
                Designed,
                Planned,
                Implementing,
                Verifying,
                Blocked,
                Rejected,
                Superseded,
            ],
        ),
        edge(ReleaseReady, &[Releasing, Blocked, Quarantined, Superseded]),
        edge(Releasing, &[Observing, Blocked, Quarantined, Superseded]),
    ]
}

fn closing_edges() -> Vec<TransitionEdge> {
    vec![
        edge(
            Observing,
            &[
                Reflecting,
                ChangesRequested,
                Blocked,
                Quarantined,
                Superseded,
            ],
        ),
        edge(
            Reflecting,
            &[Learning, ChangesRequested, Blocked, Superseded],
        ),
        edge(Learning, &[Closed, Blocked, Superseded]),
        edge(NeedsInfo, &[Blocked, Rejected, Superseded]),
        edge(Blocked, &[Rejected, Superseded]),
        edge(Quarantined, &[Blocked, Verifying, Rejected, Superseded]),
    ]
}

pub(super) fn statically_listed(from: TransitionState, to: TransitionState) -> bool {
    authored_edges()
        .into_iter()
        .find(|edge| edge.from_state == from)
        .is_some_and(|edge| edge.allowed_to_states.contains(&to))
}
