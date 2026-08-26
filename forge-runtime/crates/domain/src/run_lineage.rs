use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::{BoundRunPrompt, RunRecord, RuntimeEvent};

pub const RUN_LINEAGE_VERSION: u16 = 1;
pub const ROOT_INPUT_SOURCE_EVENT_SEQ: u64 = 1;
const SOURCE_EVENT_DIGEST_DOMAIN: &[u8] = b"forgeos.project-run.branch-source-event.v1\0";
const LINEAGE_DIGEST_DOMAIN: &[u8] = b"forgeos.project-run.branch-lineage.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunBranchMode {
    RootInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginRunBranch {
    pub v: u16,
    pub child_run_id: String,
    pub parent_run_id: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RunLineageRecord {
    pub v: u16,
    pub child_run_id: String,
    pub parent_run_id: String,
    pub branch_mode: RunBranchMode,
    pub source_event_seq: u64,
    pub source_event_sha256: String,
    pub lineage_sha256: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginRunBranchResult {
    pub disposition: crate::BeginRunDisposition,
    pub run: RunRecord,
    pub prompt: BoundRunPrompt,
    pub lineage: RunLineageRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunLineageError {
    pub message: String,
}

impl RunLineageRecord {
    /// Validates the closed root-input lineage record and its identity digest.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported versions, malformed identifiers or
    /// digests, self-parenting, or a divergent lineage digest.
    pub fn validate(&self) -> Result<(), RunLineageError> {
        if self.v != RUN_LINEAGE_VERSION
            || self.source_event_seq != ROOT_INPUT_SOURCE_EVENT_SEQ
            || self.branch_mode != RunBranchMode::RootInput
        {
            return Err(invalid("unsupported Run lineage contract"));
        }
        if !valid_id(&self.child_run_id)
            || !valid_id(&self.parent_run_id)
            || self.child_run_id == self.parent_run_id
            || !is_sha256(&self.source_event_sha256)
            || !is_sha256(&self.lineage_sha256)
        {
            return Err(invalid("invalid Run lineage fields"));
        }
        if self.lineage_sha256 != expected_lineage_sha256(self) {
            return Err(invalid("Run lineage digest does not match its fields"));
        }
        Ok(())
    }
}

/// Binds the exact source Run envelope and its root input event.
///
/// # Errors
///
/// Returns an error only if the typed values cannot be encoded as JSON.
pub fn source_event_sha256(
    run: &RunRecord,
    event: &RuntimeEvent,
) -> Result<String, RunLineageError> {
    let run_bytes = serde_json::to_vec(run).map_err(|error| encoding_error(&error))?;
    let event_bytes = serde_json::to_vec(event).map_err(|error| encoding_error(&error))?;
    let mut digest = Sha256::new();
    digest.update(SOURCE_EVENT_DIGEST_DOMAIN);
    update_framed(&mut digest, &run_bytes);
    update_framed(&mut digest, &event_bytes);
    Ok(lower_hex(&digest.finalize()))
}

#[must_use]
pub fn expected_lineage_sha256(record: &RunLineageRecord) -> String {
    let mut digest = Sha256::new();
    digest.update(LINEAGE_DIGEST_DOMAIN);
    for value in [
        record.child_run_id.as_bytes(),
        record.parent_run_id.as_bytes(),
        b"root_input",
        record.source_event_sha256.as_bytes(),
    ] {
        update_framed(&mut digest, value);
    }
    digest.update(record.source_event_seq.to_be_bytes());
    digest.update(record.created_at_ms.to_be_bytes());
    lower_hex(&digest.finalize())
}

fn update_framed(digest: &mut Sha256, value: &[u8]) {
    digest.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    digest.update(value);
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn invalid(message: &str) -> RunLineageError {
    RunLineageError {
        message: message.into(),
    }
}

fn encoding_error(error: &serde_json::Error) -> RunLineageError {
    invalid(&format!("Run lineage encoding failed: {error}"))
}

impl std::fmt::Display for RunLineageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RunLineageError {}
