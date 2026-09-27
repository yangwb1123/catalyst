//! Pure Prompt append request/receipt compatibility projection.
//!
//! The input content and idempotency key are reduced to SHA-256 digests before
//! the returned value is serialized. This module has no Hub, owner, device,
//! lease, Runner, or Audit authority.

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::runner_execution_intent::RunnerExecutionOwner;

pub const PROMPT_APPEND_RECEIPT_SCHEMA_VERSION: &str = "forge.prompt-append-receipt/v1";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_CONTENT_BYTES: usize = 256 * 1024;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptAppendReceiptInput {
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub expected_version: u64,
    pub role: String,
    pub content: String,
    pub idempotency_key: String,
    pub prompt_id: String,
    pub created_at_ms: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromptAppendReceiptRequest {
    pub conversation_id: String,
    pub expected_version: u64,
    pub role: String,
    pub content_sha256: String,
    pub idempotency_key_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromptAppendReceipt {
    pub conversation_id: String,
    pub prompt_id: String,
    pub role: String,
    pub aggregate_version: u64,
    pub created_at_ms: u64,
    pub replayed: bool,
    pub storage_commit_observed: bool,
    pub content_included: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromptAppendReceiptAuthority {
    pub run_created: bool,
    pub device_selected: bool,
    pub reservation_created: bool,
    pub dispatch_performed: bool,
    pub execution_authorized: bool,
    pub audit_published: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromptAppendReceiptObservation {
    pub schema_version: String,
    pub owner: RunnerExecutionOwner,
    pub request: PromptAppendReceiptRequest,
    pub receipt: PromptAppendReceipt,
    pub authority: PromptAppendReceiptAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptAppendReceiptError {
    Invalid,
}

impl fmt::Display for PromptAppendReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Prompt append receipt is invalid")
    }
}

impl std::error::Error for PromptAppendReceiptError {}

pub fn observe(
    input: PromptAppendReceiptInput,
) -> Result<PromptAppendReceiptObservation, PromptAppendReceiptError> {
    if !valid_input(&input) || input.expected_version == MAX_SAFE_INTEGER {
        return Err(PromptAppendReceiptError::Invalid);
    }
    let observation = PromptAppendReceiptObservation {
        schema_version: PROMPT_APPEND_RECEIPT_SCHEMA_VERSION.into(),
        owner: input.owner,
        request: PromptAppendReceiptRequest {
            conversation_id: input.conversation_id.clone(),
            expected_version: input.expected_version,
            role: input.role,
            content_sha256: digest(&input.content),
            idempotency_key_sha256: digest(&input.idempotency_key),
        },
        receipt: PromptAppendReceipt {
            conversation_id: input.conversation_id,
            prompt_id: input.prompt_id,
            role: "user".into(),
            aggregate_version: input.expected_version + 1,
            created_at_ms: input.created_at_ms,
            replayed: input.replayed,
            storage_commit_observed: true,
            content_included: false,
        },
        authority: PromptAppendReceiptAuthority::default(),
    };
    observation.validate()?;
    Ok(observation)
}

impl PromptAppendReceiptObservation {
    pub fn validate(&self) -> Result<(), PromptAppendReceiptError> {
        if self.schema_version != PROMPT_APPEND_RECEIPT_SCHEMA_VERSION
            || !valid_owner(&self.owner)
            || !valid_identifier(&self.request.conversation_id)
            || self.request.role != "user"
            || !valid_digest(&self.request.content_sha256)
            || !valid_digest(&self.request.idempotency_key_sha256)
            || self.request.expected_version == 0
            || self.request.expected_version > MAX_SAFE_INTEGER
            || !valid_identifier(&self.receipt.conversation_id)
            || self.receipt.conversation_id != self.request.conversation_id
            || !valid_identifier(&self.receipt.prompt_id)
            || self.receipt.role != self.request.role
            || self.receipt.aggregate_version != self.request.expected_version + 1
            || self.receipt.aggregate_version > MAX_SAFE_INTEGER
            || self.receipt.created_at_ms > MAX_SAFE_INTEGER
            || !self.receipt.storage_commit_observed
            || self.receipt.content_included
            || self.authority != PromptAppendReceiptAuthority::default()
        {
            return Err(PromptAppendReceiptError::Invalid);
        }
        Ok(())
    }
}

fn valid_input(input: &PromptAppendReceiptInput) -> bool {
    valid_owner(&input.owner)
        && valid_identifier(&input.conversation_id)
        && input.expected_version > 0
        && input.expected_version <= MAX_SAFE_INTEGER
        && input.role == "user"
        && !input.content.trim().is_empty()
        && input.content.len() <= MAX_CONTENT_BYTES
        && !input.idempotency_key.trim().is_empty()
        && input.idempotency_key.len() <= MAX_IDEMPOTENCY_KEY_BYTES
        && valid_identifier(&input.prompt_id)
        && input.created_at_ms <= MAX_SAFE_INTEGER
}

fn valid_owner(owner: &RunnerExecutionOwner) -> bool {
    valid_text(&owner.issuer, MAX_OWNER_PART_BYTES)
        && valid_text(&owner.subject, MAX_OWNER_PART_BYTES)
        && valid_text(&owner.tenant_id, MAX_OWNER_PART_BYTES)
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_IDENTIFIER_BYTES)
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric() || (index > 0 && ".:_+/-".contains(character))
        })
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
#[path = "prompt_append_receipt_tests.rs"]
mod tests;
