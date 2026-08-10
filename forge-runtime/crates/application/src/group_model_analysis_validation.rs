use std::collections::BTreeSet;

use forge_runtime_domain::{
    Cancellation, ClaimGroupModelAnalysisDispatch, CompleteGroupModelAnalysisResult,
    GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
    GROUP_MODEL_ANALYSIS_VERSION, GroupModelAnalysisDispatchClaim, GroupModelAnalysisInspection,
    GroupModelAnalysisRecord, GroupModelAnalysisRecovery, GroupModelAnalysisResultArtifact,
    MAX_GROUP_MODEL_ANALYSIS_ID_BYTES, MAX_GROUP_MODEL_ANALYSIS_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES, MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS,
    PrepareGroupModelAnalysis, PrepareGroupModelAnalysisDisposition,
    PrepareGroupModelAnalysisResult, endpoint_allowed,
};

use crate::{
    GroupModelAnalysisServiceError, PrepareGroupModelAnalysisInput,
    group_model_analysis_artifact::validate_result_artifact_encoding,
    group_model_analysis_codec::{canonical_json_bytes, digest_hex},
    group_model_analysis_error::PostClaimError,
    group_model_analysis_prepare::{public_config, request_config_for},
};

pub(crate) fn validate_prepare_input(
    input: &PrepareGroupModelAnalysisInput,
) -> Result<(), GroupModelAnalysisServiceError> {
    let valid = valid_text(&input.analysis_id, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES)
        && valid_text(&input.group_run_id, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES)
        && valid_text(&input.model, MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES)
        && valid_text(
            &input.idempotency_key,
            MAX_GROUP_MODEL_ANALYSIS_IDEMPOTENCY_KEY_BYTES,
        )
        && endpoint_allowed(&input.endpoint)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS).contains(&input.max_output_tokens)
        && i64::try_from(input.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or(GroupModelAnalysisServiceError::InvalidInput)
}

pub(crate) fn validate_send_input(
    request: &ClaimGroupModelAnalysisDispatch,
    cancellation: &Cancellation,
    result_created_at_ms: u64,
) -> Result<(), GroupModelAnalysisServiceError> {
    request
        .validate()
        .map_err(|_| GroupModelAnalysisServiceError::InvalidInput)?;
    let valid = request.consent_version == GROUP_MODEL_ANALYSIS_CONSENT_VERSION
        && !cancellation.is_cancelled()
        && result_created_at_ms >= request.released_at_ms
        && i64::try_from(result_created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or(GroupModelAnalysisServiceError::InvalidInput)
}

pub(crate) fn validate_prepare_result(
    input: &PrepareGroupModelAnalysisInput,
    candidate: &PrepareGroupModelAnalysis,
    result: PrepareGroupModelAnalysisResult,
) -> Result<PrepareGroupModelAnalysisResult, GroupModelAnalysisServiceError> {
    if result.v != GROUP_MODEL_ANALYSIS_VERSION {
        return Err(GroupModelAnalysisServiceError::InconsistentStoreResult);
    }
    let disposition = result.disposition;
    let inspection = checked_inspection(result.inspection)?;
    if !prepare_common(candidate, &inspection)
        || !disposition_matches(input, disposition, &inspection)
    {
        return Err(GroupModelAnalysisServiceError::InconsistentStoreResult);
    }
    Ok(PrepareGroupModelAnalysisResult {
        v: result.v,
        disposition: result.disposition,
        inspection,
    })
}

pub(crate) fn validate_already_claimed(
    request: &ClaimGroupModelAnalysisDispatch,
    expected: &GroupModelAnalysisRecord,
    inspection: GroupModelAnalysisInspection,
) -> Result<GroupModelAnalysisInspection, GroupModelAnalysisServiceError> {
    let inspection = checked_inspection(inspection)?;
    let actual = &inspection.analysis;
    let same_prepared_analysis = actual.v == expected.v
        && actual.analysis_id == expected.analysis_id
        && actual.group_run_id == expected.group_run_id
        && actual.source_snapshot_sha256 == expected.source_snapshot_sha256
        && actual.config == expected.config
        && actual.config_sha256 == expected.config_sha256
        && actual.request_sha256 == expected.request_sha256
        && actual.request_bytes == expected.request_bytes
        && actual.protocol_version == expected.protocol_version
        && actual.created_at_ms == expected.created_at_ms;
    let valid = actual.analysis_id == request.analysis_id
        && same_prepared_analysis
        && inspection.dispatch.is_some()
        && matches!(
            inspection.recovery,
            GroupModelAnalysisRecovery::DispatchUnknown { .. }
                | GroupModelAnalysisRecovery::Terminal { .. }
        );
    valid
        .then_some(inspection)
        .ok_or(GroupModelAnalysisServiceError::InconsistentStoreResult)
}

pub(crate) fn checked_inspection(
    inspection: GroupModelAnalysisInspection,
) -> Result<GroupModelAnalysisInspection, GroupModelAnalysisServiceError> {
    rebuild_inspection(inspection)
        .map_err(|()| GroupModelAnalysisServiceError::InconsistentStoreResult)
}

pub(crate) fn validate_list(
    records: &[GroupModelAnalysisRecord],
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupModelAnalysisServiceError> {
    if records.len() > limit {
        return Err(GroupModelAnalysisServiceError::InconsistentStoreResult);
    }
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|_| GroupModelAnalysisServiceError::InconsistentStoreResult)?;
        let valid = application_config_matches(record)
            && group_run_id.is_none_or(|id| id == record.group_run_id)
            && ids.insert(record.analysis_id.as_str());
        if !valid {
            return Err(GroupModelAnalysisServiceError::InconsistentStoreResult);
        }
    }
    Ok(())
}

