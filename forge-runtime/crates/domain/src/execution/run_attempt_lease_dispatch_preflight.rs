//! Strict, content-free Run/Attempt/lease preflight metadata.
//!
//! This value is intentionally a display boundary. It accepts only a
//! caller-supplied observation and never reads Hub state, a clock, or a
//! device registry. In particular, `declarative_preflight_ready` is not a
//! selection or dispatch decision.

use std::{collections::HashSet, fmt};

use serde::{Deserialize, Serialize, de, de::DeserializeSeed};

use crate::ConversationOwner;

pub const RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_SCHEMA_VERSION: &str =
    "forge.run-attempt-lease-dispatch-preflight/v1";
pub const RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_EVALUATION_MODE: &str =
    "pure_run_attempt_lease_dispatch_preflight";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_IDENTIFIER_BYTES: usize = 85;
const MAX_CANDIDATES: u32 = 128;

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunAttemptLeaseDispatchPreflightAuthority {
    pub identity_verified: bool,
    pub run_authoritative: bool,
    pub attempt_persisted: bool,
    pub lease_issued: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunAttemptLeaseDispatchPreflightObservation {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub owner: ConversationOwner,
    pub conversation_id: String,
    pub run_id: String,
    pub run_status: String,
    pub run_state_admissible: bool,
    pub attempt_id: String,
    pub attempt_state: String,
    pub attempt_state_admissible: bool,
    pub command_id: String,
    pub intent_target_id: String,
    pub lease_epoch: u64,
    pub lease_active: bool,
    pub evaluated_at_ms: u64,
    pub candidate_count: u32,
    pub declarative_ready_count: u32,
    pub declarative_preflight_ready: bool,
    pub rejection_reasons: Vec<String>,
    pub selected_target_id: Option<String>,
    pub preview_only: bool,
    pub authority: RunAttemptLeaseDispatchPreflightAuthority,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The wire decoder must retain the published preflight predicates as distinct booleans."
)]
struct RunAttemptLeaseDispatchPreflightWire {
    schema_version: String,
    evaluation_mode: String,
    owner: ConversationOwner,
    conversation_id: String,
    run_id: String,
    run_status: String,
    run_state_admissible: bool,
    attempt_id: String,
    attempt_state: String,
    attempt_state_admissible: bool,
    command_id: String,
    intent_target_id: String,
    lease_epoch: u64,
    lease_active: bool,
    evaluated_at_ms: u64,
    candidate_count: u32,
    declarative_ready_count: u32,
    declarative_preflight_ready: bool,
    rejection_reasons: Vec<String>,
    selected_target_id: Option<String>,
    preview_only: bool,
    authority: RunAttemptLeaseDispatchPreflightAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunAttemptLeaseDispatchPreflightError {
    InvalidObservation,
}

impl fmt::Display for RunAttemptLeaseDispatchPreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid_run_attempt_lease_dispatch_preflight")
    }
}

impl std::error::Error for RunAttemptLeaseDispatchPreflightError {}

/// Decodes one caller-supplied preflight document after recursively rejecting
/// duplicate JSON object keys. The typed `Deserialize` implementation keeps
/// the value shape strict; this byte entrypoint closes `serde_json`'s
/// last-member-wins behavior before that implementation runs.
///
/// # Errors
///
/// Returns an error for malformed JSON, duplicate or unknown fields, missing
/// required fields, type mismatches, trailing input, or metadata rejected by
/// [`RunAttemptLeaseDispatchPreflightObservation::validate`].
pub fn decode(bytes: &[u8]) -> Result<RunAttemptLeaseDispatchPreflightObservation, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid_run_attempt_lease_dispatch_preflight: {error}"))
}

struct UniqueJson;

impl<'de> de::DeserializeSeed<'de> for UniqueJson {
    type Value = serde_json::Value;

    fn deserialize<D: de::Deserializer<'de>>(
        self,
        decoder: D,
    ) -> Result<serde_json::Value, D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> de::Visitor<'de> for UniqueJson {
    type Value = serde_json::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(serde_json::Value::String(value.to_owned()))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(UniqueJson)? {
            values.push(value);
        }
        Ok(serde_json::Value::Array(values))
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut object: A) -> Result<Self::Value, A::Error> {
        let mut keys = HashSet::new();
        let mut values = serde_json::Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            let value = object.next_value_seed(UniqueJson)?;
            values.insert(key, value);
        }
        Ok(serde_json::Value::Object(values))
    }
}

impl RunAttemptLeaseDispatchPreflightObservation {
    /// Validates a caller-supplied metadata envelope without gaining authority.
    ///
    /// # Errors
    ///
    /// Returns [`RunAttemptLeaseDispatchPreflightError::InvalidObservation`] for
    /// invalid schema, owner, identities, states, numeric bounds, derived readiness
    /// predicates or rejection reasons, or any selection/non-preview/authority claim.
    pub fn validate(&self) -> Result<(), RunAttemptLeaseDispatchPreflightError> {
        if self.schema_version != RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_SCHEMA_VERSION
            || self.evaluation_mode != RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_EVALUATION_MODE
            || !valid_owner(&self.owner)
            || !valid_identifier(&self.conversation_id)
            || !valid_identifier(&self.run_id)
            || !valid_run_status(&self.run_status)
            || self.run_state_admissible != (self.run_status == "nonterminal")
            || !valid_identifier(&self.attempt_id)
            || !valid_attempt_state(&self.attempt_state)
            || self.attempt_state_admissible != dispatchable_attempt_state(&self.attempt_state)
            || !valid_identifier(&self.command_id)
            || !valid_identifier(&self.intent_target_id)
            || self.lease_epoch == 0
            || self.lease_epoch > MAX_SAFE_INTEGER
            || self.evaluated_at_ms == 0
            || self.evaluated_at_ms > MAX_SAFE_INTEGER
            || self.candidate_count > MAX_CANDIDATES
            || self.declarative_ready_count > self.candidate_count
            || self.declarative_preflight_ready
                != (self.run_state_admissible
                    && self.attempt_state_admissible
                    && self.lease_active
                    && self.declarative_ready_count > 0)
            || !sorted_unique(&self.rejection_reasons)
            || self.rejection_reasons
                != expected_rejection_reasons(
                    self.run_state_admissible,
                    self.attempt_state_admissible,
                    self.lease_active,
                    self.declarative_ready_count,
                )
            || (self.declarative_preflight_ready && !self.rejection_reasons.is_empty())
            || self.selected_target_id.is_some()
            || !self.preview_only
            || self.authority != RunAttemptLeaseDispatchPreflightAuthority::default()
        {
            return Err(RunAttemptLeaseDispatchPreflightError::InvalidObservation);
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for RunAttemptLeaseDispatchPreflightObservation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::from_value::<RunAttemptLeaseDispatchPreflightWire>(
            UniqueJson.deserialize(deserializer)?,
        )
        .map_err(serde::de::Error::custom)
        .map(Self::from_wire)?;
        value.validate().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

impl RunAttemptLeaseDispatchPreflightObservation {
    fn from_wire(wire: RunAttemptLeaseDispatchPreflightWire) -> Self {
        Self {
            schema_version: wire.schema_version,
            evaluation_mode: wire.evaluation_mode,
            owner: wire.owner,
            conversation_id: wire.conversation_id,
            run_id: wire.run_id,
            run_status: wire.run_status,
            run_state_admissible: wire.run_state_admissible,
            attempt_id: wire.attempt_id,
            attempt_state: wire.attempt_state,
            attempt_state_admissible: wire.attempt_state_admissible,
            command_id: wire.command_id,
            intent_target_id: wire.intent_target_id,
            lease_epoch: wire.lease_epoch,
            lease_active: wire.lease_active,
            evaluated_at_ms: wire.evaluated_at_ms,
            candidate_count: wire.candidate_count,
            declarative_ready_count: wire.declarative_ready_count,
            declarative_preflight_ready: wire.declarative_preflight_ready,
            rejection_reasons: wire.rejection_reasons,
            selected_target_id: wire.selected_target_id,
            preview_only: wire.preview_only,
            authority: wire.authority,
        }
    }
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

fn valid_run_status(value: &str) -> bool {
    matches!(
        value,
        "nonterminal" | "completed" | "cancelled" | "limit_exceeded" | "failed"
    )
}

fn valid_attempt_state(value: &str) -> bool {
    matches!(
        value,
        "requested"
            | "accepted"
            | "starting"
            | "running"
            | "interrupted"
            | "completed"
            | "failed"
            | "uncertain"
    )
}

fn dispatchable_attempt_state(value: &str) -> bool {
    matches!(value, "accepted" | "starting" | "running")
}

fn expected_rejection_reasons(
    run_state_admissible: bool,
    attempt_state_admissible: bool,
    lease_active: bool,
    ready_count: u32,
) -> Vec<String> {
    let mut reasons = Vec::with_capacity(4);
    if !attempt_state_admissible {
        reasons.push("attempt_state_not_dispatchable".to_owned());
    }
    if !lease_active {
        reasons.push("lease_inactive_at_evaluated_time".to_owned());
    }
    if ready_count == 0 {
        reasons.push("no_declarative_ready_candidate".to_owned());
    }
    if !run_state_admissible {
        reasons.push("run_state_not_dispatchable".to_owned());
    }
    reasons
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const FIXTURE: &str = include_str!(
        "../../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
    );

    #[test]
    fn canonical_fixture_is_strict_and_metadata_only() {
        let value: RunAttemptLeaseDispatchPreflightObservation =
            serde_json::from_str(FIXTURE).expect("canonical preflight fixture");
        assert!(value.validate().is_ok());
        assert!(value.selected_target_id.is_none());
        assert!(value.preview_only);
        assert_eq!(
            value.authority,
            RunAttemptLeaseDispatchPreflightAuthority::default()
        );
    }

    #[test]
    fn authority_unknown_and_readiness_mutations_fail_closed() {
        let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
        value["authority"]["dispatch_performed"] = Value::Bool(true);
        assert!(
            serde_json::from_value::<RunAttemptLeaseDispatchPreflightObservation>(value).is_err()
        );

        let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
        value["selected_target_id"] = Value::String("runner-1".into());
        assert!(
            serde_json::from_value::<RunAttemptLeaseDispatchPreflightObservation>(value).is_err()
        );

        let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
        value["unexpected"] = Value::Bool(true);
        assert!(
            serde_json::from_value::<RunAttemptLeaseDispatchPreflightObservation>(value).is_err()
        );

        let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
        value["lease_epoch"] = Value::Number((MAX_SAFE_INTEGER + 1).into());
        assert!(
            serde_json::from_value::<RunAttemptLeaseDispatchPreflightObservation>(value).is_err()
        );
    }

    #[test]
    fn byte_decode_rejects_root_and_nested_duplicate_keys() {
        let root = FIXTURE.replacen(
            "\"schema_version\":",
            "\"schema_version\":\"forge.run-attempt-lease-dispatch-preflight/v1\",\n  \"schema_version\":",
            1,
        );
        assert!(decode(root.as_bytes()).is_err());
        assert!(
            serde_json::from_str::<RunAttemptLeaseDispatchPreflightObservation>(&root).is_err()
        );

        let nested = FIXTURE.replacen(
            "    \"issuer\": \"https://id.example\",",
            "    \"issuer\": \"https://id.example\",\n    \"issuer\": \"https://id.example\",",
            1,
        );
        assert!(decode(nested.as_bytes()).is_err());
        assert!(
            serde_json::from_str::<RunAttemptLeaseDispatchPreflightObservation>(&nested).is_err()
        );
    }
}
