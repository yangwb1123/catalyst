use forge_runtime_domain::{
    Cancellation, GROUP_CONTEXT_DIGEST_DOMAIN, GROUP_CONTEXT_VERSION,
    GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION, GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_VERSION, GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, GROUP_RUN_VERSION,
    GroupModelAnalysisRecord, GroupModelAnalysisSource, GroupRunSnapshot, GroupRunStatus,
    MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES, MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES, ModelRequest,
};

use crate::{
    GroupModelAnalysisServiceError,
    group_model_analysis_codec::{canonical_json_bytes, digest_hex},
    group_model_analysis_prepare::{model_request, public_config, request_config_for},
};

pub(crate) fn validate_source(
    snapshot: &GroupRunSnapshot,
    requested_id: &str,
) -> Result<GroupModelAnalysisSource, GroupModelAnalysisServiceError> {
    let context_bytes = canonical_json_bytes(&snapshot.context)
        .map_err(|_| GroupModelAnalysisServiceError::InvalidSource)?;
    let payload_bytes = canonical_json_bytes(&snapshot.context.payload)
        .map_err(|_| GroupModelAnalysisServiceError::InvalidSource)?;
    let run = &snapshot.run;
    let valid = snapshot.v == GROUP_RUN_VERSION
        && run.v == GROUP_RUN_VERSION
        && run.status == GroupRunStatus::Prepared
        && run.run_id == requested_id
        && run.context_version == GROUP_CONTEXT_VERSION
        && snapshot.context.v == GROUP_CONTEXT_VERSION
        && run.context_version == snapshot.context.v
        && snapshot.context.payload.group.id == run.group_id
        && snapshot.context.slice_sha256 == run.context_slice_sha256
        && digest_hex(GROUP_CONTEXT_DIGEST_DOMAIN, &payload_bytes) == run.context_slice_sha256
        && snapshot.context_json.as_bytes() == context_bytes
        && run.snapshot_bytes == context_bytes.len()
        && (1..=MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES).contains(&run.snapshot_bytes)
        && digest_hex(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, &context_bytes) == run.snapshot_sha256;
    if !valid {
        return Err(GroupModelAnalysisServiceError::InvalidSource);
    }
    Ok(GroupModelAnalysisSource {
        group_run_version: run.v,
        group_run_id: run.run_id.clone(),
        group_id: run.group_id.clone(),
        context_version: run.context_version,
        context_slice_sha256: run.context_slice_sha256.clone(),
        snapshot_sha256: run.snapshot_sha256.clone(),
        snapshot_bytes: run.snapshot_bytes,
    })
}

pub(crate) fn expected_request(
    analysis: &GroupModelAnalysisRecord,
    snapshot: &GroupRunSnapshot,
    cancellation: Cancellation,
) -> Result<ModelRequest, GroupModelAnalysisServiceError> {
    let config = request_config_for(&analysis.config.model, analysis.config.max_output_tokens);
    if public_config(&config) != analysis.config {
        return Err(GroupModelAnalysisServiceError::InconsistentStoreResult);
    }
    Ok(model_request(&config, snapshot, cancellation))
}

pub(crate) fn validate_analysis_source_binding(
    analysis: &GroupModelAnalysisRecord,
    snapshot: &GroupRunSnapshot,
) -> Result<(), GroupModelAnalysisServiceError> {
    let valid = analysis.v == GROUP_MODEL_ANALYSIS_VERSION
        && analysis.group_run_id == snapshot.run.run_id
        && analysis.source_snapshot_sha256 == snapshot.run.snapshot_sha256
        && analysis.protocol_version == GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION;
    valid
        .then_some(())
        .ok_or(GroupModelAnalysisServiceError::InconsistentStoreResult)
}

pub(crate) fn validate_expected_body(
    analysis: &GroupModelAnalysisRecord,
    body: &[u8],
) -> Result<(), GroupModelAnalysisServiceError> {
    let valid = !body.is_empty()
        && body.len() == analysis.request_bytes
        && body.len() <= MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES
        && digest_hex(GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN, body) == analysis.request_sha256;
    valid
        .then_some(())
        .ok_or(GroupModelAnalysisServiceError::InconsistentStoreResult)
}
