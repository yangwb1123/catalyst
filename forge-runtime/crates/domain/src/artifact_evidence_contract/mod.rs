mod codec;
mod mapping;
mod model;
mod timestamp;
mod validation;

use std::fmt;

pub use codec::{
    artifact_source_sha256, canonical_artifact_json, canonical_request_json,
    decode_canonical_request, request_sha256,
};
pub use mapping::{adapt_canonical_request, validate_adaptation};
pub use model::*;

pub const API_VERSION: &str = "forgeos.governance.artifact-evidence-adapter/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const SOURCE_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.artifact-provenance-source.v1\0";
pub const REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forgeos.governance.artifact-evidence-adapter.request.v1\0";
pub const ADAPTED_SHADOW: &str =
    "ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect attestation)";
pub const MAX_REQUEST_BYTES: usize = 131_072;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactEvidenceContractError {
    pub message: String,
}

impl fmt::Display for ArtifactEvidenceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ArtifactEvidenceContractError {}

pub(super) fn invalid(message: impl Into<String>) -> ArtifactEvidenceContractError {
    ArtifactEvidenceContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
