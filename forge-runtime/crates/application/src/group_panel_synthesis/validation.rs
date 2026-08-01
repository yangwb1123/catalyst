use std::collections::BTreeSet;

use crate::runtime_domain::{
    Cancellation, ClaimGroupPanelSynthesisDispatch, CompleteGroupPanelSynthesisResult,
    GROUP_PANEL_SYNTHESIS_CONFIG_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_CONSENT_VERSION,
    GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_VERSION,
    GroupPanelSynthesisDispatchClaim, GroupPanelSynthesisInspection, GroupPanelSynthesisRecord,
    GroupPanelSynthesisRecovery, GroupPanelSynthesisResultArtifact,
    MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES, MAX_GROUP_PANEL_SYNTHESIS_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES, MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS,
    PrepareGroupPanelSynthesis, PrepareGroupPanelSynthesisDisposition,
    PrepareGroupPanelSynthesisResult,
};

use crate::group_model_analysis_codec::{canonical_json_bytes, digest_hex};

use super::{
    artifact::validate_result_artifact_encoding,
    error::{GroupPanelSynthesisServiceError, SynthesisPostClaimError},
    prepare::{PrepareGroupPanelSynthesisInput, public_config, request_config_for},
};

pub(super) fn validate_prepare_input(
    input: &PrepareGroupPanelSynthesisInput,
) -> Result<(), GroupPanelSynthesisServiceError> {
    let valid = valid_text(&input.synthesis_id, MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES)
        && valid_text(&input.panel_id, MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES)
        && valid_text(&input.model, MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES)
        && valid_text(
            &input.idempotency_key,
            MAX_GROUP_PANEL_SYNTHESIS_IDEMPOTENCY_KEY_BYTES,
        )
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS).contains(&input.max_output_tokens)
        && i64::try_from(input.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or(GroupPanelSynthesisServiceError::InvalidInput)
}

pub(super) fn validate_send_input(
    request: &ClaimGroupPanelSynthesisDispatch,
    confirm_off_machine: bool,
    cancellation: &Cancellation,
    result_not_before_ms: u64,
) -> Result<(), GroupPanelSynthesisServiceError> {
    request
        .validate()
        .map_err(|_| GroupPanelSynthesisServiceError::InvalidInput)?;
    let valid = confirm_off_machine
        && request.consent_version == GROUP_PANEL_SYNTHESIS_CONSENT_VERSION
        && !cancellation.is_cancelled()
        && result_not_before_ms >= request.released_at_ms
        && i64::try_from(result_not_before_ms).is_ok();
    valid
        .then_some(())
        .ok_or(GroupPanelSynthesisServiceError::InvalidInput)
}

pub(super) fn validate_prepare_result(
    input: &PrepareGroupPanelSynthesisInput,
    candidate: &PrepareGroupPanelSynthesis,
    result: PrepareGroupPanelSynthesisResult,
) -> Result<PrepareGroupPanelSynthesisResult, GroupPanelSynthesisServiceError> {
    if result.v != GROUP_PANEL_SYNTHESIS_VERSION {
        return Err(GroupPanelSynthesisServiceError::InconsistentStoreResult);
    }
    let disposition = result.disposition;
    let inspection = checked_inspection(result.inspection)?;
    if !prepare_common(candidate, &inspection)
        || !disposition_matches(input, disposition, &inspection)
    {
        return Err(GroupPanelSynthesisServiceError::InconsistentStoreResult);
    }
    Ok(PrepareGroupPanelSynthesisResult {
        v: result.v,
        disposition,
        inspection,
    })
}

