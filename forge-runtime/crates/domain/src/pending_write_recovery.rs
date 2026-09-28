//! Pure metadata used to reconcile an uncertain shared-session write.
//!
//! The metadata deliberately excludes Prompt, title, and scope bytes. This
//! module does not read a clock, persist state, contact a service, create a
//! Run, or grant a retry authority; it only derives the same-key guard that a
//! caller can use before an explicitly chosen retry.

use serde::Serialize;

pub const PENDING_WRITE_RECOVERY_SCHEMA_VERSION: &str = "forge.pending-write-recovery/v1";
pub const PENDING_WRITE_RECOVERY_EVALUATION_MODE: &str = "pure_metadata_projection";
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PendingWriteMetadata {
    pub operation: String,
    pub conversation_id: Option<String>,
    pub expected_version: Option<u64>,
    pub idempotency_key: String,
    pub state: String,
    pub attempted_at_ms: Option<u64>,
    pub last_observed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The shared recovery projection exposes separately named retry and uncertainty flags."
)]
pub struct PendingWriteRecoveryProjection {
    pub schema_version: &'static str,
    pub evaluation_mode: &'static str,
    pub operation: String,
    pub conversation_id: Option<String>,
    pub expected_version: Option<u64>,
    pub idempotency_key: String,
    pub state: String,
    pub pending: bool,
    pub unconfirmed: bool,
    pub retry_allowed: bool,
    pub same_key_required: bool,
    pub reconcile_before_retry: bool,
    pub attempted_at_ms: Option<u64>,
    pub last_observed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PendingWriteRecoveryError {
    InvalidOperation,
    ConversationIdRequired,
    ExpectedVersionRequired,
    ExpectedVersionForbidden,
    InvalidIdempotencyKey,
    InvalidState,
    ObservationTimeRegressed,
}

impl std::fmt::Display for PendingWriteRecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PendingWriteRecoveryError {}

impl PendingWriteRecoveryError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidOperation => "invalid_operation",
            Self::ConversationIdRequired => "conversation_id_required",
            Self::ExpectedVersionRequired => "expected_version_required",
            Self::ExpectedVersionForbidden => "expected_version_forbidden",
            Self::InvalidIdempotencyKey => "invalid_idempotency_key",
            Self::InvalidState => "invalid_state",
            Self::ObservationTimeRegressed => "observation_time_regressed",
        }
    }
}

/// Projects caller-supplied recovery metadata into a stable retry guard.
///
/// # Errors
///
/// Returns an error when operation-specific identifiers, state, key, or
/// caller-supplied observation times do not satisfy the contract.
pub fn project_pending_write(
    metadata: PendingWriteMetadata,
) -> Result<PendingWriteRecoveryProjection, PendingWriteRecoveryError> {
    validate_metadata(&metadata)?;
    let unconfirmed = metadata.state == "unconfirmed";
    Ok(PendingWriteRecoveryProjection {
        schema_version: PENDING_WRITE_RECOVERY_SCHEMA_VERSION,
        evaluation_mode: PENDING_WRITE_RECOVERY_EVALUATION_MODE,
        operation: metadata.operation,
        conversation_id: metadata.conversation_id,
        expected_version: metadata.expected_version,
        idempotency_key: metadata.idempotency_key,
        state: metadata.state,
        pending: true,
        unconfirmed,
        retry_allowed: true,
        same_key_required: true,
        reconcile_before_retry: unconfirmed,
        attempted_at_ms: metadata.attempted_at_ms,
        last_observed_at_ms: metadata.last_observed_at_ms,
    })
}

fn validate_metadata(metadata: &PendingWriteMetadata) -> Result<(), PendingWriteRecoveryError> {
    if !matches!(
        metadata.operation.as_str(),
        "append_prompt" | "create_conversation"
    ) {
        return Err(PendingWriteRecoveryError::InvalidOperation);
    }
    if !matches!(metadata.state.as_str(), "pending" | "unconfirmed") {
        return Err(PendingWriteRecoveryError::InvalidState);
    }
    if !valid_key(&metadata.idempotency_key) {
        return Err(PendingWriteRecoveryError::InvalidIdempotencyKey);
    }
    validate_operation_binding(metadata)?;
    if metadata
        .last_observed_at_ms
        .zip(metadata.attempted_at_ms)
        .is_some_and(|(observed, attempted)| observed < attempted)
    {
        return Err(PendingWriteRecoveryError::ObservationTimeRegressed);
    }
    Ok(())
}

