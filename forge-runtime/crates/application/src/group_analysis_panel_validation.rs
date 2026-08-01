use std::collections::BTreeSet;

use forge_runtime_domain::{
    GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, GROUP_ANALYSIS_PANEL_VERSION,
    GroupAnalysisPanelContribution, GroupAnalysisPanelInspection, GroupAnalysisPanelRecord,
    GroupModelAnalysisOutcome, GroupModelAnalysisRecovery, GroupModelAnalysisSource,
    GroupModelAnalysisStore, GroupRunSnapshot, MAX_GROUP_ANALYSIS_PANEL_ANALYSES,
    MAX_GROUP_ANALYSIS_PANEL_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT,
    MIN_GROUP_ANALYSIS_PANEL_ANALYSES, PrepareGroupAnalysisPanel,
    PrepareGroupAnalysisPanelDisposition, PrepareGroupAnalysisPanelResult,
};

use crate::{
    GroupAnalysisPanelServiceError, PrepareGroupAnalysisPanelInput,
    group_model_analysis_codec::{canonical_json_bytes, digest_hex},
    group_model_analysis_source::validate_analysis_source_binding,
    group_model_analysis_validation::{
        checked_inspection as checked_analysis, validate_identifier,
    },
};

pub(crate) fn validate_input(
    input: &PrepareGroupAnalysisPanelInput,
) -> Result<(), GroupAnalysisPanelServiceError> {
    validate_identifier(&input.panel_id).map_err(invalid_input)?;
    validate_identifier(&input.group_run_id).map_err(invalid_input)?;
    if !valid_text(
        &input.idempotency_key,
        MAX_GROUP_ANALYSIS_PANEL_IDEMPOTENCY_KEY_BYTES,
    ) || i64::try_from(input.created_at_ms).is_err()
        || !(MIN_GROUP_ANALYSIS_PANEL_ANALYSES..=MAX_GROUP_ANALYSIS_PANEL_ANALYSES)
            .contains(&input.analysis_ids.len())
    {
        return Err(GroupAnalysisPanelServiceError::InvalidInput);
    }
    let mut ids = BTreeSet::new();
    for id in &input.analysis_ids {
        validate_identifier(id).map_err(invalid_input)?;
        if !ids.insert(id.as_str()) {
            return Err(GroupAnalysisPanelServiceError::InvalidInput);
        }
    }
    Ok(())
}

pub(crate) fn checked_contribution(
    analyses: &dyn GroupModelAnalysisStore,
    requested_id: &str,
    snapshot: &GroupRunSnapshot,
    source: &GroupModelAnalysisSource,
) -> Result<GroupAnalysisPanelContribution, GroupAnalysisPanelServiceError> {
    let inspection = analyses.inspect_group_model_analysis(requested_id)?;
    if inspection.analysis.analysis_id != requested_id {
        return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
    }
    let inspection = checked_analysis(inspection)
        .map_err(|_| GroupAnalysisPanelServiceError::InconsistentStoreResult)?;
    validate_analysis_source_binding(&inspection.analysis, snapshot)
        .map_err(|_| GroupAnalysisPanelServiceError::InconsistentStoreResult)?;
    if inspection.prepared.as_ref().map(|receipt| &receipt.source) != Some(source) {
        return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
    }
    let result = completed_result(&inspection)?;
    Ok(GroupAnalysisPanelContribution {
        analysis: inspection.analysis,
        result,
    })
}

pub(crate) fn checked_inspection(
    inspection: GroupAnalysisPanelInspection,
) -> Result<GroupAnalysisPanelInspection, GroupAnalysisPanelServiceError> {
    inspection
        .validate()
        .map_err(|_| GroupAnalysisPanelServiceError::InconsistentStoreResult)?;
    let bytes = canonical_json_bytes(&inspection.manifest)
        .map_err(|_| GroupAnalysisPanelServiceError::InconsistentStoreResult)?;
    let panel = &inspection.panel;
    let valid = inspection.v == GROUP_ANALYSIS_PANEL_VERSION
        && panel.manifest_bytes == bytes.len()
        && panel.manifest_sha256 == digest_hex(GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, &bytes);
    valid
        .then_some(inspection)
        .ok_or(GroupAnalysisPanelServiceError::InconsistentStoreResult)
}

pub(crate) fn validate_prepare_result(
    input: &PrepareGroupAnalysisPanelInput,
    request: &PrepareGroupAnalysisPanel,
    result: PrepareGroupAnalysisPanelResult,
) -> Result<PrepareGroupAnalysisPanelResult, GroupAnalysisPanelServiceError> {
    if result.v != GROUP_ANALYSIS_PANEL_VERSION {
        return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
    }
    let disposition = result.disposition;
    let inspection = checked_inspection(result.inspection)?;
    let created_matches = disposition != PrepareGroupAnalysisPanelDisposition::Created
        || (inspection.panel.panel_id == input.panel_id
            && inspection.panel.created_at_ms == input.created_at_ms);
    let valid = inspection.manifest == request.manifest
        && inspection.panel.group_run_id == input.group_run_id
        && created_matches;
    if !valid {
        return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
    }
    Ok(PrepareGroupAnalysisPanelResult {
        v: result.v,
        disposition,
        inspection,
    })
}

pub(crate) fn validate_list(
    records: &[GroupAnalysisPanelRecord],
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAnalysisPanelServiceError> {
    if !(1..=MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT).contains(&limit) {
        return Err(GroupAnalysisPanelServiceError::InvalidInput);
    }
    if records.len() > limit {
        return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
    }
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|_| GroupAnalysisPanelServiceError::InconsistentStoreResult)?;
        let valid = group_run_id.is_none_or(|id| id == record.group_run_id)
            && ids.insert(record.panel_id.as_str());
        if !valid {
            return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
        }
    }
    Ok(())
}

fn completed_result(
    inspection: &forge_runtime_domain::GroupModelAnalysisInspection,
) -> Result<forge_runtime_domain::GroupModelAnalysisResultArtifact, GroupAnalysisPanelServiceError>
{
    let terminal = matches!(
        inspection.recovery,
        GroupModelAnalysisRecovery::Terminal {
            outcome: GroupModelAnalysisOutcome::Completed
        }
    );
    let result = inspection
        .result
        .clone()
        .filter(|artifact| artifact.result.outcome == GroupModelAnalysisOutcome::Completed);
    if terminal {
        result.ok_or(GroupAnalysisPanelServiceError::InconsistentStoreResult)
    } else {
        Err(GroupAnalysisPanelServiceError::InvalidInput)
    }
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(|value| {
            value.is_control()
                || matches!(
                    value,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
}

fn invalid_input(_: crate::GroupModelAnalysisServiceError) -> GroupAnalysisPanelServiceError {
    GroupAnalysisPanelServiceError::InvalidInput
}
