use std::sync::Arc;

use forge_runtime_domain::{
    GROUP_ANALYSIS_PANEL_VERSION, GroupAnalysisPanelContribution, GroupAnalysisPanelInspection,
    GroupAnalysisPanelManifest, GroupAnalysisPanelRecord, GroupAnalysisPanelStore,
    GroupModelAnalysisStore, GroupRunStore, MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT,
    PrepareGroupAnalysisPanel, PrepareGroupAnalysisPanelResult,
};

use crate::{
    GroupAnalysisPanelServiceError,
    group_analysis_panel_validation::{
        checked_contribution, checked_inspection, validate_input, validate_list,
        validate_prepare_result,
    },
    group_model_analysis_source::validate_source,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAnalysisPanelInput {
    pub panel_id: String,
    pub group_run_id: String,
    pub analysis_ids: Vec<String>,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

pub struct GroupAnalysisPanelService {
    group_runs: Arc<dyn GroupRunStore>,
    analyses: Arc<dyn GroupModelAnalysisStore>,
    panels: Arc<dyn GroupAnalysisPanelStore>,
}

impl GroupAnalysisPanelService {
    #[must_use]
    pub fn new(
        group_runs: Arc<dyn GroupRunStore>,
        analyses: Arc<dyn GroupModelAnalysisStore>,
        panels: Arc<dyn GroupAnalysisPanelStore>,
    ) -> Self {
        Self {
            group_runs,
            analyses,
            panels,
        }
    }

    /// Freezes an ordered set of completed local analysis artifacts.
    ///
    /// # Errors
    ///
    /// Returns validation, source-integrity, store-consistency, or storage errors.
    pub fn prepare(
        &self,
        input: &PrepareGroupAnalysisPanelInput,
    ) -> Result<PrepareGroupAnalysisPanelResult, GroupAnalysisPanelServiceError> {
        validate_input(input)?;
        let snapshot = self.group_runs.inspect_group_run(&input.group_run_id)?;
        let source = validate_source(&snapshot, &input.group_run_id)
            .map_err(|_| GroupAnalysisPanelServiceError::InvalidSource)?;
        let contributions = input
            .analysis_ids
            .iter()
            .map(|id| checked_contribution(self.analyses.as_ref(), id, &snapshot, &source))
            .collect::<Result<Vec<_>, _>>()?;
        let request = PrepareGroupAnalysisPanel {
            v: GROUP_ANALYSIS_PANEL_VERSION,
            panel_id: input.panel_id.clone(),
            manifest: GroupAnalysisPanelManifest {
                v: GROUP_ANALYSIS_PANEL_VERSION,
                source,
                contributions,
            },
            idempotency_key: input.idempotency_key.clone(),
            created_at_ms: input.created_at_ms,
        };
        request
            .validate()
            .map_err(|_| GroupAnalysisPanelServiceError::InvalidInput)?;
        let result = self.panels.prepare_group_analysis_panel(&request)?;
        validate_prepare_result(input, &request, result)
    }

    /// Loads a panel and independently revalidates its source analyses.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, corruption, inconsistency, or storage failure.
    pub fn inspect(
        &self,
        panel_id: &str,
    ) -> Result<GroupAnalysisPanelInspection, GroupAnalysisPanelServiceError> {
        crate::group_model_analysis_validation::validate_identifier(panel_id)
            .map_err(|_| GroupAnalysisPanelServiceError::InvalidInput)?;
        let inspection = checked_inspection(self.panels.inspect_group_analysis_panel(panel_id)?)?;
        if inspection.panel.panel_id != panel_id {
            return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
        }
        self.revalidate_sources(&inspection)?;
        Ok(inspection)
    }

    /// Lists bounded panel metadata without loading copied result bodies.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid filters, inconsistent metadata, or storage failure.
    pub fn list(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAnalysisPanelRecord>, GroupAnalysisPanelServiceError> {
        if !(1..=MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT).contains(&limit) {
            return Err(GroupAnalysisPanelServiceError::InvalidInput);
        }
        if let Some(id) = group_run_id {
            crate::group_model_analysis_validation::validate_identifier(id)
                .map_err(|_| GroupAnalysisPanelServiceError::InvalidInput)?;
        }
        let records = self
            .panels
            .list_group_analysis_panels(group_run_id, limit)?;
        validate_list(&records, group_run_id, limit)?;
        Ok(records)
    }

    fn revalidate_sources(
        &self,
        inspection: &GroupAnalysisPanelInspection,
    ) -> Result<(), GroupAnalysisPanelServiceError> {
        let run_id = &inspection.panel.group_run_id;
        let snapshot = self.group_runs.inspect_group_run(run_id)?;
        let source = validate_source(&snapshot, run_id)
            .map_err(|_| GroupAnalysisPanelServiceError::InvalidSource)?;
        if source != inspection.manifest.source {
            return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
        }
        self.revalidate_contributions(&inspection.manifest.contributions, &snapshot, &source)
    }

    fn revalidate_contributions(
        &self,
        expected: &[GroupAnalysisPanelContribution],
        snapshot: &forge_runtime_domain::GroupRunSnapshot,
        source: &forge_runtime_domain::GroupModelAnalysisSource,
    ) -> Result<(), GroupAnalysisPanelServiceError> {
        for contribution in expected {
            let actual = checked_contribution(
                self.analyses.as_ref(),
                &contribution.analysis.analysis_id,
                snapshot,
                source,
            )?;
            if actual != *contribution {
                return Err(GroupAnalysisPanelServiceError::InconsistentStoreResult);
            }
        }
        Ok(())
    }
}
