use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatch, ClaimGroupModelAnalysisDispatchResult,
    CompleteGroupModelAnalysis, CompleteGroupModelAnalysisResult,
    GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, GROUP_ANALYSIS_PANEL_VERSION,
    GroupAnalysisPanelInspection, GroupAnalysisPanelRecord, GroupAnalysisPanelStatus,
    GroupAnalysisPanelStore, GroupModelAnalysisInspection, GroupModelAnalysisRecord,
    GroupModelAnalysisStore, GroupRunRecord, GroupRunSnapshot, GroupRunStore, HubEntity,
    HubStoreError, PrepareGroupAnalysisPanel, PrepareGroupAnalysisPanelDisposition,
    PrepareGroupAnalysisPanelResult, PrepareGroupModelAnalysis, PrepareGroupModelAnalysisResult,
    PrepareGroupRun, PrepareGroupRunResult,
};

use super::fixture::{canonical, digest};

pub(crate) struct MemoryRunStore {
    snapshots: BTreeMap<String, GroupRunSnapshot>,
}

impl MemoryRunStore {
    pub(crate) fn new(snapshots: impl IntoIterator<Item = GroupRunSnapshot>) -> Self {
        let snapshots = snapshots
            .into_iter()
            .map(|snapshot| (snapshot.run.run_id.clone(), snapshot))
            .collect();
        Self { snapshots }
    }
}

impl GroupRunStore for MemoryRunStore {
    fn prepare_group_run(
        &self,
        _request: &PrepareGroupRun,
    ) -> Result<PrepareGroupRunResult, HubStoreError> {
        Err(unavailable("run preparation is not used"))
    }

    fn inspect_group_run(&self, run_id: &str) -> Result<GroupRunSnapshot, HubStoreError> {
        self.snapshots
            .get(run_id)
            .cloned()
            .ok_or_else(|| not_found(HubEntity::GroupRun, run_id))
    }

    fn list_group_runs(
        &self,
        group_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupRunRecord>, HubStoreError> {
        Ok(self
            .snapshots
            .values()
            .filter(|snapshot| group_id.is_none_or(|id| snapshot.run.group_id == id))
            .take(limit)
            .map(|snapshot| snapshot.run.clone())
            .collect())
    }
}

pub(crate) struct MemoryAnalysisStore {
    inspections: Mutex<BTreeMap<String, GroupModelAnalysisInspection>>,
}

impl MemoryAnalysisStore {
    pub(crate) fn new(inspections: impl IntoIterator<Item = GroupModelAnalysisInspection>) -> Self {
        let inspections = inspections
            .into_iter()
            .map(|inspection| (inspection.analysis.analysis_id.clone(), inspection))
            .collect();
        Self {
            inspections: Mutex::new(inspections),
        }
    }

    pub(crate) fn respond_with(
        &self,
        requested_id: &str,
        inspection: GroupModelAnalysisInspection,
    ) {
        self.lock().insert(requested_id.into(), inspection);
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, GroupModelAnalysisInspection>> {
        self.inspections.lock().expect("analysis inspections")
    }
}

impl GroupModelAnalysisStore for MemoryAnalysisStore {
    fn prepare_group_model_analysis(
        &self,
        _request: &PrepareGroupModelAnalysis,
    ) -> Result<PrepareGroupModelAnalysisResult, HubStoreError> {
        Err(unavailable("analysis preparation is not used"))
    }

    fn claim_group_model_analysis_dispatch(
        &self,
        _request: &ClaimGroupModelAnalysisDispatch,
    ) -> Result<ClaimGroupModelAnalysisDispatchResult, HubStoreError> {
        Err(unavailable("analysis dispatch is not used"))
    }

    fn complete_group_model_analysis(
        &self,
        _request: &CompleteGroupModelAnalysis,
    ) -> Result<CompleteGroupModelAnalysisResult, HubStoreError> {
        Err(unavailable("analysis completion is not used"))
    }

