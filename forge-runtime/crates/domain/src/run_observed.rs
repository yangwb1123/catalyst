//! Content-free evidence for an already owner-scoped Run summary.
//!
//! This module is a pure value boundary. It does not serialize a production
//! wire payload, read storage, obtain time, or grant any authority.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use sha2::{Digest, Sha256};

use crate::{ConversationOwner, OwnedRunStatus, OwnedRunSummary};

pub const FORGE_RUN_OBSERVED_V1: &str = "forge.run.observed.v1";
pub const MAX_RUN_OBSERVED_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_IDENTIFIER_BYTES: usize = 85;

/// Inputs already obtained from an owner-scoped, metadata-only Run read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunObservedInput {
    pub owner: ConversationOwner,
    pub conversation_id: String,
    pub run: OwnedRunSummary,
}

/// Capabilities deliberately absent from a Run observation.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunObservedAuthority {
    pub identity_verified: bool,
    pub owner_authorized: bool,
    pub run_authoritative: bool,
    pub persistence_attested: bool,
    pub content_provenance_verified: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
}

/// Deterministic, bounded, payload-free evidence for one existing Run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunObserved {
    pub api_version: &'static str,
    pub owner_ref: String,
    pub conversation_id: String,
    pub run_id: String,
    pub prompt_id: String,
    pub created_at_ms: u64,
    pub latest_sequence: u64,
    pub status: &'static str,
    pub metadata_observed: bool,
    pub content_included: bool,
    pub authority: RunObservedAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunObservedError {
    InvalidOwner,
    InvalidMetadata,
    InvalidObservation,
}

impl fmt::Display for RunObservedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Run observation is invalid: {self:?}")
    }
}

impl std::error::Error for RunObservedError {}

impl RunObserved {
    /// Validates a caller-supplied wire projection without adding authority.
    ///
    /// This is deliberately separate from `observe_run`: local CLI/TUI
    /// consumers may inspect a fixture without reconstructing the owner and
    /// source Run summary.
    pub fn validate(&self) -> Result<(), RunObservedError> {
        if self.api_version != FORGE_RUN_OBSERVED_V1
            || !valid_digest(&self.owner_ref)
            || !valid_identifier(&self.conversation_id)
            || !valid_identifier(&self.run_id)
            || !valid_identifier(&self.prompt_id)
            || self.created_at_ms > MAX_RUN_OBSERVED_SAFE_INTEGER
            || self.latest_sequence == 0
            || self.latest_sequence > MAX_RUN_OBSERVED_SAFE_INTEGER
            || !matches!(
                self.status,
                "nonterminal" | "completed" | "cancelled" | "limit_exceeded" | "failed"
            )
            || !self.metadata_observed
            || self.content_included
            || self.authority != RunObservedAuthority::default()
        {
            return Err(RunObservedError::InvalidObservation);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunObservedWire {
    api_version: String,
    owner_ref: String,
    conversation_id: String,
    run_id: String,
    prompt_id: String,
    created_at_ms: u64,
    latest_sequence: u64,
    status: String,
    metadata_observed: bool,
    content_included: bool,
    authority: RunObservedAuthority,
}

impl<'de> Deserialize<'de> for RunObserved {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RunObservedWire::deserialize(deserializer)?;
        let status = match wire.status.as_str() {
            "nonterminal" => "nonterminal",
            "completed" => "completed",
            "cancelled" => "cancelled",
            "limit_exceeded" => "limit_exceeded",
            "failed" => "failed",
            _ => return Err(D::Error::custom(RunObservedError::InvalidObservation)),
        };
        if wire.api_version != FORGE_RUN_OBSERVED_V1 {
            return Err(D::Error::custom(RunObservedError::InvalidObservation));
        }
        let observation = Self {
            api_version: FORGE_RUN_OBSERVED_V1,
            owner_ref: wire.owner_ref,
            conversation_id: wire.conversation_id,
            run_id: wire.run_id,
            prompt_id: wire.prompt_id,
            created_at_ms: wire.created_at_ms,
            latest_sequence: wire.latest_sequence,
            status,
            metadata_observed: wire.metadata_observed,
            content_included: wire.content_included,
            authority: wire.authority,
        };
        observation.validate().map_err(D::Error::custom)?;
        Ok(observation)
    }
}

/// Projects owner and scalar Run metadata into content-free evidence.
///
/// # Errors
///
/// Returns an error for malformed owner or bounded Run metadata. It never
/// reads a store, verifies authority, or changes external state.
pub fn observe_run(input: RunObservedInput) -> Result<RunObserved, RunObservedError> {
    if !valid_owner(&input.owner) {
        return Err(RunObservedError::InvalidOwner);
    }
    if !valid_identifier(&input.conversation_id)
        || !valid_identifier(&input.run.run_id)
        || !valid_identifier(&input.run.prompt_id)
        || input.run.created_at_ms > MAX_RUN_OBSERVED_SAFE_INTEGER
        || input.run.latest_sequence == 0
        || input.run.latest_sequence > MAX_RUN_OBSERVED_SAFE_INTEGER
    {
        return Err(RunObservedError::InvalidMetadata);
    }

    Ok(RunObserved {
        api_version: FORGE_RUN_OBSERVED_V1,
        owner_ref: owner_reference(&input.owner),
        conversation_id: input.conversation_id,
        run_id: input.run.run_id,
        prompt_id: input.run.prompt_id,
        created_at_ms: input.run.created_at_ms,
        latest_sequence: input.run.latest_sequence,
        status: status_name(input.run.status),
        metadata_observed: true,
        content_included: false,
        authority: RunObservedAuthority::default(),
    })
}

fn valid_owner(owner: &ConversationOwner) -> bool {
    valid_owner_component(&owner.issuer, 2_048)
        && valid_owner_component(&owner.subject, 255)
        && valid_identifier(&owner.tenant_id)
}

fn valid_owner_component(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.chars().all(|character| {
            !character.is_control()
                && !character.is_whitespace()
                && !matches!(character, ':' | '/' | '\\')
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn status_name(status: OwnedRunStatus) -> &'static str {
    match status {
        OwnedRunStatus::Nonterminal => "nonterminal",
        OwnedRunStatus::Completed => "completed",
        OwnedRunStatus::Cancelled => "cancelled",
        OwnedRunStatus::LimitExceeded => "limit_exceeded",
        OwnedRunStatus::Failed => "failed",
    }
}

pub(crate) fn owner_reference(owner: &ConversationOwner) -> String {
    let mut digest = Sha256::new();
    digest.update(b"forge.run.observed.v1/owner\0");
    for value in [&owner.issuer, &owner.subject, &owner.tenant_id] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests;
