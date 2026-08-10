use super::super::{
    API_VERSION, ClaimObjectType, ClaimObjectValue, ClaimState, ClaimType, GovernanceContractError,
    KnowledgeClaim, invalid,
};
use super::common::{
    disjoint_sorted, is_identifier, nonempty_text, sorted_unique_identifiers, validate_interval,
};

pub(super) fn validate(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    if record.api_version != API_VERSION {
        return Err(invalid("claim api_version is invalid"));
    }
    validate_state(record)?;
    validate_spec(record)?;
    validate_status(record)
}

fn validate_state(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    let claim_type = record.spec.claim_type;
    let state = record.status.state;
    if !full_state_allowed(claim_type, state) {
        return Err(invalid(
            "claim type and state are not a valid lifecycle pair",
        ));
    }
    if !shadow_state_allowed(claim_type, state) {
        return Err(invalid(
            "claim state requires authority unavailable in the shadow contract",
        ));
    }
    Ok(())
}

fn validate_spec(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    if !is_identifier(&spec.owner.principal_id)
        || !is_identifier(&spec.predicate)
        || !is_identifier(&spec.subject)
        || !nonempty_text(&spec.reasoning, 4096)
        || !sorted_unique_identifiers(&spec.supporting_evidence_record_ids)
        || !sorted_unique_identifiers(&spec.contradicting_evidence_record_ids)
        || !sorted_unique_identifiers(&spec.derived_from_claim_record_ids)
        || !disjoint_sorted(
            &spec.supporting_evidence_record_ids,
            &spec.contradicting_evidence_record_ids,
        )
    {
        return Err(invalid("claim spec fields or reference sets are invalid"));
    }
    validate_object(record)?;
    validate_confidence(record)?;
    validate_plan(record)?;
    validate_queue(record)?;
    validate_support_shape(record)?;
    if spec.decision_authority.is_some() {
        return Err(invalid(
            "decision authority is unavailable in the shadow contract",
        ));
    }
    Ok(())
}

fn validate_object(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    let valid = match (&spec.object_type, &spec.object_value) {
        (ClaimObjectType::ArtifactRef, ClaimObjectValue::String(value)) => is_identifier(value),
        (ClaimObjectType::Boolean, ClaimObjectValue::Boolean(_))
        | (ClaimObjectType::Integer, ClaimObjectValue::Integer(_))
        | (ClaimObjectType::Null, ClaimObjectValue::Null) => true,
        (ClaimObjectType::String, ClaimObjectValue::String(value)) => value.len() <= 16_384,
        _ => false,
    };
    valid
        .then_some(())
        .ok_or_else(|| invalid("claim object_type does not match object_value"))
}

fn validate_confidence(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    let required = matches!(
        spec.claim_type,
        ClaimType::Assumption | ClaimType::Hypothesis | ClaimType::Inference
    );
    let valid = match (required, spec.confidence_micros) {
        (true, Some(value)) => (0..=1_000_000).contains(&value),
        (false, None) => true,
        _ => false,
    };
    valid
        .then_some(())
        .ok_or_else(|| invalid("claim confidence_micros presence or range is invalid"))
}

fn validate_plan(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    let required = matches!(
        spec.claim_type,
        ClaimType::Assumption | ClaimType::Hypothesis
    );
    match (required, &spec.validation_plan) {
        (true, Some(plan)) => {
            let evidence_types = &plan.required_evidence_types;
            let sorted = evidence_types.windows(2).all(|pair| pair[0] < pair[1]);
            let valid = plan.due_at_unix_ms > record.metadata.created_at_unix_ms
                && is_identifier(&plan.owner_id)
                && nonempty_text(&plan.method, 4096)
                && nonempty_text(&plan.impact_if_false, 4096)
                && !evidence_types.is_empty()
                && sorted;
            valid
                .then_some(())
                .ok_or_else(|| invalid("claim validation plan is invalid"))
        }
        (false, None) => Ok(()),
        _ => Err(invalid("claim validation plan presence is invalid")),
    }
}

fn validate_queue(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    let valid = match (spec.claim_type, spec.queue_ref.as_deref()) {
        (ClaimType::Unknown, Some(queue)) => is_identifier(queue),
        (ClaimType::Unknown, None) | (_, Some(_)) => false,
        (_, None) => true,
    };
    valid
        .then_some(())
        .ok_or_else(|| invalid("claim queue_ref presence is invalid"))
}

