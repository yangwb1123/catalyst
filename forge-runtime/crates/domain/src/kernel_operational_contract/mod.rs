mod closure;
mod codec;
mod constants;
mod graph;
mod model;
mod primitives;
mod records;
mod validation;
mod wire;

use std::fmt;

pub use closure::{decode_closure, seal_closure, validate_closure};
pub use codec::{canonical_json, decode_artifact_ref};
pub use model::*;
pub use records::{
    decode_artifact_receipt, decode_capability_invocation, decode_execution_receipt,
    decode_interaction_event, seal_artifact_receipt, seal_capability_invocation,
    seal_execution_receipt, seal_interaction_event,
};

pub const SUCCESS_MARKER: &str = "STRUCTURALLY_VALID_KERNEL_OPERATIONAL_REFERENCE_CLOSURE_V1 (exact caller-supplied records and acyclic references only; no content provenance, principal, Grant, or source/context/environment/policy binding authentication; no authorization, permission, event append, persistence, transition, execution, outcome, completion, effect, or usage measurement attestation)";

pub const MAX_ARTIFACT_RECEIPT_BYTES: usize = 262_144;
pub const MAX_INVOCATION_BYTES: usize = 524_288;
pub const MAX_EVENT_BYTES: usize = 262_144;
pub const MAX_EXECUTION_RECEIPT_BYTES: usize = 1_048_576;
pub const MAX_CLOSURE_BYTES: usize = 16_777_216;
pub const MAX_ARTIFACT_REF_BYTES: usize = 16_384;
pub const MAX_JSON_DEPTH: usize = 16;
pub const MAX_OBJECT_FIELDS: usize = 64;
pub const MAX_ARRAY_ITEMS: usize = 256;
pub const MAX_STRING_BYTES: usize = 16_384;
pub const MAX_SHORT_BYTES: usize = 160;
pub const MAX_REFERENCE_BYTES: usize = 4_096;

pub(super) const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub(super) const MAX_ARTIFACTS: usize = 256;
pub(super) const MAX_ARTIFACT_RECEIPTS: usize = 64;
pub(super) const MAX_INVOCATIONS: usize = 64;
pub(super) const MAX_EVENTS: usize = 256;
pub(super) const MAX_EXECUTION_RECEIPTS: usize = 64;
pub(super) const MAX_ATTEMPT: i64 = 64;
pub(super) const MAX_IO_ITEMS: usize = 32;
pub(super) const MAX_REASON_CODES: usize = 32;
pub(super) const MAX_CONFIDENCE_MICROS: i64 = 1_000_000;
pub(super) const MAX_CALL_COUNT: i64 = 1_000_000_000;
pub(super) const MAX_COST_MICROS: i64 = 1_000_000_000_000_000;
pub(super) const MAX_ELAPSED_MS: i64 = 86_400_000;
pub(super) const MAX_TOKEN_COUNT: i64 = 1_000_000_000;
pub(super) const MAX_NETWORK_BYTES: i64 = 1_073_741_824;
pub(super) const MAX_OUTPUT_BYTES: i64 = 1_073_741_824;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelOperationalContractError {
    pub message: String,
}

impl fmt::Display for KernelOperationalContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for KernelOperationalContractError {}

pub(super) fn invalid(message: impl Into<String>) -> KernelOperationalContractError {
    KernelOperationalContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
