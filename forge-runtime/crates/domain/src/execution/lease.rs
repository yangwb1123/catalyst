//! Pure lease and fencing values for a future remote execution adapter.
//!
//! This module deliberately has no clock, storage, transport, Runner, or
//! scheduler dependency. A caller supplies observation time and proof values;
//! the state machine only checks their bounded relationship.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const EXECUTION_LEASE_ABI_VERSION: u16 = 1;
pub const MIN_EXECUTION_LEASE_TTL_MS: u64 = 1_000;
pub const MAX_EXECUTION_LEASE_TTL_MS: u64 = 600_000;
pub const EXECUTION_LEASE_CHECKPOINT_SCHEMA_VERSION: &str = "forge.execution-lease-checkpoint/v1";
pub const EXECUTION_LEASE_CHECKPOINT_EVALUATION_MODE: &str = "pure_execution_lease_checkpoint_only";
const MAX_LEASE_ID_BYTES: usize = 128;
const MAX_FENCING_TOKEN_BYTES: usize = 256;
const MAX_REASON_BYTES: usize = 256;

/// A bounded proof that a caller is presenting one exact lease incarnation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseProof {
    pub attempt_id: String,
    pub target_id: String,
    pub epoch: u64,
    pub fencing_token: String,
}

impl LeaseProof {
    pub(crate) fn validate_shape(&self) -> Result<(), LeaseError> {
        validate_identity(&self.attempt_id)?;
        validate_identity(&self.target_id)?;
        validate_token(&self.fencing_token)
    }
}

/// A coordinator-issued lease declaration. It is not durable authority until
/// an external store adopts the same fields atomically.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseGrant {
    pub v: u16,
    pub attempt_id: String,
    pub target_id: String,
    pub epoch: u64,
    pub fencing_token: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

impl LeaseGrant {
    /// Issues one bounded lease with the supplied server-observed time.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identities/tokens, a zero epoch, an
    /// invalid TTL, or arithmetic overflow.
    pub fn issue(
        attempt_id: String,
        target_id: String,
        epoch: u64,
        fencing_token: String,
        issued_at_ms: u64,
        ttl_ms: u64,
    ) -> Result<Self, LeaseError> {
        validate_identity(&attempt_id)?;
        validate_identity(&target_id)?;
        validate_token(&fencing_token)?;
        if epoch == 0 {
            return Err(LeaseError::InvalidEpoch);
        }
        validate_ttl(ttl_ms)?;
        let expires_at_ms = issued_at_ms
            .checked_add(ttl_ms)
            .ok_or(LeaseError::TimeOverflow)?;
        let grant = Self {
            v: EXECUTION_LEASE_ABI_VERSION,
            attempt_id,
            target_id,
            epoch,
            fencing_token,
            issued_at_ms,
            expires_at_ms,
        };
        grant.validate()?;
        Ok(grant)
    }

    /// Validates a decoded or caller-supplied grant before it is adopted.
    ///
    /// # Errors
    ///
    /// Returns an error when the ABI version, identity, epoch, time window, or
    /// bounded TTL is invalid.
    pub fn validate(&self) -> Result<(), LeaseError> {
        if self.v != EXECUTION_LEASE_ABI_VERSION {
            return Err(LeaseError::UnsupportedVersion);
        }
        validate_identity(&self.attempt_id)?;
        validate_identity(&self.target_id)?;
        validate_token(&self.fencing_token)?;
        if self.epoch == 0 {
            return Err(LeaseError::InvalidEpoch);
        }
        if self.expires_at_ms <= self.issued_at_ms {
            return Err(LeaseError::InvalidLeaseWindow);
        }
        validate_ttl(self.expires_at_ms - self.issued_at_ms)
    }

    /// Returns whether the lease is valid at the caller-supplied time.
    #[must_use]
    pub const fn is_active(&self, observed_at_ms: u64) -> bool {
        observed_at_ms >= self.issued_at_ms && observed_at_ms < self.expires_at_ms
    }

    /// Returns the proof fields needed for a terminal submission.
    #[must_use]
    pub fn proof(&self) -> LeaseProof {
        LeaseProof {
            attempt_id: self.attempt_id.clone(),
            target_id: self.target_id.clone(),
            epoch: self.epoch,
            fencing_token: self.fencing_token.clone(),
        }
    }

    /// Checks one proof against this lease and the supplied observation time.
    ///
    /// # Errors
    ///
    /// Returns an identity, epoch, token, time, or expiry error when the proof
    /// cannot fence this lease.
    pub fn validate_proof(
        &self,
        proof: &LeaseProof,
        observed_at_ms: u64,
    ) -> Result<(), LeaseError> {
        if proof.attempt_id != self.attempt_id {
            return Err(LeaseError::AttemptMismatch);
        }
        if proof.target_id != self.target_id {
            return Err(LeaseError::TargetMismatch);
        }
        if proof.epoch != self.epoch {
            return Err(LeaseError::EpochMismatch);
        }
        if proof.fencing_token != self.fencing_token {
            return Err(LeaseError::FencingTokenMismatch);
        }
        if observed_at_ms < self.issued_at_ms {
            return Err(LeaseError::TimeWentBackwards);
        }
        if !self.is_active(observed_at_ms) {
            return Err(LeaseError::LeaseExpired);
        }
        Ok(())
    }

