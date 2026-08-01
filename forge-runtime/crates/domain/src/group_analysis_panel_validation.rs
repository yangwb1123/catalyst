use std::collections::BTreeSet;

use super::{
    GROUP_ANALYSIS_PANEL_VERSION, GroupAnalysisPanelInspection, GroupAnalysisPanelManifest,
    GroupAnalysisPanelRecord, GroupAnalysisPanelStatus, GroupAnalysisPanelValidationError,
    MAX_GROUP_ANALYSIS_PANEL_ANALYSES, MAX_GROUP_ANALYSIS_PANEL_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_ANALYSIS_PANEL_MANIFEST_BYTES, MIN_GROUP_ANALYSIS_PANEL_ANALYSES,
    PrepareGroupAnalysisPanel,
};
use crate::{
    GroupModelAnalysisOutcome, GroupModelAnalysisStatus, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES,
};

pub(super) fn validate_manifest(
    manifest: &GroupAnalysisPanelManifest,
) -> Result<(), GroupAnalysisPanelValidationError> {
    manifest
        .source
        .validate()
        .map_err(|_| invalid("panel source is invalid"))?;
    if manifest.v != GROUP_ANALYSIS_PANEL_VERSION
        || !(MIN_GROUP_ANALYSIS_PANEL_ANALYSES..=MAX_GROUP_ANALYSIS_PANEL_ANALYSES)
            .contains(&manifest.contributions.len())
    {
        return Err(invalid(
            "panel manifest version or contribution count is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut result_bytes = 0_usize;
    for contribution in &manifest.contributions {
        validate_contribution(manifest, contribution)?;
        if !ids.insert(contribution.analysis.analysis_id.as_str()) {
            return Err(invalid("panel analysis identifiers must be unique"));
        }
        result_bytes = result_bytes
            .checked_add(contribution.result.result_bytes)
            .ok_or_else(|| invalid("panel result byte count overflowed"))?;
    }
    validate_aggregate_bytes(result_bytes)
}

pub(super) fn validate_record(
    record: &GroupAnalysisPanelRecord,
) -> Result<(), GroupAnalysisPanelValidationError> {
    let valid = record.v == GROUP_ANALYSIS_PANEL_VERSION
        && valid_text(&record.panel_id, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES)
        && valid_text(&record.group_run_id, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES)
        && record.status == GroupAnalysisPanelStatus::Prepared
        && is_lower_hex_digest(&record.source_snapshot_sha256)
        && is_lower_hex_digest(&record.manifest_sha256)
        && (1..=MAX_GROUP_ANALYSIS_PANEL_MANIFEST_BYTES).contains(&record.manifest_bytes)
        && (MIN_GROUP_ANALYSIS_PANEL_ANALYSES..=MAX_GROUP_ANALYSIS_PANEL_ANALYSES)
            .contains(&record.analysis_count)
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("panel record is invalid"))
}

pub(super) fn validate_prepare(
    request: &PrepareGroupAnalysisPanel,
) -> Result<(), GroupAnalysisPanelValidationError> {
    request.manifest.validate()?;
    let valid = request.v == GROUP_ANALYSIS_PANEL_VERSION
        && valid_text(&request.panel_id, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES)
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_ANALYSIS_PANEL_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("panel preparation is invalid"))
}

pub(super) fn validate_inspection(
    inspection: &GroupAnalysisPanelInspection,
) -> Result<(), GroupAnalysisPanelValidationError> {
    inspection.panel.validate()?;
    inspection.manifest.validate()?;
    let panel = &inspection.panel;
    let manifest = &inspection.manifest;
    let valid = inspection.v == GROUP_ANALYSIS_PANEL_VERSION
        && panel.group_run_id == manifest.source.group_run_id
        && panel.source_snapshot_sha256 == manifest.source.snapshot_sha256
        && panel.analysis_count == manifest.contributions.len();
    valid
        .then_some(())
        .ok_or_else(|| invalid("panel inspection disagrees with its manifest"))
}

fn validate_contribution(
    manifest: &GroupAnalysisPanelManifest,
    contribution: &super::GroupAnalysisPanelContribution,
) -> Result<(), GroupAnalysisPanelValidationError> {
    contribution
        .analysis
        .validate()
        .map_err(|_| invalid("panel analysis record is invalid"))?;
    contribution
        .result
        .validate()
        .map_err(|_| invalid("panel result artifact is invalid"))?;
    let analysis = &contribution.analysis;
    let result = &contribution.result;
    let valid = analysis.status == GroupModelAnalysisStatus::Completed
        && analysis.group_run_id == manifest.source.group_run_id
        && analysis.source_snapshot_sha256 == manifest.source.snapshot_sha256
        && result.result.analysis_id == analysis.analysis_id
        && result.result.request_sha256 == analysis.request_sha256
        && result.result.outcome == GroupModelAnalysisOutcome::Completed
        && result.created_at_ms >= analysis.created_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| invalid("panel contribution is not one complete source-bound analysis"))
}

fn validate_aggregate_bytes(bytes: usize) -> Result<(), GroupAnalysisPanelValidationError> {
    let maximum = MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES
        .checked_mul(MAX_GROUP_ANALYSIS_PANEL_ANALYSES)
        .expect("panel result bound fits usize");
    if bytes <= maximum {
        Ok(())
    } else {
        Err(invalid(
            "panel contribution results exceed their aggregate bound",
        ))
    }
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

fn invalid(message: &str) -> GroupAnalysisPanelValidationError {
    GroupAnalysisPanelValidationError {
        message: message.into(),
    }
}