fn validate_operation_binding(
    metadata: &PendingWriteMetadata,
) -> Result<(), PendingWriteRecoveryError> {
    match metadata.operation.as_str() {
        "append_prompt" => {
            if metadata
                .conversation_id
                .as_deref()
                .is_none_or(|value| !valid_identifier(value))
            {
                return Err(PendingWriteRecoveryError::ConversationIdRequired);
            }
            if metadata.expected_version.is_none() {
                return Err(PendingWriteRecoveryError::ExpectedVersionRequired);
            }
        }
        "create_conversation" => {
            if metadata.conversation_id.is_some() {
                return Err(PendingWriteRecoveryError::ConversationIdRequired);
            }
            if metadata.expected_version.is_some() {
                return Err(PendingWriteRecoveryError::ExpectedVersionForbidden);
            }
        }
        _ => unreachable!("operation was checked above"),
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_IDENTIFIER_BYTES
        && value.trim() == value
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().skip(1).all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-')
        })
}

fn valid_key(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_IDEMPOTENCY_KEY_BYTES
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    const FIXTURE: &str =
        include_str!("../../../../docs/contracts/fixtures/forge-pending-write-recovery-v1.json");

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fixture {
        schema_version: String,
        evaluation_mode: String,
        authority: Authority,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Authority {
        body_included: bool,
        run_created: bool,
        network_contacted: bool,
        persistence_written: bool,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        name: String,
        metadata: MetadataFixture,
        expected: Expected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct MetadataFixture {
        operation: String,
        conversation_id: Option<String>,
        expected_version: Option<u64>,
        idempotency_key: String,
        state: String,
        attempted_at_ms: Option<u64>,
        last_observed_at_ms: Option<u64>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        accepted: bool,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        operation: Option<String>,
        #[serde(default)]
        conversation_id: Option<String>,
        #[serde(default)]
        expected_version: Option<u64>,
        #[serde(default)]
        idempotency_key: Option<String>,
        #[serde(default)]
        state: Option<String>,
        #[serde(default)]
        pending: bool,
        #[serde(default)]
        unconfirmed: bool,
        #[serde(default)]
        retry_allowed: bool,
        #[serde(default)]
        same_key_required: bool,
        #[serde(default)]
        reconcile_before_retry: bool,
        #[serde(default)]
        attempted_at_ms: Option<u64>,
        #[serde(default)]
        last_observed_at_ms: Option<u64>,
    }

    #[test]
    fn pending_write_recovery_contract_fixture() {
        let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(
            fixture.schema_version,
            PENDING_WRITE_RECOVERY_SCHEMA_VERSION
        );
        assert_eq!(
            fixture.evaluation_mode,
            PENDING_WRITE_RECOVERY_EVALUATION_MODE
        );
        assert!(!fixture.authority.body_included);
        assert!(!fixture.authority.run_created);
        assert!(!fixture.authority.network_contacted);
        assert!(!fixture.authority.persistence_written);
        assert_eq!(fixture.cases.len(), 6);
        for case in fixture.cases {
            let metadata = PendingWriteMetadata {
                operation: case.metadata.operation,
                conversation_id: case.metadata.conversation_id,
                expected_version: case.metadata.expected_version,
                idempotency_key: case.metadata.idempotency_key,
                state: case.metadata.state,
                attempted_at_ms: case.metadata.attempted_at_ms,
                last_observed_at_ms: case.metadata.last_observed_at_ms,
            };
            let result = project_pending_write(metadata);
            assert_case(&case.name, &case.expected, result);
        }
    }

    fn assert_case(
        name: &str,
        expected: &Expected,
        result: Result<PendingWriteRecoveryProjection, PendingWriteRecoveryError>,
    ) {
        if !expected.accepted {
            let error = result.expect_err(name);
            assert_eq!(error.code(), expected.error.as_deref().unwrap(), "{name}");
            return;
        }
        let actual = result.unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            actual.operation,
            expected.operation.clone().unwrap(),
            "{name}"
        );
        assert_eq!(actual.conversation_id, expected.conversation_id, "{name}");
        assert_eq!(actual.expected_version, expected.expected_version, "{name}");
        assert_eq!(
            actual.idempotency_key,
            expected.idempotency_key.clone().unwrap(),
            "{name}"
        );
        assert_eq!(actual.state, expected.state.clone().unwrap(), "{name}");
        assert_eq!(actual.pending, expected.pending, "{name}");
        assert_eq!(actual.unconfirmed, expected.unconfirmed, "{name}");
        assert_eq!(actual.retry_allowed, expected.retry_allowed, "{name}");
        assert_eq!(
            actual.same_key_required, expected.same_key_required,
            "{name}"
        );
        assert_eq!(
            actual.reconcile_before_retry, expected.reconcile_before_retry,
            "{name}"
        );
        assert_eq!(actual.attempted_at_ms, expected.attempted_at_ms, "{name}");
        assert_eq!(
            actual.last_observed_at_ms, expected.last_observed_at_ms,
            "{name}"
        );
    }
}
