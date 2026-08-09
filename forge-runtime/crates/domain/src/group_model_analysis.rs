use serde::{Deserialize, Serialize};

use crate::{HubStoreError, Usage};

#[path = "group_model_analysis_journal.rs"]
mod journal;
#[path = "group_model_analysis_validation.rs"]
mod validation;
pub use validation::endpoint_allowed;

pub const GROUP_MODEL_ANALYSIS_VERSION: u16 = 1;
pub const GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_MODEL_ANALYSIS_RESULT_VERSION: u16 = 1;
pub const GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION: u16 = 1;
pub const GROUP_MODEL_ANALYSIS_CONSENT_VERSION: u16 = 1;
pub const GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT: &str = "https://api.openai.com/v1/responses";
pub const GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN: &[u8] =
    b"forge.group-model-analysis-config.v1\0";
pub const GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forge.group-model-analysis-request.v1\0";
pub const GROUP_MODEL_ANALYSIS_EVENT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-model-analysis-event.v1\0";
pub const GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-model-analysis-result.v1\0";
pub const GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-model-analysis-system-prompt.v1\0";
pub const MAX_GROUP_MODEL_ANALYSIS_ID_BYTES: usize = 128;
pub const MAX_GROUP_MODEL_ANALYSIS_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_GROUP_MODEL_ANALYSIS_LIST_LIMIT: usize = 100;
pub const MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES: usize = 128;
pub const MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS: u32 = 32_768;
pub const MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS: u32 = 4_096;
pub const MAX_GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_MODEL_ANALYSIS_CONFIG_JSON_BYTES: usize = 128 * 1024;
pub const MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES: usize = 512 * 1024;
pub const MAX_GROUP_MODEL_ANALYSIS_EVENT_JSON_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_MODEL_ANALYSIS_CURSOR_JSON_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_MODEL_ANALYSIS_EVENTS: usize = 3;
pub const MAX_GROUP_MODEL_ANALYSIS_JOURNAL_BYTES: usize = 192 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupModelAnalysisProvider {
    OpenAiResponses,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisConfig {
    pub v: u16,
    pub provider: GroupModelAnalysisProvider,
    pub endpoint: String,
    pub model: String,
    pub system_prompt_version: u16,
    pub system_prompt_sha256: String,
    pub max_output_tokens: u32,
    pub max_model_output_bytes: usize,
    pub max_model_events: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisRequestConfig {
    pub v: u16,
    pub provider: GroupModelAnalysisProvider,
    pub endpoint: String,
    pub model: String,
    pub system_prompt_version: u16,
    pub system_prompt: String,
    pub max_output_tokens: u32,
    pub max_model_output_bytes: usize,
    pub max_model_events: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisSource {
    pub group_run_version: u16,
    pub group_run_id: String,
    pub group_id: String,
    pub context_version: u16,
    pub context_slice_sha256: String,
    pub snapshot_sha256: String,
    pub snapshot_bytes: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupModelAnalysisStatus {
    AwaitingConsent,
    DispatchUnknown,
    Completed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisRecord {
    pub v: u16,
    pub analysis_id: String,
    pub group_run_id: String,
    pub status: GroupModelAnalysisStatus,
    pub source_snapshot_sha256: String,
    pub config: GroupModelAnalysisConfig,
    pub config_sha256: String,
    pub request_sha256: String,
    pub request_bytes: usize,
    pub protocol_version: u16,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupModelAnalysis {
    pub v: u16,
    pub analysis_id: String,
    pub source: GroupModelAnalysisSource,
    pub request_config: GroupModelAnalysisRequestConfig,
    pub config: GroupModelAnalysisConfig,
    pub config_json: String,
    pub config_sha256: String,
    pub request_body: Vec<u8>,
    pub request_sha256: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareGroupModelAnalysisDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupModelAnalysisResult {
    pub v: u16,
    pub disposition: PrepareGroupModelAnalysisDisposition,
    pub inspection: GroupModelAnalysisInspection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisPreparedReceipt {
    pub v: u16,
    pub analysis_id: String,
    pub source: GroupModelAnalysisSource,
    pub config_sha256: String,
    pub request_sha256: String,
    pub request_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimGroupModelAnalysisDispatch {
    pub v: u16,
    pub analysis_id: String,
    pub dispatch_id: String,
    pub consent_version: u16,
    pub released_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisDispatchClaim {
    pub v: u16,
    pub analysis_id: String,
    pub dispatch_id: String,
    pub request_sha256: String,
    pub config_sha256: String,
    pub provider: GroupModelAnalysisProvider,
    pub endpoint: String,
    pub model: String,
    pub consent_version: u16,
    pub released_at_ms: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GroupModelAnalysisDispatchAuthority {
    v: u16,
    claim: GroupModelAnalysisDispatchClaim,
    request_body: Vec<u8>,
}

impl GroupModelAnalysisDispatchAuthority {
    /// Creates the one capability that authorizes dispatch of persisted bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the claim or body disagrees with the record.
    pub fn new(
        record: &GroupModelAnalysisRecord,
        claim: GroupModelAnalysisDispatchClaim,
        request_body: Vec<u8>,
    ) -> Result<Self, GroupModelAnalysisJournalError> {
        validation::validate_claim(record, &claim)?;
        if request_body.len() != record.request_bytes
            || request_body.is_empty()
            || request_body.len() > MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES
            || validation::request_digest_hex(&request_body) != record.request_sha256
        {
            return Err(validation::analysis_error(
                "dispatch authority body does not match its record",
            ));
        }
        Ok(Self {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            claim,
            request_body,
        })
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.v
    }

    #[must_use]
    pub const fn claim(&self) -> &GroupModelAnalysisDispatchClaim {
        &self.claim
    }

    /// Consumes the single-use dispatch capability and releases its exact bytes.
    #[must_use]
    pub fn into_parts(self) -> (GroupModelAnalysisDispatchClaim, Vec<u8>) {
        (self.claim, self.request_body)
    }
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ClaimGroupModelAnalysisDispatchResult {
    Claimed {
        authority: GroupModelAnalysisDispatchAuthority,
    },
    AlreadyClaimed {
        inspection: GroupModelAnalysisInspection,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupModelAnalysisOutcome {
    Completed,
    Length,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisResult {
    pub v: u16,
    pub analysis_id: String,
    pub dispatch_id: String,
    pub request_sha256: String,
    pub outcome: GroupModelAnalysisOutcome,
    pub answer: String,
    pub usage: Usage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisResultArtifact {
    pub result: GroupModelAnalysisResult,
    pub result_sha256: String,
    pub result_bytes: usize,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteGroupModelAnalysis {
    pub v: u16,
    pub artifact: GroupModelAnalysisResultArtifact,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompleteGroupModelAnalysisDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteGroupModelAnalysisResult {
    pub v: u16,
    pub disposition: CompleteGroupModelAnalysisDisposition,
    pub inspection: GroupModelAnalysisInspection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisResultReceipt {
    pub v: u16,
    pub analysis_id: String,
    pub dispatch_id: String,
    pub request_sha256: String,
    pub outcome: GroupModelAnalysisOutcome,
    pub result_sha256: String,
    pub result_bytes: usize,
    pub usage: Usage,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisEvent {
    pub v: u16,
    pub analysis_id: String,
    pub seq: u64,
    #[serde(flatten)]
    pub kind: GroupModelAnalysisEventKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupModelAnalysisEventKind {
    AnalysisPrepared {
        receipt: GroupModelAnalysisPreparedReceipt,
    },
    ProviderDispatchReleased {
        claim: GroupModelAnalysisDispatchClaim,
    },
    AnalysisCompleted {
        receipt: GroupModelAnalysisResultReceipt,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GroupModelAnalysisRecovery {
    Unprepared,
    AwaitingConsent,
    DispatchUnknown { dispatch_id: String },
    Terminal { outcome: GroupModelAnalysisOutcome },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisInspection {
    pub v: u16,
    pub analysis: GroupModelAnalysisRecord,
    pub events: Vec<GroupModelAnalysisEvent>,
    pub recovery: GroupModelAnalysisRecovery,
    pub prepared: Option<GroupModelAnalysisPreparedReceipt>,
    pub dispatch: Option<GroupModelAnalysisDispatchClaim>,
    pub completion: Option<GroupModelAnalysisResultReceipt>,
    pub result: Option<GroupModelAnalysisResultArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupModelAnalysisJournalError {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupModelAnalysisJournalCursor {
    v: u16,
    analysis_id: String,
    group_run_id: String,
    source_snapshot_sha256: String,
    config: GroupModelAnalysisConfig,
    config_sha256: String,
    request_sha256: String,
    request_bytes: usize,
    protocol_version: u16,
    created_at_ms: u64,
    next_sequence: u64,
    state: GroupModelAnalysisJournalState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum GroupModelAnalysisJournalState {
    NeedPreparation,
    AwaitingConsent(GroupModelAnalysisPreparedReceipt),
    DispatchUnknown {
        prepared: GroupModelAnalysisPreparedReceipt,
        claim: GroupModelAnalysisDispatchClaim,
    },
    Terminal {
        prepared: GroupModelAnalysisPreparedReceipt,
        claim: GroupModelAnalysisDispatchClaim,
        completion: GroupModelAnalysisResultReceipt,
    },
}

pub trait GroupModelAnalysisStore: Send + Sync {
    /// Prepares or exactly replays one local-only analysis.
    ///
    /// # Errors
    ///
    /// Returns a store error when validation, persistence, or replay checks fail.
    fn prepare_group_model_analysis(
        &self,
        request: &PrepareGroupModelAnalysis,
    ) -> Result<PrepareGroupModelAnalysisResult, HubStoreError>;

    /// Claims the one off-machine dispatch capability.
    ///
    /// # Errors
    ///
    /// Returns a store error when validation or the atomic claim fails.
    fn claim_group_model_analysis_dispatch(
        &self,
        request: &ClaimGroupModelAnalysisDispatch,
    ) -> Result<ClaimGroupModelAnalysisDispatchResult, HubStoreError>;

    /// Persists one terminal provider result.
    ///
    /// # Errors
    ///
    /// Returns a store error when validation, binding, or persistence fails.
    fn complete_group_model_analysis(
        &self,
        request: &CompleteGroupModelAnalysis,
    ) -> Result<CompleteGroupModelAnalysisResult, HubStoreError>;

    /// Fully validates and returns one durable analysis.
    ///
    /// # Errors
    ///
    /// Returns a store error when the analysis is absent or corrupt.
    fn inspect_group_model_analysis(
        &self,
        analysis_id: &str,
    ) -> Result<GroupModelAnalysisInspection, HubStoreError>;

    /// Lists bounded analysis metadata.
    ///
    /// # Errors
    ///
    /// Returns a store error when the query or stored metadata is invalid.
    fn list_group_model_analyses(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupModelAnalysisRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupModelAnalysisJournalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupModelAnalysisJournalError {}
