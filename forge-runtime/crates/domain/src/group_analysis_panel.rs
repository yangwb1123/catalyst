use serde::{Deserialize, Serialize};

use crate::{
    GroupModelAnalysisRecord, GroupModelAnalysisResultArtifact, GroupModelAnalysisSource,
    HubStoreError,
};

#[path = "group_analysis_panel_validation.rs"]
mod validation;

pub const GROUP_ANALYSIS_PANEL_VERSION: u16 = 1;
pub const GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN: &[u8] =
    b"forge.group-analysis-panel-manifest.v1\0";
pub const MIN_GROUP_ANALYSIS_PANEL_ANALYSES: usize = 2;
pub const MAX_GROUP_ANALYSIS_PANEL_ANALYSES: usize = 8;
pub const MAX_GROUP_ANALYSIS_PANEL_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT: usize = 100;
pub const MAX_GROUP_ANALYSIS_PANEL_MANIFEST_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAnalysisPanelStatus {
    Prepared,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupAnalysisPanelContribution {
    pub analysis: GroupModelAnalysisRecord,
    pub result: GroupModelAnalysisResultArtifact,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupAnalysisPanelManifest {
    pub v: u16,
    pub source: GroupModelAnalysisSource,
    pub contributions: Vec<GroupAnalysisPanelContribution>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupAnalysisPanelRecord {
    pub v: u16,
    pub panel_id: String,
    pub group_run_id: String,
    pub status: GroupAnalysisPanelStatus,
    pub source_snapshot_sha256: String,
    pub manifest_sha256: String,
    pub manifest_bytes: usize,
    pub analysis_count: usize,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAnalysisPanel {
    pub v: u16,
    pub panel_id: String,
    pub manifest: GroupAnalysisPanelManifest,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareGroupAnalysisPanelDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAnalysisPanelResult {
    pub v: u16,
    pub disposition: PrepareGroupAnalysisPanelDisposition,
    pub inspection: GroupAnalysisPanelInspection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupAnalysisPanelInspection {
    pub v: u16,
    pub panel: GroupAnalysisPanelRecord,
    pub manifest: GroupAnalysisPanelManifest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAnalysisPanelValidationError {
    pub message: String,
}

impl GroupAnalysisPanelManifest {
    /// Validates the ordered, complete analysis contribution set.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, duplicate, incomplete, or cross-source contributions.
    pub fn validate(&self) -> Result<(), GroupAnalysisPanelValidationError> {
        validation::validate_manifest(self)
    }
}

impl GroupAnalysisPanelRecord {
    /// Validates content-free durable panel metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when a field violates the versioned contract.
    pub fn validate(&self) -> Result<(), GroupAnalysisPanelValidationError> {
        validation::validate_record(self)
    }
}

impl PrepareGroupAnalysisPanel {
    /// Validates one local-only panel preparation request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, manifest, key, or time.
    pub fn validate(&self) -> Result<(), GroupAnalysisPanelValidationError> {
        validation::validate_prepare(self)
    }
}

impl GroupAnalysisPanelInspection {
    /// Validates a durable panel record against its copied manifest.
    ///
    /// # Errors
    ///
    /// Returns an error for record, source, count, or manifest divergence.
    pub fn validate(&self) -> Result<(), GroupAnalysisPanelValidationError> {
        validation::validate_inspection(self)
    }
}

pub trait GroupAnalysisPanelStore: Send + Sync {
    /// Atomically freezes an ordered set of completed analysis artifacts.
    ///
    /// # Errors
    ///
    /// Returns a structured error for invalid input, conflicts, corruption, or storage failure.
    fn prepare_group_analysis_panel(
        &self,
        request: &PrepareGroupAnalysisPanel,
    ) -> Result<PrepareGroupAnalysisPanelResult, HubStoreError>;

    /// Fully validates one durable panel and every referenced analysis result.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the panel is missing, corrupt, or unavailable.
    fn inspect_group_analysis_panel(
        &self,
        panel_id: &str,
    ) -> Result<GroupAnalysisPanelInspection, HubStoreError>;

    /// Lists bounded, content-free panel metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured error for invalid filters, corrupt metadata, or storage failure.
    fn list_group_analysis_panels(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAnalysisPanelRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupAnalysisPanelValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAnalysisPanelValidationError {}
