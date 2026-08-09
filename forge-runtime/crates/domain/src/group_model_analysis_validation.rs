use super::{
    ClaimGroupModelAnalysisDispatch, CompleteGroupModelAnalysis,
    GROUP_MODEL_ANALYSIS_CONSENT_VERSION, GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
    GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT, GROUP_MODEL_ANALYSIS_RESULT_VERSION,
    GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION, GROUP_MODEL_ANALYSIS_VERSION,
    GroupModelAnalysisConfig, GroupModelAnalysisDispatchClaim, GroupModelAnalysisJournalError,
    GroupModelAnalysisOutcome, GroupModelAnalysisPreparedReceipt, GroupModelAnalysisRecord,
    GroupModelAnalysisRecovery, GroupModelAnalysisRequestConfig, GroupModelAnalysisResult,
    GroupModelAnalysisResultArtifact, GroupModelAnalysisResultReceipt, GroupModelAnalysisSource,
    GroupModelAnalysisStatus, MAX_GROUP_MODEL_ANALYSIS_CONFIG_JSON_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_ID_BYTES, MAX_GROUP_MODEL_ANALYSIS_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES, MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS,
    MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES, MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS,
    MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES, MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_BYTES, PrepareGroupModelAnalysis,
};
use crate::{GROUP_CONTEXT_VERSION, GROUP_RUN_VERSION, MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES};
use sha2::{Digest, Sha256};

impl GroupModelAnalysisConfig {
    /// Validates the durable, content-free provider configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported provider, destination, or limit.
    pub fn validate(&self) -> Result<(), GroupModelAnalysisJournalError> {
        validate_config(self)
    }
}

impl GroupModelAnalysisRequestConfig {
    /// Validates the private configuration used to construct the exact request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, system Prompt, or limits.
    pub fn validate(&self) -> Result<(), GroupModelAnalysisJournalError> {
        validate_request_config(self)
    }
}

impl GroupModelAnalysisSource {
    /// Validates the immutable prepared Group Run identity.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identifiers, versions, digests, or size.
    pub fn validate(&self) -> Result<(), GroupModelAnalysisJournalError> {
        validate_source(self)
    }
}

impl GroupModelAnalysisRecord {
    /// Validates durable metadata without trusting its journal.
    ///
    /// # Errors
    ///
    /// Returns an error when a field violates the versioned contract.
    pub fn validate(&self) -> Result<(), GroupModelAnalysisJournalError> {
        validate_record(self)
    }
}

impl PrepareGroupModelAnalysis {
    /// Validates one bounded private preparation candidate.
    ///
    /// Digest recomputation and canonical JSON checks remain store duties.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid source, configuration, body, key, or time.
    pub fn validate(&self) -> Result<(), GroupModelAnalysisJournalError> {
        validate_prepare(self)
    }
}

impl ClaimGroupModelAnalysisDispatch {
    /// Validates caller fields for one exclusive off-machine release.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid identity, consent version, or time.
    pub fn validate(&self) -> Result<(), GroupModelAnalysisJournalError> {
        validate_claim_request(self)
    }
}

impl GroupModelAnalysisResultArtifact {
    /// Validates the intrinsic bounds of a result artifact.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identity, digest, result, usage, or time.
    pub fn validate(&self) -> Result<(), GroupModelAnalysisJournalError> {
        validate_result_artifact(self)
    }
}

impl CompleteGroupModelAnalysis {
    /// Validates a completion request before store binding checks.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported version or invalid artifact.
    pub fn validate(&self) -> Result<(), GroupModelAnalysisJournalError> {
        if self.v != GROUP_MODEL_ANALYSIS_VERSION {
            return Err(analysis_error(
                "unsupported Group Model Analysis completion version",
            ));
        }
        validate_result_artifact(&self.artifact)
    }
}

fn validate_claim_request(
    request: &ClaimGroupModelAnalysisDispatch,
) -> Result<(), GroupModelAnalysisJournalError> {
    let valid = request.v == GROUP_MODEL_ANALYSIS_VERSION
        && valid_identifier(&request.analysis_id)
        && valid_identifier(&request.dispatch_id)
        && request.consent_version == GROUP_MODEL_ANALYSIS_CONSENT_VERSION
        && i64::try_from(request.released_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("invalid Group Model Analysis dispatch claim"))
}

fn validate_result_artifact(
    artifact: &GroupModelAnalysisResultArtifact,
) -> Result<(), GroupModelAnalysisJournalError> {
    let result = &artifact.result;
    let valid = validate_result(result)
        && is_lower_hex_digest(&artifact.result_sha256)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES).contains(&artifact.result_bytes)
        && i64::try_from(artifact.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("invalid Group Model Analysis result artifact"))
}

fn validate_result(result: &GroupModelAnalysisResult) -> bool {
    result.v == GROUP_MODEL_ANALYSIS_RESULT_VERSION
        && valid_identifier(&result.analysis_id)
        && valid_identifier(&result.dispatch_id)
        && is_lower_hex_digest(&result.request_sha256)
        && valid_outcome(result.outcome)
        && !result.answer.trim().is_empty()
        && result.answer.len() <= MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES
        && usage_is_storable(result.usage)
}

pub(super) fn validate_record(
    record: &GroupModelAnalysisRecord,
) -> Result<(), GroupModelAnalysisJournalError> {
    let valid = record.v == GROUP_MODEL_ANALYSIS_VERSION
        && valid_identifier(&record.analysis_id)
        && valid_identifier(&record.group_run_id)
        && is_lower_hex_digest(&record.source_snapshot_sha256)
        && validate_config(&record.config).is_ok()
        && is_lower_hex_digest(&record.config_sha256)
        && is_lower_hex_digest(&record.request_sha256)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES).contains(&record.request_bytes)
        && record.protocol_version == GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("invalid Group Model Analysis record"))
}

///  accepts the official endpoint, or any http(s) 
/// endpoint chosen by an explicit caller opt-in (): the
/// prepared dossier may only travel to a destination the operator pinned.
#[must_use]
pub fn endpoint_allowed(endpoint: &str) -> bool {
    endpoint == GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT
        || (endpoint.starts_with("http://") || endpoint.starts_with("https://"))
            && endpoint.ends_with("/v1/responses")
}

pub(super) fn validate_config(
    config: &GroupModelAnalysisConfig,
) -> Result<(), GroupModelAnalysisJournalError> {
    let valid = config.v == GROUP_MODEL_ANALYSIS_VERSION
        && matches!(
            config.provider,
            super::GroupModelAnalysisProvider::OpenAiResponses
        )
        && endpoint_allowed(&config.endpoint)
        && valid_text(&config.model, MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES)
        && config.system_prompt_version == GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION
        && is_lower_hex_digest(&config.system_prompt_sha256)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS).contains(&config.max_output_tokens)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES).contains(&config.max_model_output_bytes)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS).contains(&config.max_model_events);
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("invalid Group Model Analysis configuration"))
}

pub(super) fn validate_request_config(
    config: &GroupModelAnalysisRequestConfig,
) -> Result<(), GroupModelAnalysisJournalError> {
    let valid = config.v == GROUP_MODEL_ANALYSIS_VERSION
        && matches!(
            config.provider,
            super::GroupModelAnalysisProvider::OpenAiResponses
        )
        && endpoint_allowed(&config.endpoint)
        && valid_text(&config.model, MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES)
        && config.system_prompt_version == GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION
        && !config.system_prompt.trim().is_empty()
        && config.system_prompt.len() <= MAX_GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_BYTES
        && (1..=MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS).contains(&config.max_output_tokens)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES).contains(&config.max_model_output_bytes)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS).contains(&config.max_model_events);
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("invalid Group Model Analysis request configuration"))
}