    /// Produces the next fenced lease before the current lease expires.
    ///
    /// # Errors
    ///
    /// Returns an error when the old lease is expired, time regresses, the new
    /// token is reused, the TTL is outside the bounded range, or time overflows.
    pub fn renew(
        &self,
        observed_at_ms: u64,
        fencing_token: String,
        ttl_ms: u64,
    ) -> Result<Self, LeaseError> {
        if observed_at_ms < self.issued_at_ms {
            return Err(LeaseError::TimeWentBackwards);
        }
        if !self.is_active(observed_at_ms) {
            return Err(LeaseError::LeaseExpired);
        }
        if fencing_token == self.fencing_token {
            return Err(LeaseError::FencingTokenReused);
        }
        let epoch = self.epoch.checked_add(1).ok_or(LeaseError::EpochOverflow)?;
        Self::issue(
            self.attempt_id.clone(),
            self.target_id.clone(),
            epoch,
            fencing_token,
            observed_at_ms,
            ttl_ms,
        )
    }
}

/// A bounded terminal disposition. `Uncertain` is intentionally terminal for
/// this in-memory model and never implies an automatic retry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TerminalDisposition {
    Completed { receipt_sha256: String },
    Failed { reason: String },
    Uncertain { reason: String },
}

impl TerminalDisposition {
    fn validate(&self) -> Result<(), LeaseError> {
        match self {
            Self::Completed { receipt_sha256 } => validate_digest(receipt_sha256),
            Self::Failed { reason } | Self::Uncertain { reason } => validate_reason(reason),
        }
    }

    /// Returns true when the outcome is effect-uncertain and must be
    /// reconciled explicitly rather than retried automatically.
    #[must_use]
    pub const fn is_uncertain(&self) -> bool {
        matches!(self, Self::Uncertain { .. })
    }
}

/// The immutable receipt emitted by one accepted terminal submission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalReceipt {
    pub v: u16,
    pub proof: LeaseProof,
    pub disposition: TerminalDisposition,
    pub observed_at_ms: u64,
}

/// A restart-safe value image of one [`LeaseState`]. It is not a durable
/// store record and confers no execution authority when decoded.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseCheckpoint {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub grant: LeaseGrant,
    pub terminal: Option<TerminalReceipt>,
}

/// Result of a terminal submission. A replay returns the original receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSubmission {
    pub receipt: TerminalReceipt,
    pub replayed: bool,
}

/// In-memory lease holder used only to exercise fencing semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseState {
    grant: LeaseGrant,
    terminal: Option<TerminalReceipt>,
}

impl LeaseState {
    /// Creates a nonterminal state from one valid grant.
    ///
    /// # Errors
    ///
    /// Returns an error when the decoded grant is outside the lease ABI.
    pub fn new(grant: LeaseGrant) -> Result<Self, LeaseError> {
        grant.validate()?;
        Ok(Self {
            grant,
            terminal: None,
        })
    }

    /// Returns the current grant, not a persisted store observation.
    #[must_use]
    pub const fn grant(&self) -> &LeaseGrant {
        &self.grant
    }

    /// Returns the terminal receipt, when this state has one.
    #[must_use]
    pub const fn terminal(&self) -> Option<&TerminalReceipt> {
        self.terminal.as_ref()
    }

    /// Returns a defensive value image suitable for a caller-owned restart
    /// boundary. No storage or clock operation occurs.
    #[must_use]
    pub fn checkpoint(&self) -> LeaseCheckpoint {
        LeaseCheckpoint {
            schema_version: EXECUTION_LEASE_CHECKPOINT_SCHEMA_VERSION.into(),
            evaluation_mode: EXECUTION_LEASE_CHECKPOINT_EVALUATION_MODE.into(),
            grant: self.grant.clone(),
            terminal: self.terminal.clone(),
        }
    }

    /// Restores a lease state from a validated value image. Terminal evidence
    /// must match the exact proof and have been accepted within the grant's
    /// active window.
    pub fn from_checkpoint(checkpoint: LeaseCheckpoint) -> Result<Self, LeaseError> {
        if checkpoint.schema_version != EXECUTION_LEASE_CHECKPOINT_SCHEMA_VERSION
            || checkpoint.evaluation_mode != EXECUTION_LEASE_CHECKPOINT_EVALUATION_MODE
        {
            return Err(LeaseError::InvalidCheckpoint);
        }
        let mut state =
            Self::new(checkpoint.grant.clone()).map_err(|_| LeaseError::InvalidCheckpoint)?;
        if let Some(receipt) = checkpoint.terminal {
            if receipt.v != EXECUTION_LEASE_ABI_VERSION
                || receipt.proof != checkpoint.grant.proof()
                || receipt.disposition.validate().is_err()
                || checkpoint
                    .grant
                    .validate_proof(&receipt.proof, receipt.observed_at_ms)
                    .is_err()
            {
                return Err(LeaseError::InvalidCheckpoint);
            }
            state.terminal = Some(receipt);
        }
        Ok(state)
    }

