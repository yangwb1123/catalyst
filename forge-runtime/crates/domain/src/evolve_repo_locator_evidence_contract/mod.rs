mod codec;
mod mapping;
mod model;
mod validation;

use std::fmt;

pub use codec::{
    canonical_locator_json, canonical_observation_json, canonical_request_json,
    decode_canonical_request, locator_sha256, request_sha256, source_snapshot_sha256,
};
pub use mapping::{adapt_canonical_request, validate_adaptation};
pub use model::*;

pub const API_VERSION: &str = "forgeos.governance.evolve-repo-locator-evidence-adapter/v1";
pub const OBSERVATION_API_VERSION: &str = "forgeos.evolve-repo-locator/v1";
pub const SCAN_CONTRACT: &str = "evolve_scan_v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const LOCATOR_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.evolve-repo-locator.locator.v1\0";
pub const SOURCE_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.evolve-repo-locator-source.v1\0";
pub const REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forgeos.governance.evolve-repo-locator-evidence-adapter.request.v1\0";
pub const ADAPTED_SHADOW: &str = "ADAPTED_SHADOW (locator mapping only; no file/report verification, scan judgment, completion, truth, authority, claim, atom, persistence, or effect attestation)";
pub const MAX_REQUEST_BYTES: usize = 131_072;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvolveRepoLocatorEvidenceContractError {
    pub message: String,
}

impl fmt::Display for EvolveRepoLocatorEvidenceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for EvolveRepoLocatorEvidenceContractError {}

pub(super) fn invalid(message: impl Into<String>) -> EvolveRepoLocatorEvidenceContractError {
    EvolveRepoLocatorEvidenceContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