pub(super) fn validate_source(
    source: &GroupModelAnalysisSource,
) -> Result<(), GroupModelAnalysisJournalError> {
    let valid = source.group_run_version == GROUP_RUN_VERSION
        && valid_identifier(&source.group_run_id)
        && valid_identifier(&source.group_id)
        && source.context_version == GROUP_CONTEXT_VERSION
        && is_lower_hex_digest(&source.context_slice_sha256)
        && is_lower_hex_digest(&source.snapshot_sha256)
        && (1..=MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES).contains(&source.snapshot_bytes);
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("invalid Group Model Analysis source"))
}

pub(super) fn validate_prepare(
    request: &PrepareGroupModelAnalysis,
) -> Result<(), GroupModelAnalysisJournalError> {
    validate_source(&request.source)?;
    validate_request_config(&request.request_config)?;
    validate_config(&request.config)?;
    let same_config = request.config.v == request.request_config.v
        && request.config.provider == request.request_config.provider
        && request.config.endpoint == request.request_config.endpoint
        && request.config.model == request.request_config.model
        && request.config.system_prompt_version == request.request_config.system_prompt_version
        && request.config.max_output_tokens == request.request_config.max_output_tokens
        && request.config.max_model_output_bytes == request.request_config.max_model_output_bytes
        && request.config.max_model_events == request.request_config.max_model_events;
    let valid = request.v == GROUP_MODEL_ANALYSIS_VERSION
        && valid_identifier(&request.analysis_id)
        && same_config
        && (1..=MAX_GROUP_MODEL_ANALYSIS_CONFIG_JSON_BYTES).contains(&request.config_json.len())
        && is_lower_hex_digest(&request.config_sha256)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES).contains(&request.request_body.len())
        && is_lower_hex_digest(&request.request_sha256)
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_MODEL_ANALYSIS_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("invalid Group Model Analysis preparation"))
}

pub(super) fn validate_prepared(
    record: &GroupModelAnalysisRecord,
    receipt: &GroupModelAnalysisPreparedReceipt,
) -> Result<(), GroupModelAnalysisJournalError> {
    validate_record(record)?;
    validate_source(&receipt.source)?;
    let source = &receipt.source;
    let source_valid = source.group_run_id == record.group_run_id
        && source.snapshot_sha256 == record.source_snapshot_sha256
        && source.snapshot_bytes <= MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES;
    let binding = receipt.v == GROUP_MODEL_ANALYSIS_VERSION
        && receipt.analysis_id == record.analysis_id
        && receipt.config_sha256 == record.config_sha256
        && receipt.request_sha256 == record.request_sha256
        && receipt.request_bytes == record.request_bytes;
    (source_valid && binding)
        .then_some(())
        .ok_or_else(|| analysis_error("prepared receipt does not match its analysis"))
}

pub(super) fn validate_claim(
    record: &GroupModelAnalysisRecord,
    claim: &GroupModelAnalysisDispatchClaim,
) -> Result<(), GroupModelAnalysisJournalError> {
    validate_record(record)?;
    let valid = claim.v == GROUP_MODEL_ANALYSIS_VERSION
        && claim.analysis_id == record.analysis_id
        && valid_identifier(&claim.dispatch_id)
        && claim.request_sha256 == record.request_sha256
        && claim.config_sha256 == record.config_sha256
        && claim.provider == record.config.provider
        && claim.endpoint == record.config.endpoint
        && claim.model == record.config.model
        && claim.consent_version == GROUP_MODEL_ANALYSIS_CONSENT_VERSION
        && i64::try_from(claim.released_at_ms).is_ok()
        && claim.released_at_ms >= record.created_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("dispatch claim does not match its prepared analysis"))
}

