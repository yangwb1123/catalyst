mod codec;
mod mapping;
mod model;
mod validation;

use std::fmt;

pub use codec::{
    canonical_command_json, canonical_observation_json, canonical_request_json, command_sha256,
    decode_canonical_request, request_sha256, source_snapshot_sha256,
};
pub use mapping::{adapt_canonical_request, validate_adaptation};
pub use model::*;

pub const API_VERSION: &str = "forgeos.governance.command-observation-evidence-adapter/v1";
pub const OBSERVATION_API_VERSION: &str = "forgeos.command-observation/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const COMMAND_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.command-observation.command.v1\0";
pub const SOURCE_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.command-observation-source.v1\0";
pub const REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forgeos.governance.command-observation-evidence-adapter.request.v1\0";
pub const ADAPTED_SHADOW: &str = "ADAPTED_SHADOW (observation mapping only; no execution, pass, completion, truth, authority, claim, atom, persistence, or effect attestation)";
pub const MAX_REQUEST_BYTES: usize = 131_072;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandObservationEvidenceContractError {
    pub message: String,
}

impl fmt::Display for CommandObservationEvidenceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandObservationEvidenceContractError {}

pub(super) fn invalid(message: impl Into<String>) -> CommandObservationEvidenceContractError {
    CommandObservationEvidenceContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
