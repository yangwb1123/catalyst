mod codec;
pub(super) mod read;
mod rows;
mod write;

use crate::runtime_domain::{
    GroupAnalysisPanelInspection, GroupAnalysisPanelRecord, GroupAnalysisPanelStore, HubStoreError,
    PrepareGroupAnalysisPanel, PrepareGroupAnalysisPanelResult,
};

use super::SqliteHubStore;

impl GroupAnalysisPanelStore for SqliteHubStore {
    fn prepare_group_analysis_panel(
        &self,
        request: &PrepareGroupAnalysisPanel,
    ) -> Result<PrepareGroupAnalysisPanelResult, HubStoreError> {
        write::prepare(&mut self.connect()?, request)
    }

    fn inspect_group_analysis_panel(
        &self,
        panel_id: &str,
    ) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, panel_id)
    }

    fn list_group_analysis_panels(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAnalysisPanelRecord>, HubStoreError> {
        read::list(&self.connect()?, group_run_id, limit)
    }
}
