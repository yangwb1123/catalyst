mod model;
mod projection;
mod validation;

use std::fmt;

pub use model::*;
pub use projection::*;

pub const API_VERSION: &str = "forgeos.aadm.cognitive-atom/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const ATOM_DIGEST_DOMAIN: &[u8] = b"forgeos.aadm.cognitive-atom.v1\0";
pub const ATOM_ID_DOMAIN: &[u8] = b"forgeos.aadm.cognitive-atom-id.v1\0";
pub const ATOM_SET_DIGEST_DOMAIN: &[u8] = b"forgeos.aadm.cognitive-atom-set.v1\0";
pub const SOURCE_CLOSURE_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.record-set.v1\0";
pub const MAX_ATOM_BYTES: usize = 131_072;
pub const MAX_ATOM_SET_BYTES: usize = 1_048_576;
pub const MAX_ATOMS: usize = 256;
pub const PROJECTED_SHADOW: &str = "PROJECTED_SHADOW (no truth, authority, instruction, hard-guard, transition, completion or effect attestation)";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CognitiveAtomContractError {
    pub message: String,
}

impl fmt::Display for CognitiveAtomContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CognitiveAtomContractError {}

impl CognitiveAtom {
    /// Validates the exact v1 shape and digest without asserting truth or authority.
    ///
    /// # Errors
    ///
    /// Returns an error for any identity, semantic, canonicalization, size, or
    /// digest violation.
    pub fn validate(&self) -> Result<(), CognitiveAtomContractError> {
        validation::validate_atom(self)
    }
}

fn invalid(message: impl Into<String>) -> CognitiveAtomContractError {
    CognitiveAtomContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
