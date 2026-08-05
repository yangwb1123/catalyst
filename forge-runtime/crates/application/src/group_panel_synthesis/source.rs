use crate::runtime_domain::{
    GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
    GROUP_PANEL_SYNTHESIS_VERSION, GroupAnalysisPanelInspection,
    GroupPanelSynthesisPreparedReceipt, GroupPanelSynthesisRecord, GroupPanelSynthesisSource,
};

use crate::{
    group_analysis_panel_service::GroupAnalysisPanelService,
    group_model_analysis_codec::{canonical_json_bytes, digest_hex},
    group_model_analysis_error::GroupAnalysisPanelServiceError,
};

use super::error::GroupPanelSynthesisServiceError;

pub(super) struct ValidatedPanel {
    pub(super) inspection: GroupAnalysisPanelInspection,
    pub(super) manifest_json: String,
    pub(super) source: GroupPanelSynthesisSource,
}

pub(super) fn validate_panel(
    panels: &GroupAnalysisPanelService,
    panel_id: &str,
) -> Result<ValidatedPanel, GroupPanelSynthesisServiceError> {
    let inspection = panels.inspect(panel_id).map_err(map_panel_error)?;
    let bytes = canonical_json_bytes(&inspection.manifest)
        .map_err(|_| GroupPanelSynthesisServiceError::InvalidSource)?;
    let panel = &inspection.panel;
    let valid = panel.panel_id == panel_id
        && panel.manifest_bytes == bytes.len()
        && panel.manifest_sha256 == digest_hex(GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, &bytes);
    if !valid {
        return Err(GroupPanelSynthesisServiceError::InvalidSource);
    }
    let source = source_for(&inspection);
    source
        .validate()
        .map_err(|_| GroupPanelSynthesisServiceError::InvalidSource)?;
    let manifest_json =
        String::from_utf8(bytes).map_err(|_| GroupPanelSynthesisServiceError::InvalidSource)?;
    Ok(ValidatedPanel {
        inspection,
        manifest_json,
        source,
    })
}

pub(super) fn validate_source_binding(
    synthesis: &GroupPanelSynthesisRecord,
    prepared: &GroupPanelSynthesisPreparedReceipt,
    panel: &ValidatedPanel,
) -> Result<(), GroupPanelSynthesisServiceError> {
    let actual = &panel.inspection.panel;
    let valid = synthesis.v == GROUP_PANEL_SYNTHESIS_VERSION
        && synthesis.protocol_version == GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION
        && synthesis.panel_id == actual.panel_id
        && synthesis.group_run_id == actual.group_run_id
        && synthesis.source_snapshot_sha256 == actual.source_snapshot_sha256
        && synthesis.panel_manifest_sha256 == actual.manifest_sha256
        && prepared.source == panel.source;
    valid
        .then_some(())
        .ok_or(GroupPanelSynthesisServiceError::InconsistentStoreResult)
}

fn source_for(inspection: &GroupAnalysisPanelInspection) -> GroupPanelSynthesisSource {
    let panel = &inspection.panel;
    let panel_source = &inspection.manifest.source;
    GroupPanelSynthesisSource {
        panel_version: panel.v,
        panel_id: panel.panel_id.clone(),
        group_run_id: panel.group_run_id.clone(),
        group_id: panel_source.group_id.clone(),
        source_snapshot_sha256: panel.source_snapshot_sha256.clone(),
        panel_manifest_sha256: panel.manifest_sha256.clone(),
        panel_manifest_bytes: panel.manifest_bytes,
        analysis_count: panel.analysis_count,
    }
}

fn map_panel_error(error: GroupAnalysisPanelServiceError) -> GroupPanelSynthesisServiceError {
    match error {
        GroupAnalysisPanelServiceError::InvalidInput => {
            GroupPanelSynthesisServiceError::InvalidInput
        }
        GroupAnalysisPanelServiceError::InvalidSource => {
            GroupPanelSynthesisServiceError::InvalidSource
        }
        GroupAnalysisPanelServiceError::InconsistentStoreResult => {
            GroupPanelSynthesisServiceError::InconsistentStoreResult
        }
        GroupAnalysisPanelServiceError::Store(error) => {
            GroupPanelSynthesisServiceError::Store(error)
        }
    }
}