pub(super) fn validate_artifact(
    record: &GroupModelAnalysisRecord,
    claim: &GroupModelAnalysisDispatchClaim,
    artifact: &GroupModelAnalysisResultArtifact,
) -> Result<(), GroupModelAnalysisJournalError> {
    validate_record(record)?;
    validate_claim(record, claim)?;
    validate_result_artifact(artifact)?;
    let result = &artifact.result;
    let valid = result.analysis_id == record.analysis_id
        && result.dispatch_id == claim.dispatch_id
        && result.request_sha256 == record.request_sha256
        && result.answer.len() <= record.config.max_model_output_bytes
        && artifact.created_at_ms >= claim.released_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("invalid Group Model Analysis result artifact"))
}

pub(super) fn validate_completion(
    record: &GroupModelAnalysisRecord,
    claim: &GroupModelAnalysisDispatchClaim,
    receipt: &GroupModelAnalysisResultReceipt,
    artifact: &GroupModelAnalysisResultArtifact,
) -> Result<(), GroupModelAnalysisJournalError> {
    validate_artifact(record, claim, artifact)?;
    validate_completion_receipt(record, claim, receipt)?;
    let result = &artifact.result;
    let valid = receipt.outcome == result.outcome
        && receipt.result_sha256 == artifact.result_sha256
        && receipt.result_bytes == artifact.result_bytes
        && receipt.usage == result.usage
        && receipt.created_at_ms == artifact.created_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("completion receipt does not match its result artifact"))
}

pub(super) fn validate_completion_receipt(
    record: &GroupModelAnalysisRecord,
    claim: &GroupModelAnalysisDispatchClaim,
    receipt: &GroupModelAnalysisResultReceipt,
) -> Result<(), GroupModelAnalysisJournalError> {
    validate_record(record)?;
    validate_claim(record, claim)?;
    let valid = receipt.v == GROUP_MODEL_ANALYSIS_RESULT_VERSION
        && receipt.analysis_id == record.analysis_id
        && receipt.dispatch_id == claim.dispatch_id
        && receipt.request_sha256 == record.request_sha256
        && valid_outcome(receipt.outcome)
        && is_lower_hex_digest(&receipt.result_sha256)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES).contains(&receipt.result_bytes)
        && usage_is_storable(receipt.usage)
        && i64::try_from(receipt.created_at_ms).is_ok()
        && receipt.created_at_ms >= claim.released_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("completion receipt does not match its dispatch"))
}

pub(super) fn validate_status(
    status: GroupModelAnalysisStatus,
    recovery: &GroupModelAnalysisRecovery,
) -> Result<(), GroupModelAnalysisJournalError> {
    let valid = matches!(
        (status, recovery),
        (
            GroupModelAnalysisStatus::AwaitingConsent,
            GroupModelAnalysisRecovery::Unprepared | GroupModelAnalysisRecovery::AwaitingConsent
        ) | (
            GroupModelAnalysisStatus::DispatchUnknown,
            GroupModelAnalysisRecovery::DispatchUnknown { .. }
        ) | (
            GroupModelAnalysisStatus::Completed,
            GroupModelAnalysisRecovery::Terminal { .. }
        )
    );
    valid
        .then_some(())
        .ok_or_else(|| analysis_error("analysis status disagrees with its journal"))
}

pub(super) fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES)
}

pub(super) fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(unsupported_character)
}

pub(super) fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn request_digest_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(super::GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

pub(super) fn valid_outcome(outcome: GroupModelAnalysisOutcome) -> bool {
    matches!(
        outcome,
        GroupModelAnalysisOutcome::Completed | GroupModelAnalysisOutcome::Length
    )
}

pub(super) fn analysis_error(message: &str) -> GroupModelAnalysisJournalError {
    GroupModelAnalysisJournalError {
        message: message.into(),
    }
}

fn unsupported_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn usage_is_storable(usage: crate::Usage) -> bool {
    i64::try_from(usage.input_tokens).is_ok() && i64::try_from(usage.output_tokens).is_ok()
}
