//! Pure bounded command and terminal-receipt values for a future Runner.
//!
//! This module describes a direct-argv command envelope and the evidence shape
//! that can close it. It does not execute a process, read a clock, persist a
//! receipt, open a transport, or grant execution authority. The caller must
//! provide the already-issued lease and the observed terminal time.

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::lease::{LeaseError, LeaseGrant, LeaseProof, LeaseState, TerminalDisposition};

/// Schema version for the metadata-only terminal receipt observation.
pub const RUNNER_TERMINAL_RECEIPT_SCHEMA_VERSION: &str = "forge.runner-command-terminal-receipt/v1";
pub const RUNNER_TERMINAL_RECEIPT_EVALUATION_MODE: &str = "pure_runner_command_receipt_only";

pub const RUNNER_COMMAND_ABI_VERSION: u16 = 1;
pub const RUNNER_COMMAND_DIGEST_DOMAIN: &[u8] = b"forge.runtime.runner-command.v1\0";
pub const MAX_RUNNER_COMMAND_ID_BYTES: usize = 128;
pub const MAX_RUNNER_COMMAND_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_RUNNER_COMMAND_WORKSPACE_REF_BYTES: usize = 256;
pub const MAX_RUNNER_COMMAND_ARGUMENTS: usize = 64;
pub const MAX_RUNNER_COMMAND_ARGUMENT_BYTES: usize = 4_096;
pub const MAX_RUNNER_COMMAND_ARGUMENT_TOTAL_BYTES: usize = 65_536;
pub const MAX_RUNNER_COMMAND_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

/// A direct-argv command bound to one exact lease incarnation.
///
/// `workspace_ref` is an opaque staged-workspace identity. It is intentionally
/// not a host path, and `argv` is never interpreted as a shell command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerCommand {
    pub v: u16,
    pub command_id: String,
    pub lease_proof: LeaseProof,
    pub idempotency_key: String,
    pub workspace_ref: String,
    pub argv: Vec<String>,
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
}

impl RunnerCommand {
    /// Validates the bounded command declaration without performing any effect.
    ///
    /// # Errors
    ///
    /// Returns a bounded-shape, lease-proof, argument, or resource-limit error.
    pub fn validate(&self) -> Result<(), RunnerCommandError> {
        if self.v != RUNNER_COMMAND_ABI_VERSION {
            return Err(RunnerCommandError::UnsupportedVersion);
        }
        self.validate_identity_fields()?;
        self.validate_arguments()?;
        if !(1..=super::lease::MAX_EXECUTION_LEASE_TTL_MS).contains(&self.timeout_ms) {
            return Err(RunnerCommandError::InvalidTimeout);
        }
        if !(1..=MAX_RUNNER_COMMAND_OUTPUT_BYTES).contains(&self.max_output_bytes) {
            return Err(RunnerCommandError::InvalidOutputLimit);
        }
        Ok(())
    }

    fn validate_identity_fields(&self) -> Result<(), RunnerCommandError> {
        validate_text(
            &self.command_id,
            MAX_RUNNER_COMMAND_ID_BYTES,
            false,
            RunnerCommandError::InvalidCommandID,
        )?;
        if self.command_id.trim() != self.command_id {
            return Err(RunnerCommandError::InvalidCommandID);
        }
        self.lease_proof
            .validate_shape()
            .map_err(RunnerCommandError::Lease)?;
        if self.lease_proof.attempt_id.is_empty() || self.lease_proof.target_id.is_empty() {
            return Err(RunnerCommandError::Lease(LeaseError::InvalidIdentity));
        }
        validate_text(
            &self.idempotency_key,
            MAX_RUNNER_COMMAND_IDEMPOTENCY_KEY_BYTES,
            false,
            RunnerCommandError::InvalidIdempotencyKey,
        )?;
        if self.idempotency_key.trim() != self.idempotency_key {
            return Err(RunnerCommandError::InvalidIdempotencyKey);
        }
        validate_text(
            &self.workspace_ref,
            MAX_RUNNER_COMMAND_WORKSPACE_REF_BYTES,
            false,
            RunnerCommandError::InvalidWorkspaceRef,
        )?;
        if self.workspace_ref.trim() != self.workspace_ref {
            return Err(RunnerCommandError::InvalidWorkspaceRef);
        }
        Ok(())
    }

    fn validate_arguments(&self) -> Result<(), RunnerCommandError> {
        if self.argv.is_empty() {
            return Err(RunnerCommandError::EmptyArgv);
        }
        if self.argv.len() > MAX_RUNNER_COMMAND_ARGUMENTS {
            return Err(RunnerCommandError::TooManyArguments);
        }
        let mut total_bytes = 0usize;
        for (index, argument) in self.argv.iter().enumerate() {
            validate_text(
                argument,
                MAX_RUNNER_COMMAND_ARGUMENT_BYTES,
                index != 0,
                RunnerCommandError::InvalidArgument,
            )?;
            total_bytes = total_bytes
                .checked_add(argument.len())
                .ok_or(RunnerCommandError::ArgumentsTooLarge)?;
        }
        if total_bytes > MAX_RUNNER_COMMAND_ARGUMENT_TOTAL_BYTES {
            return Err(RunnerCommandError::ArgumentsTooLarge);
        }
        Ok(())
    }

    /// Computes a domain-separated identity over the validated command bytes.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::validate`] or a JSON
    /// encoding error.
    pub fn command_sha256(&self) -> Result<String, RunnerCommandError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| RunnerCommandError::EncodingFailed)?;
        let mut digest = Sha256::new();
        digest.update(RUNNER_COMMAND_DIGEST_DOMAIN);
        digest.update(bytes);
        Ok(crate::governance_contract::codec::lower_hex(
            &digest.finalize(),
        ))
    }
}

