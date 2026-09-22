use std::collections::HashSet;
use std::fmt;

use super::super::MAX_DEVICE_IDENTIFIER_BYTES;
use super::{
    SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION, SessionPlacementAuthority,
    SessionPlacementObservation,
};

pub const RUN_INTENT_OBSERVATION_SCHEMA_VERSION: &str = "forge.run-intent-observation/v1";
pub const MAX_RUN_INTENT_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// The immutable, payload-free receipt returned after a prompt intent was
/// accepted. It is a declaration supplied to this value-only observer; it
/// does not create a Run or re-read Hub state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunIntentPromptReceipt {
    pub prompt_id: String,
    pub conversation_id: String,
    pub role: String,
    pub accepted_at_ms: u64,
    pub intent_id: String,
    pub initial_event_id: String,
    pub initial_event_sequence: u64,
    pub initial_event_type: String,
    pub replayed: bool,
}

/// The sanitized Run summary observed for the prompt receipt. This is an
/// existing reference, never a command to create or start a Run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunIntentRunReference {
    pub run_id: String,
    pub conversation_id: String,
    pub prompt_id: String,
    pub created_at_ms: u64,
    pub latest_sequence: u64,
    pub status: String,
}

/// Inputs to the pure Run intent preview. The placement observation is
/// already computed by the offline session-placement contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunIntentObservationRequest {
    pub owner: super::SessionPlacementOwner,
    pub conversation_id: String,
    pub prompt: RunIntentPromptReceipt,
    pub run: RunIntentRunReference,
    pub placement: SessionPlacementObservation,
}

/// A deterministic summary tying an accepted prompt receipt and observed Run
/// reference to one existing offline placement observation. No target is
/// selected and no authority is granted.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunIntentObservation {
    pub schema_version: &'static str,
    pub evaluation_mode: &'static str,
    pub owner: super::SessionPlacementOwner,
    pub conversation_id: String,
    pub prompt_id: String,
    pub intent_id: String,
    pub run_id: String,
    pub prompt_accepted: bool,
    pub run_reference_observed: bool,
    pub prompt_run_binding_valid: bool,
    pub placement_observation_bound: bool,
    pub preview_only: bool,
    pub intent_replayed: bool,
    pub run_status: String,
    pub run_latest_sequence: u64,
    pub prompt_accepted_at_ms: u64,
    pub placement_evaluated_at_ms: u64,
    pub placement_decision_count: usize,
    pub eligible_instance_count: usize,
    pub owner_declaration_unverified: bool,
    pub device_attributes_unverified: bool,
    pub selected_device_id: Option<String>,
    pub selected_instance_id: Option<String>,
    pub authority: SessionPlacementAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunIntentObservationError {
    InvalidReceipt,
    InvalidBinding,
    InvalidPlacement,
}

impl fmt::Display for RunIntentObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "run intent observation is invalid: {self:?}")
    }
}

impl std::error::Error for RunIntentObservationError {}

/// Binds the caller-declared owner, accepted prompt receipt, existing Run
/// summary, and existing session placement observation. This function has no
/// clock, storage, network, selection, reservation, execution, or dispatch.
///
/// # Errors
///
/// Returns an error when any owner, receipt, Run, placement, or authority
/// binding is malformed or claims a capability outside this preview contract.
pub fn observe_run_intent(
    input: RunIntentObservationRequest,
) -> Result<RunIntentObservation, RunIntentObservationError> {
    if !valid_owner(&input.owner)
        || !valid_identifier(&input.conversation_id)
        || input.placement.owner != input.owner
        || input.placement.conversation_id != input.conversation_id
    {
        return Err(RunIntentObservationError::InvalidBinding);
    }
    validate_prompt(&input.prompt, &input.conversation_id)?;
    validate_run(&input.run, &input.prompt, &input.conversation_id)?;
    validate_placement(
        &input.placement,
        &input.owner,
        &input.conversation_id,
        &input.run.run_id,
    )?;

    let eligible_instance_count = input
        .placement
        .decisions
        .iter()
        .filter(|decision| decision.matches_requirements)
        .count();
    Ok(RunIntentObservation {
        schema_version: RUN_INTENT_OBSERVATION_SCHEMA_VERSION,
        evaluation_mode: input.placement.evaluation_mode,
        owner: input.owner,
        conversation_id: input.conversation_id,
        prompt_id: input.prompt.prompt_id,
        intent_id: input.prompt.intent_id,
        run_id: input.run.run_id,
        prompt_accepted: true,
        run_reference_observed: true,
        prompt_run_binding_valid: true,
        placement_observation_bound: true,
        preview_only: true,
        intent_replayed: input.prompt.replayed,
        run_status: input.run.status,
        run_latest_sequence: input.run.latest_sequence,
        prompt_accepted_at_ms: input.prompt.accepted_at_ms,
        placement_evaluated_at_ms: input.placement.evaluated_at_ms,
        placement_decision_count: input.placement.decisions.len(),
        eligible_instance_count,
        owner_declaration_unverified: true,
        device_attributes_unverified: true,
        selected_device_id: None,
        selected_instance_id: None,
        authority: SessionPlacementAuthority::default(),
    })
}

