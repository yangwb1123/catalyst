use sha2::{Digest, Sha256};

use super::{DIGEST_DOMAIN, WorkIntent, WorkIntentContractError, invalid, validation, wire};

/// Decodes one exact compact canonical `WorkIntent` v1 instance.
///
/// # Errors
///
/// Returns an error for malformed, duplicate, unknown, missing, noncanonical,
/// oversized, semantically invalid, or incorrectly sealed input.
pub fn decode_canonical_work_intent(bytes: &[u8]) -> Result<WorkIntent, WorkIntentContractError> {
    let intent = wire::decode_typed_and_shape(bytes)?;
    validation::validate_work_intent(&intent)?;
    let canonical = wire::canonical_work_intent_unchecked(&intent)?;
    if canonical.as_bytes() == bytes {
        Ok(intent)
    } else {
        Err(invalid(
            "WorkIntent input is not exact compact canonical JSON",
        ))
    }
}

/// Encodes one validated sealed `WorkIntent` as exact compact canonical JSON.
///
/// # Errors
///
/// Returns an error when any declaration, bound, order, or self-identity is invalid.
pub fn canonical_work_intent_json(intent: &WorkIntent) -> Result<String, WorkIntentContractError> {
    validation::validate_work_intent(intent)?;
    wire::canonical_work_intent_unchecked(intent)
}

/// Computes the digest after blanking both identity fields.
///
/// # Errors
///
/// Returns an error when any non-identity declaration or preimage bound is invalid.
pub fn work_intent_sha256(intent: &WorkIntent) -> Result<String, WorkIntentContractError> {
    work_intent_sha256_unchecked(intent)
}

/// Seals an exact declaration whose identity fields are both empty.
///
/// The caller's value is not mutated, and no authority or runtime effect is inferred.
///
/// # Errors
///
/// Returns an error for nonempty identity fields or any invalid declaration or bound.
pub fn seal_work_intent(intent: &WorkIntent) -> Result<WorkIntent, WorkIntentContractError> {
    if !intent.work_intent_id.is_empty() || !intent.work_intent_sha256.is_empty() {
        return Err(invalid(
            "sealing requires empty work_intent_id and work_intent_sha256",
        ));
    }
    validation::validate_body(intent)?;
    let digest = work_intent_sha256_unchecked(intent)?;
    let mut sealed = intent.clone();
    sealed.work_intent_sha256.clone_from(&digest);
    sealed.work_intent_id = format!("work-intent-{digest}");
    validation::validate_work_intent(&sealed)?;
    Ok(sealed)
}

pub(super) fn work_intent_sha256_unchecked(
    intent: &WorkIntent,
) -> Result<String, WorkIntentContractError> {
    validation::validate_body(intent)?;
    let mut blank = intent.clone();
    blank.work_intent_id.clear();
    blank.work_intent_sha256.clear();
    let preimage = wire::canonical_work_intent_unchecked(&blank)?;
    let mut digest = Sha256::new();
    digest.update(DIGEST_DOMAIN);
    digest.update(preimage.as_bytes());
    Ok(crate::governance_contract::codec::lower_hex(
        &digest.finalize(),
    ))
}