fn validate_support_shape(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    let has_evidence = !spec.supporting_evidence_record_ids.is_empty();
    let has_derivation = !spec.derived_from_claim_record_ids.is_empty();
    let valid = match spec.claim_type {
        ClaimType::Fact | ClaimType::Constraint | ClaimType::Lesson => has_evidence,
        ClaimType::Inference => has_evidence || has_derivation,
        _ => true,
    };
    valid
        .then_some(())
        .ok_or_else(|| invalid("claim type lacks required supporting records"))
}

fn validate_status(record: &KnowledgeClaim) -> Result<(), GovernanceContractError> {
    let status = &record.status;
    validate_interval(status.valid_from_unix_ms, status.valid_until_unix_ms)?;
    if status.valid_from_unix_ms < record.metadata.created_at_unix_ms
        || status
            .valid_until_unix_ms
            .is_some_and(|until| until <= status.valid_from_unix_ms)
        || record
            .spec
            .review_by_unix_ms
            .is_some_and(|review| review < record.metadata.created_at_unix_ms)
        || !sorted_unique_identifiers(&status.reason_codes)
    {
        Err(invalid("claim status or review time is invalid"))
    } else {
        Ok(())
    }
}

fn full_state_allowed(claim_type: ClaimType, state: ClaimState) -> bool {
    match claim_type {
        ClaimType::Fact => FACT_STATES.contains(&state),
        ClaimType::Constraint => CONSTRAINT_STATES.contains(&state),
        ClaimType::Decision => DECISION_STATES.contains(&state),
        ClaimType::Inference => INFERENCE_STATES.contains(&state),
        ClaimType::Assumption | ClaimType::Hypothesis => VALIDATION_STATES.contains(&state),
        ClaimType::Lesson => LESSON_STATES.contains(&state),
        ClaimType::Proposal => PROPOSAL_STATES.contains(&state),
        ClaimType::Unknown => UNKNOWN_STATES.contains(&state),
    }
}

const FACT_STATES: &[ClaimState] = &[
    ClaimState::Candidate,
    ClaimState::Confirmed,
    ClaimState::Contested,
    ClaimState::Stale,
    ClaimState::Retracted,
    ClaimState::Superseded,
];
const CONSTRAINT_STATES: &[ClaimState] = &[
    ClaimState::Candidate,
    ClaimState::Active,
    ClaimState::Waived,
    ClaimState::Expired,
    ClaimState::Superseded,
];
const DECISION_STATES: &[ClaimState] = &[
    ClaimState::Proposed,
    ClaimState::Accepted,
    ClaimState::Rejected,
    ClaimState::Deprecated,
    ClaimState::Superseded,
];
const INFERENCE_STATES: &[ClaimState] = &[
    ClaimState::Candidate,
    ClaimState::Supported,
    ClaimState::Contested,
    ClaimState::Invalidated,
    ClaimState::Expired,
];
const VALIDATION_STATES: &[ClaimState] = &[
    ClaimState::Open,
    ClaimState::Testing,
    ClaimState::Validated,
    ClaimState::Invalidated,
    ClaimState::Expired,
];
const LESSON_STATES: &[ClaimState] = &[
    ClaimState::Candidate,
    ClaimState::Observed,
    ClaimState::Repeated,
    ClaimState::Retired,
    ClaimState::Promoted,
];
const PROPOSAL_STATES: &[ClaimState] = &[
    ClaimState::Draft,
    ClaimState::Submitted,
    ClaimState::Adopted,
    ClaimState::Rejected,
    ClaimState::Superseded,
];
const UNKNOWN_STATES: &[ClaimState] = &[
    ClaimState::Open,
    ClaimState::Investigating,
    ClaimState::Resolved,
    ClaimState::AcceptedRisk,
];

fn shadow_state_allowed(claim_type: ClaimType, state: ClaimState) -> bool {
    match claim_type {
        ClaimType::Fact => matches!(state, ClaimState::Candidate | ClaimState::Contested),
        ClaimType::Constraint | ClaimType::Inference | ClaimType::Lesson => {
            state == ClaimState::Candidate
        }
        ClaimType::Decision => state == ClaimState::Proposed,
        ClaimType::Assumption | ClaimType::Hypothesis => {
            matches!(state, ClaimState::Open | ClaimState::Testing)
        }
        ClaimType::Proposal => matches!(state, ClaimState::Draft | ClaimState::Submitted),
        ClaimType::Unknown => matches!(state, ClaimState::Open | ClaimState::Investigating),
    }
}
