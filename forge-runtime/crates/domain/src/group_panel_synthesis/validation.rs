use sha2::{Digest, Sha256};

use super::{
    ClaimGroupPanelSynthesisDispatch, CompleteGroupPanelSynthesis,
    GROUP_PANEL_SYNTHESIS_CONSENT_VERSION, GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
    GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT, GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
    GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION, GROUP_PANEL_SYNTHESIS_VERSION,
    GroupPanelSynthesisConfig, GroupPanelSynthesisDispatchClaim, GroupPanelSynthesisJournalError,
    GroupPanelSynthesisOutcome, GroupPanelSynthesisOutputTarget,
    GroupPanelSynthesisPreparedReceipt, GroupPanelSynthesisProvider, GroupPanelSynthesisRecord,
    GroupPanelSynthesisRecovery, GroupPanelSynthesisRequestConfig, GroupPanelSynthesisResult,
    GroupPanelSynthesisResultArtifact, GroupPanelSynthesisResultReceipt, GroupPanelSynthesisSource,
    GroupPanelSynthesisStatus, GroupPanelSynthesisWritebackTarget,
    MAX_GROUP_PANEL_SYNTHESIS_CONFIG_JSON_BYTES, MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_MODEL_EVENTS, MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS, MAX_GROUP_PANEL_SYNTHESIS_REQUEST_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES, MAX_GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_BYTES,
    PrepareGroupPanelSynthesis,
};
use crate::{
    GROUP_ANALYSIS_PANEL_VERSION, MAX_GROUP_ANALYSIS_PANEL_ANALYSES,
    MAX_GROUP_ANALYSIS_PANEL_MANIFEST_BYTES, MIN_GROUP_ANALYSIS_PANEL_ANALYSES,
};

impl GroupPanelSynthesisConfig {
    /// Validates content-free provider configuration and fixed local targets.
    ///
    /// # Errors
    ///
    /// Returns an error when a version, destination, limit, or target is invalid.
    pub fn validate(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        validate_config(self)
    }
}

impl GroupPanelSynthesisRequestConfig {
    /// Validates the private configuration used to construct exact request bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, Prompt, limits, or targets.
    pub fn validate(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        validate_request_config(self)
    }
}

impl GroupPanelSynthesisSource {
    /// Validates the immutable Group Analysis Panel identity.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identifiers, digests, counts, or sizes.
    pub fn validate(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        validate_source(self)
    }
}

impl GroupPanelSynthesisRecord {
    /// Validates durable synthesis metadata without trusting its journal.
    ///
    /// # Errors
    ///
    /// Returns an error when a field violates the versioned protocol.
    pub fn validate(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        validate_record(self)
    }
}

impl PrepareGroupPanelSynthesis {
    /// Validates one bounded private preparation candidate.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid source, configuration, body, key, or time.
    pub fn validate(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        validate_prepare(self)
    }
}

impl ClaimGroupPanelSynthesisDispatch {
    /// Validates fields for one exclusive off-machine dispatch release.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, consent version, or time.
    pub fn validate(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        validate_claim_request(self)
    }
}

impl GroupPanelSynthesisResultArtifact {
    /// Validates the intrinsic bounds of one synthesis result artifact.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identity, digest, result, usage, or time.
    pub fn validate(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        validate_result_artifact(self)
    }
}

impl CompleteGroupPanelSynthesis {
    /// Validates a completion request before store-side binding checks.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported version or invalid artifact.
    pub fn validate(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        if self.v != GROUP_PANEL_SYNTHESIS_VERSION {
            return Err(synthesis_error("unsupported synthesis completion version"));
        }
        validate_result_artifact(&self.artifact)
    }
}

fn validate_claim_request(
    request: &ClaimGroupPanelSynthesisDispatch,
) -> Result<(), GroupPanelSynthesisJournalError> {
    let valid = request.v == GROUP_PANEL_SYNTHESIS_VERSION
        && valid_identifier(&request.synthesis_id)
        && valid_identifier(&request.dispatch_id)
        && request.consent_version == GROUP_PANEL_SYNTHESIS_CONSENT_VERSION
        && i64::try_from(request.released_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("invalid synthesis dispatch claim"))
}

fn validate_result_artifact(
    artifact: &GroupPanelSynthesisResultArtifact,
) -> Result<(), GroupPanelSynthesisJournalError> {
    let valid = validate_result(&artifact.result)
        && is_lower_hex_digest(&artifact.result_sha256)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES).contains(&artifact.result_bytes)
        && i64::try_from(artifact.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("invalid synthesis result artifact"))
}

fn validate_result(result: &GroupPanelSynthesisResult) -> bool {
    result.v == GROUP_PANEL_SYNTHESIS_RESULT_VERSION
        && valid_identifier(&result.synthesis_id)
        && valid_identifier(&result.dispatch_id)
        && is_lower_hex_digest(&result.request_sha256)
        && valid_outcome(result.outcome)
        && !result.answer.trim().is_empty()
        && result.answer.len() <= MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES
        && usage_is_storable(result.usage)
}

