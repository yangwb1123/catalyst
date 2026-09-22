//! Pure device identity and one-time challenge binding.
//!
//! This reference model compares already verified owner declarations and
//! configured device bindings. It does not perform cryptography, issue or
//! consume credentials, mutate approval, persist a record, or grant runtime
//! authority.

use std::fmt;

pub const IDENTITY_PROOF_SCHEMA_VERSION: &str = "forge.device-identity-proof-contract/v1";
pub const IDENTITY_PROOF_EVALUATION_MODE: &str = "pure_binding_only";
const DIGEST_HEX_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityOwner {
    pub(super) issuer: String,
    pub(super) subject: String,
    pub(super) tenant_id: String,
}

impl IdentityOwner {
    #[must_use]
    pub fn new(
        issuer: impl Into<String>,
        subject: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            issuer: issuer.into(),
            subject: subject.into(),
            tenant_id: tenant_id.into(),
        }
    }

    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    #[must_use]
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdentityBinding {
    pub(super) device_id: String,
    pub(super) owner: IdentityOwner,
    pub(super) key_id: String,
    pub(super) public_key_sha256: String,
    pub(super) approval_state: String,
    pub(super) credential_state: String,
}

impl DeviceIdentityBinding {
    #[must_use]
    pub fn new(
        device_id: impl Into<String>,
        owner: IdentityOwner,
        key_id: impl Into<String>,
        public_key_sha256: impl Into<String>,
        approval_state: impl Into<String>,
        credential_state: impl Into<String>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            owner,
            key_id: key_id.into(),
            public_key_sha256: public_key_sha256.into(),
            approval_state: approval_state.into(),
            credential_state: credential_state.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityChallenge {
    pub(super) challenge_id: String,
    pub(super) challenge_sha256: String,
    pub(super) issued_at_ms: u64,
    pub(super) expires_at_ms: u64,
    pub(super) consumed: bool,
}

impl IdentityChallenge {
    #[must_use]
    pub fn new(
        challenge_id: impl Into<String>,
        challenge_sha256: impl Into<String>,
        issued_at_ms: u64,
        expires_at_ms: u64,
        consumed: bool,
    ) -> Self {
        Self {
            challenge_id: challenge_id.into(),
            challenge_sha256: challenge_sha256.into(),
            issued_at_ms,
            expires_at_ms,
            consumed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdentityProof {
    pub(super) device_id: String,
    pub(super) key_id: String,
    pub(super) public_key_sha256: String,
    pub(super) owner: IdentityOwner,
    pub(super) challenge_id: String,
    pub(super) challenge_sha256: String,
    pub(super) proof_sha256: String,
    pub(super) issued_at_ms: u64,
    pub(super) expires_at_ms: u64,
}

impl DeviceIdentityProof {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        device_id: impl Into<String>,
        key_id: impl Into<String>,
        public_key_sha256: impl Into<String>,
        owner: IdentityOwner,
        challenge_id: impl Into<String>,
        challenge_sha256: impl Into<String>,
        proof_sha256: impl Into<String>,
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            key_id: key_id.into(),
            public_key_sha256: public_key_sha256.into(),
            owner,
            challenge_id: challenge_id.into(),
            challenge_sha256: challenge_sha256.into(),
            proof_sha256: proof_sha256.into(),
            issued_at_ms,
            expires_at_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityProofDecision {
    pub(super) identity_bound: bool,
    pub(super) approval_required: bool,
    pub(super) reason: &'static str,
}

impl IdentityProofDecision {
    #[must_use]
    pub fn identity_bound(&self) -> bool {
        self.identity_bound
    }

    #[must_use]
    pub fn approval_required(&self) -> bool {
        self.approval_required
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityProofError {
    OwnerMismatch,
    DeviceMismatch,
    KeyMismatch,
    CredentialRevoked,
    CredentialExpired,
    ChallengeMismatch,
    ChallengeReplayed,
    ChallengeExpired,
    ChallengeNotYetValid,
    ProofExpired,
    ProofNotYetValid,
    InvalidProofWindow,
    InvalidBinding,
    UnknownApprovalState,
    UnknownCredentialState,
}

impl fmt::Display for IdentityProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::OwnerMismatch => "owner_mismatch",
            Self::DeviceMismatch => "device_mismatch",
            Self::KeyMismatch => "key_mismatch",
            Self::CredentialRevoked => "credential_revoked",
            Self::CredentialExpired => "credential_expired",
            Self::ChallengeMismatch => "challenge_mismatch",
            Self::ChallengeReplayed => "challenge_replayed",
            Self::ChallengeExpired => "challenge_expired",
            Self::ChallengeNotYetValid => "challenge_not_yet_valid",
            Self::ProofExpired => "proof_expired",
            Self::ProofNotYetValid => "proof_not_yet_valid",
            Self::InvalidProofWindow => "invalid_proof_window",
            Self::InvalidBinding => "invalid_binding",
            Self::UnknownApprovalState => "unknown_approval_state",
            Self::UnknownCredentialState => "unknown_credential_state",
        };
        formatter.write_str(value)
    }
}

impl std::error::Error for IdentityProofError {}

/// Evaluates a proof against explicit, already-bound inputs.
///
/// The function is intentionally effect-free: `now_ms` and challenge
/// consumption are caller-supplied facts, and no field is mutated.
///
/// # Errors
///
/// Returns a bounded rejection when owner, device, key, credential, challenge,
/// proof window, or approval state does not satisfy the reference contract.
pub fn evaluate_identity_proof(
    owner: &IdentityOwner,
    device: &DeviceIdentityBinding,
    challenge: &IdentityChallenge,
    proof: &DeviceIdentityProof,
    now_ms: u64,
) -> Result<IdentityProofDecision, IdentityProofError> {
    validate_static_binding(owner, device, proof)?;
    // Validate challenge structure before credential state, matching the Go
    // reference model's fail-closed precedence for malformed inputs.
    validate_challenge_shape(challenge)?;
    validate_credential(&device.credential_state)?;
    validate_challenge(challenge, proof, now_ms)?;
    validate_proof_window(proof, now_ms)?;
    let approval_required = approval_required(&device.approval_state)?;
    let reason = if approval_required {
        "bound_pending_approval"
    } else {
        "bound_approved"
    };
    Ok(IdentityProofDecision {
        identity_bound: true,
        approval_required,
        reason,
    })
}

fn validate_static_binding(
    owner: &IdentityOwner,
    device: &DeviceIdentityBinding,
    proof: &DeviceIdentityProof,
) -> Result<(), IdentityProofError> {
    if owner != &device.owner || owner != &proof.owner {
        return Err(IdentityProofError::OwnerMismatch);
    }
    if proof.device_id != device.device_id {
        return Err(IdentityProofError::DeviceMismatch);
    }
    if proof.key_id != device.key_id || proof.public_key_sha256 != device.public_key_sha256 {
        return Err(IdentityProofError::KeyMismatch);
    }
    if !valid_binding(device) || !valid_proof(proof) {
        return Err(IdentityProofError::InvalidBinding);
    }
    Ok(())
}

fn validate_credential(state: &str) -> Result<(), IdentityProofError> {
    match state {
        "active" => Ok(()),
        "revoked" => Err(IdentityProofError::CredentialRevoked),
        "expired" => Err(IdentityProofError::CredentialExpired),
        _ => Err(IdentityProofError::UnknownCredentialState),
    }
}

fn validate_challenge(
    challenge: &IdentityChallenge,
    proof: &DeviceIdentityProof,
    now_ms: u64,
) -> Result<(), IdentityProofError> {
    if proof.challenge_id != challenge.challenge_id
        || proof.challenge_sha256 != challenge.challenge_sha256
    {
        return Err(IdentityProofError::ChallengeMismatch);
    }
    if challenge.consumed {
        return Err(IdentityProofError::ChallengeReplayed);
    }
    if now_ms < challenge.issued_at_ms {
        return Err(IdentityProofError::ChallengeNotYetValid);
    }
    if now_ms >= challenge.expires_at_ms {
        return Err(IdentityProofError::ChallengeExpired);
    }
    Ok(())
}

fn validate_challenge_shape(challenge: &IdentityChallenge) -> Result<(), IdentityProofError> {
    if valid_challenge(challenge) {
        Ok(())
    } else {
        Err(IdentityProofError::InvalidBinding)
    }
}

fn validate_proof_window(
    proof: &DeviceIdentityProof,
    now_ms: u64,
) -> Result<(), IdentityProofError> {
    if now_ms < proof.issued_at_ms {
        return Err(IdentityProofError::ProofNotYetValid);
    }
    if now_ms >= proof.expires_at_ms {
        return Err(IdentityProofError::ProofExpired);
    }
    Ok(())
}

fn approval_required(state: &str) -> Result<bool, IdentityProofError> {
    match state {
        "approved" => Ok(false),
        "pending" => Ok(true),
        _ => Err(IdentityProofError::UnknownApprovalState),
    }
}

fn valid_binding(device: &DeviceIdentityBinding) -> bool {
    !device.device_id.is_empty()
        && !device.key_id.is_empty()
        && valid_digest(&device.public_key_sha256)
}

fn valid_proof(proof: &DeviceIdentityProof) -> bool {
    !proof.device_id.is_empty()
        && !proof.key_id.is_empty()
        && valid_digest(&proof.public_key_sha256)
        && !proof.challenge_id.is_empty()
        && valid_digest(&proof.challenge_sha256)
        && valid_digest(&proof.proof_sha256)
        && proof.issued_at_ms < proof.expires_at_ms
}

fn valid_challenge(challenge: &IdentityChallenge) -> bool {
    !challenge.challenge_id.is_empty()
        && valid_digest(&challenge.challenge_sha256)
        && challenge.issued_at_ms < challenge.expires_at_ms
}

fn valid_digest(value: &str) -> bool {
    value.len() == DIGEST_HEX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
