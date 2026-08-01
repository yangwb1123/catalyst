use crate::runtime_domain::{
    GROUP_PANEL_SYNTHESIS_RESULT_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
    GroupPanelSynthesisDispatchClaim, GroupPanelSynthesisOutcome, GroupPanelSynthesisResult,
    GroupPanelSynthesisResultArtifact, MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES, ModelFinishReason,
};

use crate::{
    group_model_analysis_codec::{canonical_json_bytes, digest_hex},
    group_model_analysis_collector::PreparedTurn,
};

use super::error::SynthesisPostClaimError;

pub(super) fn build_result_artifact(
    claim: &GroupPanelSynthesisDispatchClaim,
    turn: PreparedTurn,
    created_at_ms: u64,
) -> Result<GroupPanelSynthesisResultArtifact, SynthesisPostClaimError> {
    let result = GroupPanelSynthesisResult {
        v: GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
        synthesis_id: claim.synthesis_id.clone(),
        dispatch_id: claim.dispatch_id.clone(),
        request_sha256: claim.request_sha256.clone(),
        outcome: outcome(turn.finish_reason)?,
        answer: turn.answer,
        usage: turn.usage,
    };
    let bytes = canonical_json_bytes(&result).map_err(|_| SynthesisPostClaimError::Turn)?;
    if bytes.is_empty() || bytes.len() > MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES {
        return Err(SynthesisPostClaimError::Turn);
    }
    Ok(GroupPanelSynthesisResultArtifact {
        result,
        result_sha256: digest_hex(GROUP_PANEL_SYNTHESIS_RESULT_DIGEST_DOMAIN, &bytes),
        result_bytes: bytes.len(),
        created_at_ms,
    })
}

pub(super) fn validate_result_artifact_encoding(
    artifact: &GroupPanelSynthesisResultArtifact,
) -> Result<(), SynthesisPostClaimError> {
    let bytes = canonical_json_bytes(&artifact.result)
        .map_err(|_| SynthesisPostClaimError::InconsistentStoreResult)?;
    let valid = artifact.result_bytes == bytes.len()
        && artifact.result_sha256 == digest_hex(GROUP_PANEL_SYNTHESIS_RESULT_DIGEST_DOMAIN, &bytes);
    valid
        .then_some(())
        .ok_or(SynthesisPostClaimError::InconsistentStoreResult)
}

fn outcome(
    reason: ModelFinishReason,
) -> Result<GroupPanelSynthesisOutcome, SynthesisPostClaimError> {
    match reason {
        ModelFinishReason::Completed => Ok(GroupPanelSynthesisOutcome::Completed),
        ModelFinishReason::Length => Ok(GroupPanelSynthesisOutcome::Length),
        ModelFinishReason::ToolUse => Err(SynthesisPostClaimError::Turn),
    }
}