pub(super) fn validate_record(
    record: &GroupPanelSynthesisRecord,
) -> Result<(), GroupPanelSynthesisJournalError> {
    let valid = record.v == GROUP_PANEL_SYNTHESIS_VERSION
        && valid_identifier(&record.synthesis_id)
        && valid_identifier(&record.panel_id)
        && valid_identifier(&record.group_run_id)
        && is_lower_hex_digest(&record.source_snapshot_sha256)
        && is_lower_hex_digest(&record.panel_manifest_sha256)
        && validate_config(&record.config).is_ok()
        && is_lower_hex_digest(&record.config_sha256)
        && is_lower_hex_digest(&record.request_sha256)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_REQUEST_BYTES).contains(&record.request_bytes)
        && record.protocol_version == GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("invalid Group Panel Synthesis record"))
}

pub(super) fn validate_config(
    config: &GroupPanelSynthesisConfig,
) -> Result<(), GroupPanelSynthesisJournalError> {
    let valid = config.v == GROUP_PANEL_SYNTHESIS_VERSION
        && config.provider == GroupPanelSynthesisProvider::OpenAiResponses
        && config.endpoint == GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT
        && valid_text(&config.model, MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES)
        && config.system_prompt_version == GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION
        && is_lower_hex_digest(&config.system_prompt_sha256)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS).contains(&config.max_output_tokens)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES).contains(&config.max_model_output_bytes)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_MODEL_EVENTS).contains(&config.max_model_events)
        && config.output_target == GroupPanelSynthesisOutputTarget::LocalArtifact
        && config.writeback_target == GroupPanelSynthesisWritebackTarget::None;
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("invalid Group Panel Synthesis configuration"))
}

pub(super) fn validate_request_config(
    config: &GroupPanelSynthesisRequestConfig,
) -> Result<(), GroupPanelSynthesisJournalError> {
    let valid = config.v == GROUP_PANEL_SYNTHESIS_VERSION
        && config.provider == GroupPanelSynthesisProvider::OpenAiResponses
        && config.endpoint == GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT
        && valid_text(&config.model, MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES)
        && config.system_prompt_version == GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION
        && !config.system_prompt.trim().is_empty()
        && config.system_prompt.len() <= MAX_GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_BYTES
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS).contains(&config.max_output_tokens)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES).contains(&config.max_model_output_bytes)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_MODEL_EVENTS).contains(&config.max_model_events)
        && config.output_target == GroupPanelSynthesisOutputTarget::LocalArtifact
        && config.writeback_target == GroupPanelSynthesisWritebackTarget::None;
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("invalid synthesis request configuration"))
}

pub(super) fn validate_source(
    source: &GroupPanelSynthesisSource,
) -> Result<(), GroupPanelSynthesisJournalError> {
    let valid = source.panel_version == GROUP_ANALYSIS_PANEL_VERSION
        && valid_identifier(&source.panel_id)
        && valid_identifier(&source.group_run_id)
        && valid_identifier(&source.group_id)
        && is_lower_hex_digest(&source.source_snapshot_sha256)
        && is_lower_hex_digest(&source.panel_manifest_sha256)
        && (1..=MAX_GROUP_ANALYSIS_PANEL_MANIFEST_BYTES).contains(&source.panel_manifest_bytes)
        && (MIN_GROUP_ANALYSIS_PANEL_ANALYSES..=MAX_GROUP_ANALYSIS_PANEL_ANALYSES)
            .contains(&source.analysis_count);
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("invalid Group Panel Synthesis source"))
}

pub(super) fn validate_prepare(
    request: &PrepareGroupPanelSynthesis,
) -> Result<(), GroupPanelSynthesisJournalError> {
    validate_source(&request.source)?;
    validate_request_config(&request.request_config)?;
    validate_config(&request.config)?;
    let same = config_matches(&request.config, &request.request_config);
    let valid = request.v == GROUP_PANEL_SYNTHESIS_VERSION
        && valid_identifier(&request.synthesis_id)
        && same
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_CONFIG_JSON_BYTES).contains(&request.config_json.len())
        && is_lower_hex_digest(&request.config_sha256)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_REQUEST_BYTES).contains(&request.request_body.len())
        && is_lower_hex_digest(&request.request_sha256)
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_PANEL_SYNTHESIS_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("invalid Group Panel Synthesis preparation"))
}

fn config_matches(
    public: &GroupPanelSynthesisConfig,
    private: &GroupPanelSynthesisRequestConfig,
) -> bool {
    public.v == private.v
        && public.provider == private.provider
        && public.endpoint == private.endpoint
        && public.model == private.model
        && public.system_prompt_version == private.system_prompt_version
        && public.max_output_tokens == private.max_output_tokens
        && public.max_model_output_bytes == private.max_model_output_bytes
        && public.max_model_events == private.max_model_events
        && public.output_target == private.output_target
        && public.writeback_target == private.writeback_target
}