pub(super) fn validate_already_claimed(
    request: &ClaimGroupPanelSynthesisDispatch,
    expected: &GroupPanelSynthesisRecord,
    inspection: GroupPanelSynthesisInspection,
) -> Result<GroupPanelSynthesisInspection, GroupPanelSynthesisServiceError> {
    let inspection = checked_inspection(inspection)?;
    let actual = &inspection.synthesis;
    let same = actual.v == expected.v
        && actual.synthesis_id == expected.synthesis_id
        && actual.panel_id == expected.panel_id
        && actual.group_run_id == expected.group_run_id
        && actual.source_snapshot_sha256 == expected.source_snapshot_sha256
        && actual.panel_manifest_sha256 == expected.panel_manifest_sha256
        && actual.config == expected.config
        && actual.config_sha256 == expected.config_sha256
        && actual.request_sha256 == expected.request_sha256
        && actual.request_bytes == expected.request_bytes
        && actual.protocol_version == expected.protocol_version
        && actual.created_at_ms == expected.created_at_ms;
    let valid = actual.synthesis_id == request.synthesis_id
        && same
        && inspection.dispatch.is_some()
        && matches!(
            inspection.recovery,
            GroupPanelSynthesisRecovery::DispatchUnknown { .. }
                | GroupPanelSynthesisRecovery::Terminal { .. }
        );
    valid
        .then_some(inspection)
        .ok_or(GroupPanelSynthesisServiceError::InconsistentStoreResult)
}

pub(super) fn checked_inspection(
    inspection: GroupPanelSynthesisInspection,
) -> Result<GroupPanelSynthesisInspection, GroupPanelSynthesisServiceError> {
    rebuild_inspection(inspection)
        .map_err(|()| GroupPanelSynthesisServiceError::InconsistentStoreResult)
}

pub(super) fn validate_list(
    records: &[GroupPanelSynthesisRecord],
    panel_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupPanelSynthesisServiceError> {
    if records.len() > limit {
        return Err(GroupPanelSynthesisServiceError::InconsistentStoreResult);
    }
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|_| GroupPanelSynthesisServiceError::InconsistentStoreResult)?;
        let valid = application_config_matches(record)
            && panel_id.is_none_or(|id| id == record.panel_id)
            && ids.insert(record.synthesis_id.as_str());
        if !valid {
            return Err(GroupPanelSynthesisServiceError::InconsistentStoreResult);
        }
    }
    Ok(())
}

pub(super) fn validate_claim_binding(
    synthesis: &GroupPanelSynthesisRecord,
    requested: &ClaimGroupPanelSynthesisDispatch,
    claim: &GroupPanelSynthesisDispatchClaim,
) -> Result<(), SynthesisPostClaimError> {
    let valid = claim.v == GROUP_PANEL_SYNTHESIS_VERSION
        && claim.synthesis_id == requested.synthesis_id
        && claim.dispatch_id == requested.dispatch_id
        && claim.request_sha256 == synthesis.request_sha256
        && claim.config_sha256 == synthesis.config_sha256
        && claim.provider == synthesis.config.provider
        && claim.endpoint == synthesis.config.endpoint
        && claim.model == synthesis.config.model
        && claim.consent_version == requested.consent_version
        && claim.released_at_ms == requested.released_at_ms;
    valid
        .then_some(())
        .ok_or(SynthesisPostClaimError::InconsistentStoreResult)
}

pub(super) fn validate_completion_result(
    artifact: &GroupPanelSynthesisResultArtifact,
    result: CompleteGroupPanelSynthesisResult,
) -> Result<CompleteGroupPanelSynthesisResult, SynthesisPostClaimError> {
    if result.v != GROUP_PANEL_SYNTHESIS_VERSION {
        return Err(SynthesisPostClaimError::InconsistentStoreResult);
    }
    let inspection = rebuild_inspection(result.inspection)
        .map_err(|()| SynthesisPostClaimError::InconsistentStoreResult)?;
    let valid = inspection.result.as_ref() == Some(artifact)
        && inspection.completion.as_ref().is_some_and(|receipt| {
            receipt.result_sha256 == artifact.result_sha256
                && receipt.result_bytes == artifact.result_bytes
                && receipt.outcome == artifact.result.outcome
        })
        && matches!(
            inspection.recovery,
            GroupPanelSynthesisRecovery::Terminal { outcome }
                if outcome == artifact.result.outcome
        );
    if !valid {
        return Err(SynthesisPostClaimError::InconsistentStoreResult);
    }
    Ok(CompleteGroupPanelSynthesisResult {
        v: result.v,
        disposition: result.disposition,
        inspection,
    })
}

