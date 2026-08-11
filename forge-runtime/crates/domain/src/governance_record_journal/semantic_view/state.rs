use crate::governance_contract::{ClaimState, ClaimType};

pub(super) const fn authority_free_shadow_claim_state(
    claim_type: ClaimType,
    state: ClaimState,
) -> bool {
    match claim_type {
        ClaimType::Fact => matches!(state, ClaimState::Candidate | ClaimState::Contested),
        ClaimType::Constraint | ClaimType::Inference | ClaimType::Lesson => {
            matches!(state, ClaimState::Candidate)
        }
        ClaimType::Decision => matches!(state, ClaimState::Proposed),
        ClaimType::Assumption | ClaimType::Hypothesis => {
            matches!(state, ClaimState::Open | ClaimState::Testing)
        }
        ClaimType::Proposal => matches!(state, ClaimState::Draft | ClaimState::Submitted),
        ClaimType::Unknown => matches!(state, ClaimState::Open | ClaimState::Investigating),
    }
}

pub(super) fn authority_free_shadow_declared_state(
    claim_type: ClaimType,
    declared_state: &str,
) -> bool {
    parse_claim_state(declared_state)
        .is_some_and(|state| authority_free_shadow_claim_state(claim_type, state))
}

pub(super) const fn claim_state_name(state: ClaimState) -> &'static str {
    match state {
        ClaimState::Accepted => "accepted",
        ClaimState::AcceptedRisk => "accepted_risk",
        ClaimState::Active => "active",
        ClaimState::Adopted => "adopted",
        ClaimState::Candidate => "candidate",
        ClaimState::Confirmed => "confirmed",
        ClaimState::Contested => "contested",
        ClaimState::Deprecated => "deprecated",
        ClaimState::Draft => "draft",
        ClaimState::Expired => "expired",
        ClaimState::Investigating => "investigating",
        ClaimState::Invalidated => "invalidated",
        ClaimState::Observed => "observed",
        ClaimState::Open => "open",
        ClaimState::Promoted => "promoted",
        ClaimState::Proposed => "proposed",
        ClaimState::Rejected => "rejected",
        ClaimState::Repeated => "repeated",
        ClaimState::Resolved => "resolved",
        ClaimState::Retracted => "retracted",
        ClaimState::Retired => "retired",
        ClaimState::Stale => "stale",
        ClaimState::Submitted => "submitted",
        ClaimState::Superseded => "superseded",
        ClaimState::Supported => "supported",
        ClaimState::Testing => "testing",
        ClaimState::Validated => "validated",
        ClaimState::Waived => "waived",
    }
}

fn parse_claim_state(value: &str) -> Option<ClaimState> {
    Some(match value {
        "accepted" => ClaimState::Accepted,
        "accepted_risk" => ClaimState::AcceptedRisk,
        "active" => ClaimState::Active,
        "adopted" => ClaimState::Adopted,
        "candidate" => ClaimState::Candidate,
        "confirmed" => ClaimState::Confirmed,
        "contested" => ClaimState::Contested,
        "deprecated" => ClaimState::Deprecated,
        "draft" => ClaimState::Draft,
        "expired" => ClaimState::Expired,
        "investigating" => ClaimState::Investigating,
        "invalidated" => ClaimState::Invalidated,
        "observed" => ClaimState::Observed,
        "open" => ClaimState::Open,
        "promoted" => ClaimState::Promoted,
        "proposed" => ClaimState::Proposed,
        "rejected" => ClaimState::Rejected,
        "repeated" => ClaimState::Repeated,
        "resolved" => ClaimState::Resolved,
        "retracted" => ClaimState::Retracted,
        "retired" => ClaimState::Retired,
        "stale" => ClaimState::Stale,
        "submitted" => ClaimState::Submitted,
        "superseded" => ClaimState::Superseded,
        "supported" => ClaimState::Supported,
        "testing" => ClaimState::Testing,
        "validated" => ClaimState::Validated,
        "waived" => ClaimState::Waived,
        _ => return None,
    })
}