pub(super) fn validate_prepared(
    record: &GroupPanelSynthesisRecord,
    receipt: &GroupPanelSynthesisPreparedReceipt,
) -> Result<(), GroupPanelSynthesisJournalError> {
    validate_record(record)?;
    validate_source(&receipt.source)?;
    let source = &receipt.source;
    let source_valid = source.panel_id == record.panel_id
        && source.group_run_id == record.group_run_id
        && source.source_snapshot_sha256 == record.source_snapshot_sha256
        && source.panel_manifest_sha256 == record.panel_manifest_sha256;
    let binding = receipt.v == GROUP_PANEL_SYNTHESIS_VERSION
        && receipt.synthesis_id == record.synthesis_id
        && receipt.config_sha256 == record.config_sha256
        && receipt.request_sha256 == record.request_sha256
        && receipt.request_bytes == record.request_bytes;
    (source_valid && binding)
        .then_some(())
        .ok_or_else(|| synthesis_error("prepared receipt does not match its synthesis"))
}

pub(super) fn validate_claim(
    record: &GroupPanelSynthesisRecord,
    claim: &GroupPanelSynthesisDispatchClaim,
) -> Result<(), GroupPanelSynthesisJournalError> {
    validate_record(record)?;
    let valid = claim.v == GROUP_PANEL_SYNTHESIS_VERSION
        && claim.synthesis_id == record.synthesis_id
        && valid_identifier(&claim.dispatch_id)
        && claim.request_sha256 == record.request_sha256
        && claim.config_sha256 == record.config_sha256
        && claim.provider == record.config.provider
        && claim.endpoint == record.config.endpoint
        && claim.model == record.config.model
        && claim.consent_version == GROUP_PANEL_SYNTHESIS_CONSENT_VERSION
        && i64::try_from(claim.released_at_ms).is_ok()
        && claim.released_at_ms >= record.created_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("dispatch claim does not match its prepared synthesis"))
}

pub(super) fn validate_artifact(
    record: &GroupPanelSynthesisRecord,
    claim: &GroupPanelSynthesisDispatchClaim,
    artifact: &GroupPanelSynthesisResultArtifact,
) -> Result<(), GroupPanelSynthesisJournalError> {
    validate_record(record)?;
    validate_claim(record, claim)?;
    validate_result_artifact(artifact)?;
    let result = &artifact.result;
    let valid = result.synthesis_id == record.synthesis_id
        && result.dispatch_id == claim.dispatch_id
        && result.request_sha256 == record.request_sha256
        && result.answer.len() <= record.config.max_model_output_bytes
        && artifact.created_at_ms >= claim.released_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("invalid Group Panel Synthesis result artifact"))
}

pub(super) fn validate_completion(
    record: &GroupPanelSynthesisRecord,
    claim: &GroupPanelSynthesisDispatchClaim,
    receipt: &GroupPanelSynthesisResultReceipt,
    artifact: &GroupPanelSynthesisResultArtifact,
) -> Result<(), GroupPanelSynthesisJournalError> {
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
        .ok_or_else(|| synthesis_error("completion receipt disagrees with its artifact"))
}

pub(super) fn validate_completion_receipt(
    record: &GroupPanelSynthesisRecord,
    claim: &GroupPanelSynthesisDispatchClaim,
    receipt: &GroupPanelSynthesisResultReceipt,
) -> Result<(), GroupPanelSynthesisJournalError> {
    validate_record(record)?;
    validate_claim(record, claim)?;
    let valid = receipt.v == GROUP_PANEL_SYNTHESIS_RESULT_VERSION
        && receipt.synthesis_id == record.synthesis_id
        && receipt.dispatch_id == claim.dispatch_id
        && receipt.request_sha256 == record.request_sha256
        && valid_outcome(receipt.outcome)
        && is_lower_hex_digest(&receipt.result_sha256)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES).contains(&receipt.result_bytes)
        && usage_is_storable(receipt.usage)
        && i64::try_from(receipt.created_at_ms).is_ok()
        && receipt.created_at_ms >= claim.released_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("completion receipt disagrees with its dispatch"))
}

pub(super) fn validate_status(
    status: GroupPanelSynthesisStatus,
    recovery: &GroupPanelSynthesisRecovery,
) -> Result<(), GroupPanelSynthesisJournalError> {
    let valid = matches!(
        (status, recovery),
        (
            GroupPanelSynthesisStatus::AwaitingConsent,
            GroupPanelSynthesisRecovery::Unprepared | GroupPanelSynthesisRecovery::AwaitingConsent
        ) | (
            GroupPanelSynthesisStatus::DispatchUnknown,
            GroupPanelSynthesisRecovery::DispatchUnknown { .. }
        ) | (
            GroupPanelSynthesisStatus::Completed,
            GroupPanelSynthesisRecovery::Terminal { .. }
        )
    );
    valid
        .then_some(())
        .ok_or_else(|| synthesis_error("synthesis status disagrees with its journal"))
}

pub(super) fn request_digest_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(super::GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

pub(super) fn synthesis_error(message: &str) -> GroupPanelSynthesisJournalError {
    GroupPanelSynthesisJournalError {
        message: message.into(),
    }
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES)
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(unsupported_character)
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_outcome(outcome: GroupPanelSynthesisOutcome) -> bool {
    matches!(
        outcome,
        GroupPanelSynthesisOutcome::Completed | GroupPanelSynthesisOutcome::Length
    )
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
