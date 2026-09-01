mod artifact;
mod codec;
mod command;
mod envelope;
mod event;
mod execution_receipt;
mod identity;
mod model;
mod receipt_model;
mod references;
mod state;
mod verification;
mod wire;
mod wire_enum;

use std::fmt;

pub use artifact::validate_artifact_ref;
pub use codec::{
    artifact_ref_sha256, canonical_artifact_ref_json, canonical_command_envelope_json,
    canonical_event_envelope_json, canonical_execution_receipt_json,
    canonical_verification_receipt_json, canonical_verification_request_json,
    command_envelope_sha256, decode_canonical_artifact_ref, decode_canonical_command_envelope,
    decode_canonical_event_envelope, decode_canonical_execution_receipt,
    decode_canonical_verification_receipt, decode_canonical_verification_request,
    event_envelope_sha256, execution_receipt_sha256, verification_receipt_sha256,
    verification_request_sha256,
};
pub use command::validate_command_envelope;
pub(crate) use envelope::validate_idempotency_key;
pub use event::validate_event_envelope;
pub use execution_receipt::validate_execution_receipt;
pub(crate) use execution_receipt::validate_executor_descriptor;
pub use identity::validate_platform_id;
pub use model::*;
pub use receipt_model::*;
pub(crate) use references::{validate_entity_ref, validate_record_ref, validate_scope_ref};
pub use state::{
    validate_action_transition, validate_attempt_transition, validate_work_item_transition,
};
pub use verification::{
    validate_verification_exchange, validate_verification_receipt, validate_verification_request,
};

pub const CANONICALIZATION: &str = "forge.canonical-json/v1";
pub const ENVELOPE_VERSION: i64 = 1;
pub const RECEIPT_VERSION: i64 = 1;
pub const MAX_ARTIFACT_REF_BYTES: usize = 16 * 1024;
pub const MAX_ENVELOPE_BYTES: usize = 256 * 1024;
pub const MAX_RECEIPT_BYTES: usize = 256 * 1024;
pub const MAX_PAYLOAD_BYTES: usize = 32 * 1024;
pub const MAX_EXTENSIONS_BYTES: usize = 8 * 1024;
pub const MAX_JSON_DEPTH: usize = 12;
pub const MAX_OBJECT_FIELDS: usize = 64;
pub const MAX_ARRAY_ITEMS: usize = 256;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_EXTENSION_FIELDS: usize = 16;
pub const MAX_ARTIFACT_BYTES: i64 = 1_i64 << 40;
pub const MAX_UNIX_MILLISECONDS: i64 = 253_402_300_799_999;
pub const MAX_RECEIPT_ARTIFACTS: usize = 32;
pub const MAX_VERIFICATION_CHECKS: usize = 64;
pub const MAX_REASON_CODES: usize = 16;
pub const MAX_EVIDENCE_REFS: usize = 16;
pub const MAX_EXECUTION_ELAPSED_MS: i64 = 31_536_000_000;
pub const MAX_OBSERVED_COUNT: i64 = 1_000_000_000;
pub const MAX_OBSERVED_QUANTITY: i64 = 1_000_000_000_000_000;

const ARTIFACT_DIGEST_DOMAIN: &[u8] = b"forge.platform.artifact-ref.v1\0";
const COMMAND_DIGEST_DOMAIN: &[u8] = b"forge.platform.command-envelope.v1\0";
const EVENT_DIGEST_DOMAIN: &[u8] = b"forge.platform.event-envelope.v1\0";
const EXECUTION_RECEIPT_DIGEST_DOMAIN: &[u8] = b"forge.platform.execution-receipt.v1\0";
const VERIFICATION_REQUEST_DIGEST_DOMAIN: &[u8] = b"forge.platform.verification-request.v1\0";
const VERIFICATION_RECEIPT_DIGEST_DOMAIN: &[u8] = b"forge.platform.verification-receipt.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionCode {
    DocumentInvalid,
    IdentifierInvalid,
    ValueInvalid,
    ReferenceMismatch,
    StateInvalid,
    TransitionInvalid,
    RelationMismatch,
}

impl RejectionCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DocumentInvalid => "pc_document_invalid",
            Self::IdentifierInvalid => "pc_identifier_invalid",
            Self::ValueInvalid => "pc_value_invalid",
            Self::ReferenceMismatch => "pc_reference_mismatch",
            Self::StateInvalid => "pc_state_invalid",
            Self::TransitionInvalid => "pc_transition_invalid",
            Self::RelationMismatch => "pc_relation_mismatch",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformCoreContractError {
    pub code: RejectionCode,
    pub message: String,
}

impl fmt::Display for PlatformCoreContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for PlatformCoreContractError {}

fn invalid(message: impl Into<String>) -> PlatformCoreContractError {
    reject(RejectionCode::ValueInvalid, message)
}

fn reject(code: RejectionCode, message: impl Into<String>) -> PlatformCoreContractError {
    PlatformCoreContractError {
        code,
        message: message.into(),
    }
}

fn recode(error: PlatformCoreContractError, code: RejectionCode) -> PlatformCoreContractError {
    PlatformCoreContractError {
        code,
        message: error.message,
    }
}

#[cfg(test)]
mod tests;
