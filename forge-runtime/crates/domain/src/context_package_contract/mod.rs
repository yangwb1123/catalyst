mod assembly;
mod codec;
mod model;
mod package;
mod package_validation;
mod validation;

use std::fmt;

pub use assembly::{assemble, validate_cache_hit, validate_package};
pub use codec::{
    cache_key_sha256, canonical_package_json, canonical_request_json, decode_canonical_package,
    decode_canonical_request, request_sha256,
};
pub use model::*;

pub const BUILD_REQUEST_API_VERSION: &str = "forgeos.context-package-build-request/v1";
pub const CONTEXT_PACKAGE_API_VERSION: &str = "forgeos.context-package/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const ASSEMBLY_MODE: &str = "authority_free_deterministic_context_projection";
pub const NORMALIZATION: &str = "exact_lf_utf8_after_declared_redactions";
pub const DELIMITER: &str = "structured_json_lane_no_text_delimiter";
pub const RESULT: &str = "ASSEMBLED_SHADOW (no truth, authority, instruction, permission, approval, completion, persistence, or effect attestation)";
pub const REQUEST_DIGEST_DOMAIN: &[u8] = b"forgeos.context-package-build-request.v1\0";
pub const CACHE_KEY_DIGEST_DOMAIN: &[u8] = b"forgeos.context-package-cache-key.v1\0";
pub const CONTEXT_DIGEST_DOMAIN: &[u8] = b"forgeos.context-package.v1\0";
pub const SNIPPET_DIGEST_DOMAIN: &[u8] = b"forgeos.context-snippet.v1\0";
pub const PROJECTED_CONTENT_DIGEST_DOMAIN: &[u8] = b"forgeos.context-content.v1\0";
pub const PROJECTION_DIGEST_DOMAIN: &[u8] = b"forgeos.context-package-projection.v1\0";
pub const REDACTION_REPLACEMENT: &str = "[REDACTED]";

pub const MAX_REQUEST_BYTES: usize = 20 * 1_024 * 1_024;
pub const MAX_PACKAGE_BYTES: usize = 2 * 1_024 * 1_024;
pub const MAX_SOURCE_CONTENT_BYTES: usize = 131_072;
pub const MAX_STRING_BYTES: usize = 131_072;
pub const MAX_SHORT_TEXT_BYTES: usize = 160;
pub const MAX_REFERENCE_BYTES: usize = 4_096;
pub const MAX_DEPTH: usize = 16;
pub const MAX_OBJECT_FIELDS: usize = 32;
pub const MAX_ARRAY_ITEMS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenizerIdentity {
    pub tokenizer_id: String,
    pub tokenizer_sha256: String,
}

pub trait TokenCounter {
    fn identity(&self) -> TokenizerIdentity;

    /// Counts the exact canonical projection bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the pinned tokenizer cannot count the projection.
    fn count(&self, projection: &[u8]) -> Result<u64, ContextPackageContractError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextPackageContractError {
    pub message: String,
}

impl fmt::Display for ContextPackageContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ContextPackageContractError {}

pub(super) fn invalid(message: impl Into<String>) -> ContextPackageContractError {
    ContextPackageContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
