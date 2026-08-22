use serde::Serialize;

use super::{
    ArtifactRef, KernelOperationalContractError, MAX_ARTIFACT_REF_BYTES, primitives, wire,
};

/// Emits exact compact canonical JSON within the closure-level wire profile.
///
/// # Errors
///
/// Returns an error for unsupported JSON values or any frozen wire bound violation.
pub fn canonical_json<T>(value: &T) -> Result<String, KernelOperationalContractError>
where
    T: Serialize + ?Sized,
{
    wire::canonical_typed(value)
}

/// Decodes the exact reused `ArtifactRef` value object.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_artifact_ref(bytes: &[u8]) -> Result<ArtifactRef, KernelOperationalContractError> {
    let artifact: ArtifactRef = wire::decode_typed(bytes, MAX_ARTIFACT_REF_BYTES)?;
    primitives::validate_artifact(&artifact, "ArtifactRef")?;
    Ok(artifact)
}
