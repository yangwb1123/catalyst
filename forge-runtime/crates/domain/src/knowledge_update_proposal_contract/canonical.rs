use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{KnowledgeUpdateProposalContractError, invalid};

pub(super) fn encode(
    value: &(impl Serialize + ?Sized),
    maximum: usize,
    label: &str,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    let encoded = crate::governance_contract::codec::canonical_json(value)
        .map_err(|error| invalid(format!("{label}: {}", error.message)))?;
    if encoded.len() > maximum {
        Err(invalid(format!(
            "{label} exceeds {maximum} canonical bytes"
        )))
    } else {
        Ok(encoded)
    }
}

pub(super) fn domain_sha256(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    crate::governance_contract::codec::lower_hex(&digest.finalize())
}