pub(crate) fn validate_claim_binding(
    analysis: &GroupModelAnalysisRecord,
    requested: &ClaimGroupModelAnalysisDispatch,
    claim: &GroupModelAnalysisDispatchClaim,
) -> Result<(), PostClaimError> {
    let valid = claim.v == GROUP_MODEL_ANALYSIS_VERSION
        && claim.analysis_id == requested.analysis_id
        && claim.dispatch_id == requested.dispatch_id
        && claim.request_sha256 == analysis.request_sha256
        && claim.config_sha256 == analysis.config_sha256
        && claim.provider == analysis.config.provider
        && claim.endpoint == analysis.config.endpoint
        && claim.model == analysis.config.model
        && claim.consent_version == requested.consent_version
        && claim.released_at_ms == requested.released_at_ms;
    valid
        .then_some(())
        .ok_or(PostClaimError::InconsistentStoreResult)
}

pub(crate) fn validate_completion_result(
    artifact: &GroupModelAnalysisResultArtifact,
    result: CompleteGroupModelAnalysisResult,
) -> Result<CompleteGroupModelAnalysisResult, PostClaimError> {
    if result.v != GROUP_MODEL_ANALYSIS_VERSION {
        return Err(PostClaimError::InconsistentStoreResult);
    }
    let inspection = rebuild_inspection(result.inspection)
        .map_err(|()| PostClaimError::InconsistentStoreResult)?;
    let valid = inspection.result.as_ref() == Some(artifact)
        && inspection.completion.as_ref().is_some_and(|receipt| {
            receipt.result_sha256 == artifact.result_sha256
                && receipt.result_bytes == artifact.result_bytes
                && receipt.outcome == artifact.result.outcome
        })
        && matches!(
            inspection.recovery,
            GroupModelAnalysisRecovery::Terminal { outcome }
                if outcome == artifact.result.outcome
        );
    if !valid {
        return Err(PostClaimError::InconsistentStoreResult);
    }
    Ok(CompleteGroupModelAnalysisResult {
        v: result.v,
        disposition: result.disposition,
        inspection,
    })
}

pub(crate) fn validate_identifier(value: &str) -> Result<(), GroupModelAnalysisServiceError> {
    valid_text(value, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES)
        .then_some(())
        .ok_or(GroupModelAnalysisServiceError::InvalidInput)
}

fn rebuild_inspection(
    inspection: GroupModelAnalysisInspection,
) -> Result<GroupModelAnalysisInspection, ()> {
    if !application_config_matches(&inspection.analysis)
        || inspection
            .result
            .as_ref()
            .is_some_and(|artifact| validate_result_artifact_encoding(artifact).is_err())
    {
        return Err(());
    }
    let rebuilt = GroupModelAnalysisInspection::validate(
        inspection.analysis.clone(),
        inspection.events.clone(),
        inspection.result.clone(),
    )
    .map_err(|_| ())?;
    (rebuilt == inspection).then_some(inspection).ok_or(())
}

fn application_config_matches(record: &GroupModelAnalysisRecord) -> bool {
    let request_config = request_config_for(
        &record.config.model,
        record.config.max_output_tokens,
        &record.config.endpoint,
    );
    if record.config != public_config(&request_config) {
        return false;
    }
    canonical_json_bytes(&request_config).is_ok_and(|bytes| {
        digest_hex(GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN, &bytes) == record.config_sha256
    })
}

fn prepare_common(
    candidate: &PrepareGroupModelAnalysis,
    inspection: &GroupModelAnalysisInspection,
) -> bool {
    let record = &inspection.analysis;
    record.group_run_id == candidate.source.group_run_id
        && record.source_snapshot_sha256 == candidate.source.snapshot_sha256
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
    input: &PrepareGroupModelAnalysisInput,
    disposition: PrepareGroupModelAnalysisDisposition,
    inspection: &GroupModelAnalysisInspection,
) -> bool {
    match disposition {
        PrepareGroupModelAnalysisDisposition::Created => {
            inspection.analysis.analysis_id == input.analysis_id
                && inspection.analysis.created_at_ms == input.created_at_ms
                && matches!(
                    inspection.recovery,
                    GroupModelAnalysisRecovery::AwaitingConsent
                )
        }
        PrepareGroupModelAnalysisDisposition::Replayed => true,
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
