use forge_runtime_domain::{
    GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_RESULT_VERSION,
    GroupModelAnalysisDispatchClaim, GroupModelAnalysisOutcome, GroupModelAnalysisResult,
    GroupModelAnalysisResultArtifact, MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES, ModelFinishReason,
};

use crate::{
    group_model_analysis_codec::{canonical_json_bytes, digest_hex},
    group_model_analysis_collector::AnalysisModelTurn,
    group_model_analysis_error::{AnalysisLimit, PostClaimError},
};

pub(crate) fn build_result_artifact(
    claim: &GroupModelAnalysisDispatchClaim,
    turn: AnalysisModelTurn,
    created_at_ms: u64,
) -> Result<GroupModelAnalysisResultArtifact, PostClaimError> {
    let result = GroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_RESULT_VERSION,
        analysis_id: claim.analysis_id.clone(),
        dispatch_id: claim.dispatch_id.clone(),
        request_sha256: claim.request_sha256.clone(),
        outcome: outcome(turn.finish_reason)?,
        answer: turn.answer,
        usage: turn.usage,
    };
    let bytes = canonical_json_bytes(&result).map_err(|_| PostClaimError::Protocol)?;
    if bytes.is_empty() || bytes.len() > MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES {
        return Err(PostClaimError::Limit(AnalysisLimit::ResultBytes));
    }
    Ok(GroupModelAnalysisResultArtifact {
        result,
        result_sha256: digest_hex(GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, &bytes),
        result_bytes: bytes.len(),
        created_at_ms,
    })
}

pub(crate) fn validate_result_artifact_encoding(
    artifact: &GroupModelAnalysisResultArtifact,
) -> Result<(), PostClaimError> {
    let bytes = canonical_json_bytes(&artifact.result)
        .map_err(|_| PostClaimError::InconsistentStoreResult)?;
    let valid = artifact.result_bytes == bytes.len()
        && artifact.result_sha256 == digest_hex(GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, &bytes);
    valid
        .then_some(())
        .ok_or(PostClaimError::InconsistentStoreResult)
}

fn outcome(reason: ModelFinishReason) -> Result<GroupModelAnalysisOutcome, PostClaimError> {
    match reason {
        ModelFinishReason::Completed => Ok(GroupModelAnalysisOutcome::Completed),
        ModelFinishReason::Length => Ok(GroupModelAnalysisOutcome::Length),
        ModelFinishReason::ToolUse => Err(PostClaimError::ToolCall),
    }
}