    /// Rotates the fencing epoch before terminalization.
    ///
    /// # Errors
    ///
    /// Returns `TerminalAlreadyRecorded` after a terminal receipt exists, or
    /// any validation error returned by [`LeaseGrant::renew`].
    pub fn renew(
        &mut self,
        observed_at_ms: u64,
        fencing_token: String,
        ttl_ms: u64,
    ) -> Result<(), LeaseError> {
        if self.terminal.is_some() {
            return Err(LeaseError::TerminalAlreadyRecorded);
        }
        self.grant = self.grant.renew(observed_at_ms, fencing_token, ttl_ms)?;
        Ok(())
    }

    /// Accepts one fenced terminal result or replays the exact original.
    ///
    /// # Errors
    ///
    /// Returns a fencing/expiry error for stale proofs, or a terminal conflict
    /// when a different result is submitted after terminalization.
    pub fn submit_terminal(
        &mut self,
        proof: LeaseProof,
        disposition: TerminalDisposition,
        observed_at_ms: u64,
    ) -> Result<TerminalSubmission, LeaseError> {
        disposition.validate()?;
        if let Some(existing) = &self.terminal {
            if existing.proof == proof && existing.disposition == disposition {
                return Ok(TerminalSubmission {
                    receipt: existing.clone(),
                    replayed: true,
                });
            }
            return Err(LeaseError::TerminalAlreadyRecorded);
        }
        self.grant.validate_proof(&proof, observed_at_ms)?;
        let candidate = TerminalReceipt {
            v: EXECUTION_LEASE_ABI_VERSION,
            proof,
            disposition,
            observed_at_ms,
        };
        self.terminal = Some(candidate.clone());
        Ok(TerminalSubmission {
            receipt: candidate,
            replayed: false,
        })
    }
}

/// Errors produced by the pure lease/fencing checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeaseError {
    UnsupportedVersion,
    InvalidIdentity,
    InvalidFencingToken,
    InvalidEpoch,
    InvalidLeaseDuration,
    InvalidLeaseWindow,
    InvalidDigest,
    InvalidReason,
    TimeOverflow,
    EpochOverflow,
    TimeWentBackwards,
    LeaseExpired,
    AttemptMismatch,
    TargetMismatch,
    EpochMismatch,
    FencingTokenMismatch,
    FencingTokenReused,
    TerminalAlreadyRecorded,
    InvalidCheckpoint,
}

impl LeaseError {
    /// Returns the stable machine-readable rejection code shared with Go.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::InvalidIdentity => "invalid_identity",
            Self::InvalidFencingToken => "invalid_fencing_token",
            Self::InvalidEpoch => "invalid_epoch",
            Self::InvalidLeaseDuration => "invalid_lease_duration",
            Self::InvalidLeaseWindow => "invalid_lease_window",
            Self::InvalidDigest => "invalid_digest",
            Self::InvalidReason => "invalid_reason",
            Self::TimeOverflow => "time_overflow",
            Self::EpochOverflow => "epoch_overflow",
            Self::TimeWentBackwards => "time_went_backwards",
            Self::LeaseExpired => "lease_expired",
            Self::AttemptMismatch => "attempt_mismatch",
            Self::TargetMismatch => "target_mismatch",
            Self::EpochMismatch => "epoch_mismatch",
            Self::FencingTokenMismatch => "fencing_token_mismatch",
            Self::FencingTokenReused => "fencing_token_reused",
            Self::TerminalAlreadyRecorded => "terminal_already_recorded",
            Self::InvalidCheckpoint => "invalid_checkpoint",
        }
    }
}

impl fmt::Display for LeaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LeaseError {}

fn validate_identity(value: &str) -> Result<(), LeaseError> {
    if value.is_empty()
        || value.len() > MAX_LEASE_ID_BYTES
        || value.trim() != value
        || value.contains(['\0', '\r', '\n'])
    {
        return Err(LeaseError::InvalidIdentity);
    }
    Ok(())
}

fn validate_token(value: &str) -> Result<(), LeaseError> {
    if value.is_empty()
        || value.len() > MAX_FENCING_TOKEN_BYTES
        || value.trim() != value
        || value.contains(['\0', '\r', '\n'])
    {
        return Err(LeaseError::InvalidFencingToken);
    }
    Ok(())
}

fn validate_ttl(ttl_ms: u64) -> Result<(), LeaseError> {
    if !(MIN_EXECUTION_LEASE_TTL_MS..=MAX_EXECUTION_LEASE_TTL_MS).contains(&ttl_ms) {
        return Err(LeaseError::InvalidLeaseDuration);
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), LeaseError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(LeaseError::InvalidDigest);
    }
    Ok(())
}

fn validate_reason(value: &str) -> Result<(), LeaseError> {
    if value.is_empty() || value.len() > MAX_REASON_BYTES || value.contains('\0') {
        return Err(LeaseError::InvalidReason);
    }
    Ok(())
}

#[cfg(test)]
#[path = "lease_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "lease_contract_fixture.rs"]
mod contract_fixture;