    fn inspect_group_model_analysis(
        &self,
        analysis_id: &str,
    ) -> Result<GroupModelAnalysisInspection, HubStoreError> {
        self.lock()
            .get(analysis_id)
            .cloned()
            .ok_or_else(|| not_found(HubEntity::GroupModelAnalysis, analysis_id))
    }

    fn list_group_model_analyses(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupModelAnalysisRecord>, HubStoreError> {
        Ok(self
            .lock()
            .values()
            .filter(|value| group_run_id.is_none_or(|id| value.analysis.group_run_id == id))
            .take(limit)
            .map(|value| value.analysis.clone())
            .collect())
    }
}

#[derive(Default)]
struct PanelState {
    panels: BTreeMap<String, GroupAnalysisPanelInspection>,
    last_request: Option<PrepareGroupAnalysisPanel>,
    corrupt_next_prepare: bool,
    list_override: Option<Vec<GroupAnalysisPanelRecord>>,
}

#[derive(Default)]
pub(crate) struct MemoryPanelStore {
    state: Mutex<PanelState>,
}

impl MemoryPanelStore {
    pub(crate) fn last_request(&self) -> PrepareGroupAnalysisPanel {
        self.lock()
            .last_request
            .clone()
            .expect("panel request was captured")
    }

    pub(crate) fn corrupt_next_prepare(&self) {
        self.lock().corrupt_next_prepare = true;
    }

    pub(crate) fn set_list_override(&self, records: Vec<GroupAnalysisPanelRecord>) {
        self.lock().list_override = Some(records);
    }

    fn lock(&self) -> MutexGuard<'_, PanelState> {
        self.state.lock().expect("panel state")
    }
}

impl GroupAnalysisPanelStore for MemoryPanelStore {
    fn prepare_group_analysis_panel(
        &self,
        request: &PrepareGroupAnalysisPanel,
    ) -> Result<PrepareGroupAnalysisPanelResult, HubStoreError> {
        let mut state = self.lock();
        state.last_request = Some(request.clone());
        let mut inspection = panel_inspection(request)?;
        if state.corrupt_next_prepare {
            inspection.panel.manifest_sha256 = "0".repeat(64);
            state.corrupt_next_prepare = false;
        }
        state
            .panels
            .insert(request.panel_id.clone(), inspection.clone());
        Ok(PrepareGroupAnalysisPanelResult {
            v: GROUP_ANALYSIS_PANEL_VERSION,
            disposition: PrepareGroupAnalysisPanelDisposition::Created,
            inspection,
        })
    }

    fn inspect_group_analysis_panel(
        &self,
        panel_id: &str,
    ) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
        self.lock()
            .panels
            .get(panel_id)
            .cloned()
            .ok_or_else(|| not_found(HubEntity::GroupAnalysisPanel, panel_id))
    }

    fn list_group_analysis_panels(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAnalysisPanelRecord>, HubStoreError> {
        let state = self.lock();
        if let Some(records) = &state.list_override {
            return Ok(records.clone());
        }
        Ok(state
            .panels
            .values()
            .filter(|value| group_run_id.is_none_or(|id| value.panel.group_run_id == id))
            .take(limit)
            .map(|value| value.panel.clone())
            .collect())
    }
}

fn panel_inspection(
    request: &PrepareGroupAnalysisPanel,
) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
    let bytes = canonical(&request.manifest).map_err(|error| unavailable(&error.to_string()))?;
    let panel = GroupAnalysisPanelRecord {
        v: GROUP_ANALYSIS_PANEL_VERSION,
        panel_id: request.panel_id.clone(),
        group_run_id: request.manifest.source.group_run_id.clone(),
        status: GroupAnalysisPanelStatus::Prepared,
        source_snapshot_sha256: request.manifest.source.snapshot_sha256.clone(),
        manifest_sha256: digest(GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, &bytes),
        manifest_bytes: bytes.len(),
        analysis_count: request.manifest.contributions.len(),
        created_at_ms: request.created_at_ms,
    };
    Ok(GroupAnalysisPanelInspection {
        v: GROUP_ANALYSIS_PANEL_VERSION,
        panel,
        manifest: request.manifest.clone(),
    })
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}