pub(super) fn validate_identifier(value: &str) -> Result<(), GroupPanelSynthesisServiceError> {
    valid_text(value, MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES)
        .then_some(())
        .ok_or(GroupPanelSynthesisServiceError::InvalidInput)
}

pub(super) fn validate_expected_body(
    synthesis: &GroupPanelSynthesisRecord,
    body: &[u8],
) -> Result<(), GroupPanelSynthesisServiceError> {
    let valid = !body.is_empty()
        && body.len() == synthesis.request_bytes
        && digest_hex(GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN, body)
            == synthesis.request_sha256;
    valid
        .then_some(())
        .ok_or(GroupPanelSynthesisServiceError::InconsistentStoreResult)
}

fn rebuild_inspection(
    inspection: GroupPanelSynthesisInspection,
) -> Result<GroupPanelSynthesisInspection, ()> {
    if !application_config_matches(&inspection.synthesis)
        || inspection
            .result
            .as_ref()
            .is_some_and(|artifact| validate_result_artifact_encoding(artifact).is_err())
    {
        return Err(());
    }
    let rebuilt = GroupPanelSynthesisInspection::validate(
        inspection.synthesis.clone(),
        inspection.events.clone(),
        inspection.result.clone(),
    )
    .map_err(|_| ())?;
    (rebuilt == inspection).then_some(inspection).ok_or(())
}

fn application_config_matches(record: &GroupPanelSynthesisRecord) -> bool {
    let config = request_config_for(&record.config.model, record.config.max_output_tokens);
    if record.config != public_config(&config) {
        return false;
    }
    canonical_json_bytes(&config).is_ok_and(|bytes| {
        digest_hex(GROUP_PANEL_SYNTHESIS_CONFIG_DIGEST_DOMAIN, &bytes) == record.config_sha256
    })
}

fn prepare_common(
    candidate: &PrepareGroupPanelSynthesis,
    inspection: &GroupPanelSynthesisInspection,
) -> bool {
    let record = &inspection.synthesis;
    record.panel_id == candidate.source.panel_id
        && record.group_run_id == candidate.source.group_run_id
        && record.source_snapshot_sha256 == candidate.source.source_snapshot_sha256
        && record.panel_manifest_sha256 == candidate.source.panel_manifest_sha256
        && record.config == candidate.config
        && record.config_sha256 == candidate.config_sha256
        && record.request_sha256 == candidate.request_sha256
        && record.request_bytes == candidate.request_body.len()
        && inspection.prepared.as_ref().is_some_and(|prepared| {
            prepared.source == candidate.source
                && prepared.config_sha256 == candidate.config_sha256
                && prepared.request_sha256 == candidate.request_sha256
                && prepared.request_bytes == candidate.request_body.len()
        })
}

fn disposition_matches(
    input: &PrepareGroupPanelSynthesisInput,
    disposition: PrepareGroupPanelSynthesisDisposition,
    inspection: &GroupPanelSynthesisInspection,
) -> bool {
    match disposition {
        PrepareGroupPanelSynthesisDisposition::Created => {
            inspection.synthesis.synthesis_id == input.synthesis_id
                && inspection.synthesis.created_at_ms == input.created_at_ms
                && matches!(
                    inspection.recovery,
                    GroupPanelSynthesisRecovery::AwaitingConsent
                )
        }
        PrepareGroupPanelSynthesisDisposition::Replayed => true,
    }
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(unsupported_character)
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