/// The immutable terminal observation for one command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerTerminalReceipt {
    pub v: u16,
    pub command_id: String,
    pub command_sha256: String,
    pub proof: LeaseProof,
    pub disposition: TerminalDisposition,
    pub observed_at_ms: u64,
}

/// Authority is deliberately absent from a terminal receipt observation.
/// These values are caller-supplied evidence and never establish execution
/// authority, persistence, dispatch, or audit publication.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerTerminalReceiptAuthority {
    pub device_identity_verified: bool,
    pub command_persisted: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

/// A bounded, payload-free projection of one validated terminal receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerTerminalReceiptObservation {
    pub schema_version: &'static str,
    pub evaluation_mode: &'static str,
    pub command_id: String,
    pub command_sha256: String,
    pub attempt_id: String,
    pub target_id: String,
    pub disposition_kind: String,
    pub observed_at_ms: u64,
    pub receipt_valid: bool,
    pub preview_only: bool,
    pub uncertain: bool,
    pub reconciliation_required: bool,
    pub manual_review_required: bool,
    pub automatic_retry: bool,
    pub follow_up: String,
    pub authority: RunnerTerminalReceiptAuthority,
}

impl RunnerTerminalReceipt {
    /// Builds a receipt declaration from one command and caller-supplied result.
    ///
    /// # Errors
    ///
    /// Returns a command validation or encoding error when the command cannot
    /// be given a stable digest.
    pub fn from_command(
        command: &RunnerCommand,
        disposition: TerminalDisposition,
        observed_at_ms: u64,
    ) -> Result<Self, RunnerCommandError> {
        Ok(Self {
            v: RUNNER_COMMAND_ABI_VERSION,
            command_id: command.command_id.clone(),
            command_sha256: command.command_sha256()?,
            proof: command.lease_proof.clone(),
            disposition,
            observed_at_ms,
        })
    }

    /// Checks command identity, lease fencing, and terminal disposition.
    ///
    /// The supplied grant is an observation of the current lease; this method
    /// does not store it or change it.
    ///
    /// # Errors
    ///
    /// Returns a command identity, proof, disposition, or lease-fencing error.
    pub fn validate_against(
        &self,
        command: &RunnerCommand,
        grant: &LeaseGrant,
    ) -> Result<(), RunnerCommandError> {
        command.validate()?;
        if self.v != RUNNER_COMMAND_ABI_VERSION {
            return Err(RunnerCommandError::UnsupportedVersion);
        }
        if self.command_id != command.command_id {
            return Err(RunnerCommandError::CommandMismatch);
        }
        if self.command_sha256 != command.command_sha256()? {
            return Err(RunnerCommandError::CommandDigestMismatch);
        }
        if self.proof != command.lease_proof {
            return Err(RunnerCommandError::ProofMismatch);
        }
        let mut state = LeaseState::new(grant.clone()).map_err(RunnerCommandError::Lease)?;
        state
            .submit_terminal(
                self.proof.clone(),
                self.disposition.clone(),
                self.observed_at_ms,
            )
            .map_err(RunnerCommandError::Lease)?;
        Ok(())
    }
}

/// Validates and projects a caller-supplied terminal receipt without reading
/// a clock, persisting state, contacting a Runner, or granting authority.
pub fn observe_runner_terminal_receipt(
    command: &RunnerCommand,
    grant: &LeaseGrant,
    receipt: &RunnerTerminalReceipt,
) -> Result<RunnerTerminalReceiptObservation, RunnerCommandError> {
    receipt.validate_against(command, grant)?;
    let uncertain = receipt.disposition.is_uncertain();
    Ok(RunnerTerminalReceiptObservation {
        schema_version: RUNNER_TERMINAL_RECEIPT_SCHEMA_VERSION,
        evaluation_mode: RUNNER_TERMINAL_RECEIPT_EVALUATION_MODE,
        command_id: receipt.command_id.clone(),
        command_sha256: receipt.command_sha256.clone(),
        attempt_id: receipt.proof.attempt_id.clone(),
        target_id: receipt.proof.target_id.clone(),
        disposition_kind: match &receipt.disposition {
            TerminalDisposition::Completed { .. } => "completed".into(),
            TerminalDisposition::Failed { .. } => "failed".into(),
            TerminalDisposition::Uncertain { .. } => "uncertain".into(),
        },
        observed_at_ms: receipt.observed_at_ms,
        receipt_valid: true,
        preview_only: true,
        uncertain,
        reconciliation_required: uncertain,
        manual_review_required: uncertain,
        automatic_retry: false,
        follow_up: if uncertain {
            "reconciliation_manual".into()
        } else {
            "none".into()
        },
        authority: RunnerTerminalReceiptAuthority::default(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunnerCommandError {
    UnsupportedVersion,
    InvalidCommandID,
    InvalidIdempotencyKey,
    InvalidWorkspaceRef,
    EmptyArgv,
    TooManyArguments,
    InvalidArgument,
    ArgumentsTooLarge,
    InvalidTimeout,
    InvalidOutputLimit,
    EncodingFailed,
    CommandMismatch,
    CommandDigestMismatch,
    ProofMismatch,
    Lease(LeaseError),
}

impl fmt::Display for RunnerCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runner command rejected: {self:?}")
    }
}

impl std::error::Error for RunnerCommandError {}

fn validate_text(
    value: &str,
    maximum: usize,
    allow_empty: bool,
    error: RunnerCommandError,
) -> Result<(), RunnerCommandError> {
    if (!allow_empty && value.is_empty())
        || value.len() > maximum
        || value.chars().any(|character| {
            character <= '\u{001f}' || ('\u{007f}'..='\u{009f}').contains(&character)
        })
    {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
#[path = "runner_command_tests.rs"]
mod tests;