fn validate_prompt(
    prompt: &RunIntentPromptReceipt,
    conversation_id: &str,
) -> Result<(), RunIntentObservationError> {
    if !valid_identifier(&prompt.prompt_id)
        || prompt.conversation_id != conversation_id
        || prompt.role != "user"
        || prompt.accepted_at_ms > MAX_RUN_INTENT_SAFE_INTEGER
        || !valid_identifier(&prompt.intent_id)
        || !valid_identifier(&prompt.initial_event_id)
        || prompt.initial_event_sequence != 1
        || prompt.initial_event_type != "submitted"
    {
        return Err(RunIntentObservationError::InvalidReceipt);
    }
    Ok(())
}

fn validate_run(
    run: &RunIntentRunReference,
    prompt: &RunIntentPromptReceipt,
    conversation_id: &str,
) -> Result<(), RunIntentObservationError> {
    if !valid_identifier(&run.run_id)
        || run.conversation_id != conversation_id
        || run.prompt_id != prompt.prompt_id
        || run.created_at_ms < prompt.accepted_at_ms
        || run.created_at_ms > MAX_RUN_INTENT_SAFE_INTEGER
        || run.latest_sequence == 0
        || run.latest_sequence > MAX_RUN_INTENT_SAFE_INTEGER
        || !valid_run_status(&run.status)
    {
        return Err(RunIntentObservationError::InvalidBinding);
    }
    Ok(())
}

fn validate_placement(
    placement: &SessionPlacementObservation,
    owner: &super::SessionPlacementOwner,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RunIntentObservationError> {
    if placement.schema_version != SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION
        || placement.evaluation_mode != "offline_static_only"
        || placement.owner != *owner
        || placement.conversation_id != conversation_id
        || placement.run_id != run_id
        || placement.evaluated_at_ms == 0
        || placement.evaluated_at_ms > MAX_RUN_INTENT_SAFE_INTEGER
        || !placement.owner_declaration_unverified
        || !placement.device_attributes_unverified
        || placement.selected_device_id.is_some()
        || placement.selected_instance_id.is_some()
        || placement.authority != SessionPlacementAuthority::default()
    {
        return Err(RunIntentObservationError::InvalidPlacement);
    }
    let mut devices = HashSet::with_capacity(placement.decisions.len());
    let mut instances = HashSet::with_capacity(placement.decisions.len());
    if placement.decisions.iter().any(|decision| {
        !valid_identifier(&decision.device_id)
            || !valid_identifier(&decision.instance_id)
            || !devices.insert(decision.device_id.clone())
            || !instances.insert(decision.instance_id.clone())
    }) {
        return Err(RunIntentObservationError::InvalidPlacement);
    }
    Ok(())
}

fn valid_owner(owner: &super::SessionPlacementOwner) -> bool {
    valid_owner_part(&owner.issuer)
        && valid_owner_part(&owner.subject)
        && !owner.tenant_id.as_str().is_empty()
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_DEVICE_IDENTIFIER_BYTES
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-')
        })
}

fn valid_run_status(status: &str) -> bool {
    matches!(
        status,
        "nonterminal" | "completed" | "cancelled" | "limit_exceeded" | "failed"
    )
}
