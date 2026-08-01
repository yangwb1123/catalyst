use serde::{Deserialize, Serialize};

use crate::{HubStoreError, Usage};

mod journal;
mod validation;

pub const GROUP_PANEL_SYNTHESIS_VERSION: u16 = 1;
pub const GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_PANEL_SYNTHESIS_RESULT_VERSION: u16 = 1;
pub const GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION: u16 = 1;
pub const GROUP_PANEL_SYNTHESIS_CONSENT_VERSION: u16 = 1;
pub const GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT: &str = "https://api.openai.com/v1/responses";
pub const GROUP_PANEL_SYNTHESIS_CONFIG_DIGEST_DOMAIN: &[u8] =
    b"forge.group-panel-synthesis-config.v1\0";
pub const GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forge.group-panel-synthesis-request.v1\0";
pub const GROUP_PANEL_SYNTHESIS_EVENT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-panel-synthesis-event.v1\0";
pub const GROUP_PANEL_SYNTHESIS_RESULT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-panel-synthesis-result.v1\0";
pub const GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-panel-synthesis-system-prompt.v1\0";
pub const MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES: usize = 128;
pub const MAX_GROUP_PANEL_SYNTHESIS_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT: usize = 100;
pub const MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES: usize = 128;
pub const MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS: u32 = 32_768;
pub const MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_PANEL_SYNTHESIS_MODEL_EVENTS: u32 = 4_096;
pub const MAX_GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_PANEL_SYNTHESIS_CONFIG_JSON_BYTES: usize = 128 * 1024;
pub const MAX_GROUP_PANEL_SYNTHESIS_REQUEST_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES: usize = 512 * 1024;
pub const MAX_GROUP_PANEL_SYNTHESIS_EVENT_JSON_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_PANEL_SYNTHESIS_CURSOR_JSON_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_PANEL_SYNTHESIS_EVENTS: usize = 3;
pub const MAX_GROUP_PANEL_SYNTHESIS_JOURNAL_BYTES: usize = 192 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupPanelSynthesisProvider {
    OpenAiResponses,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupPanelSynthesisOutputTarget {
    LocalArtifact,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupPanelSynthesisWritebackTarget {
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisConfig {
    pub v: u16,
    pub provider: GroupPanelSynthesisProvider,
    pub endpoint: String,
    pub model: String,
    pub system_prompt_version: u16,
    pub system_prompt_sha256: String,
    pub max_output_tokens: u32,
    pub max_model_output_bytes: usize,
    pub max_model_events: u32,
    pub output_target: GroupPanelSynthesisOutputTarget,
    pub writeback_target: GroupPanelSynthesisWritebackTarget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisRequestConfig {
    pub v: u16,
    pub provider: GroupPanelSynthesisProvider,
    pub endpoint: String,
    pub model: String,
    pub system_prompt_version: u16,
    pub system_prompt: String,
    pub max_output_tokens: u32,
    pub max_model_output_bytes: usize,
    pub max_model_events: u32,
    pub output_target: GroupPanelSynthesisOutputTarget,
    pub writeback_target: GroupPanelSynthesisWritebackTarget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisSource {
    pub panel_version: u16,
    pub panel_id: String,
    pub group_run_id: String,
    pub group_id: String,
    pub source_snapshot_sha256: String,
    pub panel_manifest_sha256: String,
    pub panel_manifest_bytes: usize,
    pub analysis_count: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupPanelSynthesisStatus {
    AwaitingConsent,
    DispatchUnknown,
    Completed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisRecord {
    pub v: u16,
    pub synthesis_id: String,
    pub panel_id: String,
    pub group_run_id: String,
    pub status: GroupPanelSynthesisStatus,
    pub source_snapshot_sha256: String,
    pub panel_manifest_sha256: String,
    pub config: GroupPanelSynthesisConfig,
    pub config_sha256: String,
    pub request_sha256: String,
    pub request_bytes: usize,
    pub protocol_version: u16,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupPanelSynthesis {
    pub v: u16,
    pub synthesis_id: String,
    pub source: GroupPanelSynthesisSource,
    pub request_config: GroupPanelSynthesisRequestConfig,
    pub config: GroupPanelSynthesisConfig,
    pub config_json: String,
    pub config_sha256: String,
    pub request_body: Vec<u8>,
    pub request_sha256: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareGroupPanelSynthesisDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupPanelSynthesisResult {
    pub v: u16,
    pub disposition: PrepareGroupPanelSynthesisDisposition,
    pub inspection: GroupPanelSynthesisInspection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisPreparedReceipt {
    pub v: u16,
    pub synthesis_id: String,
    pub source: GroupPanelSynthesisSource,
    pub config_sha256: String,
    pub request_sha256: String,
    pub request_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimGroupPanelSynthesisDispatch {
    pub v: u16,
    pub synthesis_id: String,
    pub dispatch_id: String,
    pub consent_version: u16,
    pub released_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisDispatchClaim {
    pub v: u16,
    pub synthesis_id: String,
    pub dispatch_id: String,
    pub request_sha256: String,
    pub config_sha256: String,
    pub provider: GroupPanelSynthesisProvider,
    pub endpoint: String,
    pub model: String,
    pub consent_version: u16,
    pub released_at_ms: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GroupPanelSynthesisDispatchAuthority {
    v: u16,
    claim: GroupPanelSynthesisDispatchClaim,
    request_body: Vec<u8>,
}

impl GroupPanelSynthesisDispatchAuthority {
    /// Creates the single capability authorizing dispatch of persisted bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the claim or exact body disagrees with the record.
    pub fn new(
        record: &GroupPanelSynthesisRecord,
        claim: GroupPanelSynthesisDispatchClaim,
        request_body: Vec<u8>,
    ) -> Result<Self, GroupPanelSynthesisJournalError> {
        validation::validate_claim(record, &claim)?;
        let valid = request_body.len() == record.request_bytes
            && !request_body.is_empty()
            && request_body.len() <= MAX_GROUP_PANEL_SYNTHESIS_REQUEST_BYTES
            && validation::request_digest_hex(&request_body) == record.request_sha256;
        valid
            .then_some(Self {
                v: GROUP_PANEL_SYNTHESIS_VERSION,
                claim,
                request_body,
            })
            .ok_or_else(|| validation::synthesis_error("dispatch body does not match its record"))
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.v
    }

    #[must_use]
    pub const fn claim(&self) -> &GroupPanelSynthesisDispatchClaim {
        &self.claim
    }

    #[must_use]
    pub fn into_parts(self) -> (GroupPanelSynthesisDispatchClaim, Vec<u8>) {
        (self.claim, self.request_body)
    }
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ClaimGroupPanelSynthesisDispatchResult {
    Claimed {
        authority: GroupPanelSynthesisDispatchAuthority,
    },
    AlreadyClaimed {
        inspection: GroupPanelSynthesisInspection,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupPanelSynthesisOutcome {
    Completed,
    Length,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisResult {
    pub v: u16,
    pub synthesis_id: String,
    pub dispatch_id: String,
    pub request_sha256: String,
    pub outcome: GroupPanelSynthesisOutcome,
    pub answer: String,
    pub usage: Usage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisResultArtifact {
    pub result: GroupPanelSynthesisResult,
    pub result_sha256: String,
    pub result_bytes: usize,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteGroupPanelSynthesis {
    pub v: u16,
    pub artifact: GroupPanelSynthesisResultArtifact,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompleteGroupPanelSynthesisDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteGroupPanelSynthesisResult {
    pub v: u16,
    pub disposition: CompleteGroupPanelSynthesisDisposition,
    pub inspection: GroupPanelSynthesisInspection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisResultReceipt {
    pub v: u16,
    pub synthesis_id: String,
    pub dispatch_id: String,
    pub request_sha256: String,
    pub outcome: GroupPanelSynthesisOutcome,
    pub result_sha256: String,
    pub result_bytes: usize,
    pub usage: Usage,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisEvent {
    pub v: u16,
    pub synthesis_id: String,
    pub seq: u64,
    #[serde(flatten)]
    pub kind: GroupPanelSynthesisEventKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupPanelSynthesisEventKind {
    SynthesisPrepared {
        receipt: GroupPanelSynthesisPreparedReceipt,
    },
    ProviderDispatchReleased {
        claim: GroupPanelSynthesisDispatchClaim,
    },
    SynthesisCompleted {
        receipt: GroupPanelSynthesisResultReceipt,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GroupPanelSynthesisRecovery {
    Unprepared,
    AwaitingConsent,
    DispatchUnknown { dispatch_id: String },
    Terminal { outcome: GroupPanelSynthesisOutcome },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisInspection {
    pub v: u16,
    pub synthesis: GroupPanelSynthesisRecord,
    pub events: Vec<GroupPanelSynthesisEvent>,
    pub recovery: GroupPanelSynthesisRecovery,
    pub prepared: Option<GroupPanelSynthesisPreparedReceipt>,
    pub dispatch: Option<GroupPanelSynthesisDispatchClaim>,
    pub completion: Option<GroupPanelSynthesisResultReceipt>,
    pub result: Option<GroupPanelSynthesisResultArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupPanelSynthesisJournalError {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupPanelSynthesisJournalCursor {
    v: u16,
    synthesis_id: String,
    panel_id: String,
    group_run_id: String,
    source_snapshot_sha256: String,
    panel_manifest_sha256: String,
    config: GroupPanelSynthesisConfig,
    config_sha256: String,
    request_sha256: String,
    request_bytes: usize,
    protocol_version: u16,
    created_at_ms: u64,
    next_sequence: u64,
    state: GroupPanelSynthesisJournalState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum GroupPanelSynthesisJournalState {
    NeedPreparation,
    AwaitingConsent(GroupPanelSynthesisPreparedReceipt),
    DispatchUnknown {
        prepared: GroupPanelSynthesisPreparedReceipt,
        claim: GroupPanelSynthesisDispatchClaim,
    },
    Terminal {
        prepared: GroupPanelSynthesisPreparedReceipt,
        claim: GroupPanelSynthesisDispatchClaim,
        completion: GroupPanelSynthesisResultReceipt,
    },
}

pub trait GroupPanelSynthesisStore: Send + Sync {
    /// Atomically prepares or exactly replays one local-only synthesis request.
    ///
    /// # Errors
    ///
    /// Returns a store error for invalid input, conflict, corruption, or availability failure.
    fn prepare_group_panel_synthesis(
        &self,
        request: &PrepareGroupPanelSynthesis,
    ) -> Result<PrepareGroupPanelSynthesisResult, HubStoreError>;

    /// Atomically claims the sole off-machine dispatch capability.
    ///
    /// # Errors
    ///
    /// Returns a store error when validation or the exclusive claim fails.
    fn claim_group_panel_synthesis_dispatch(
        &self,
        request: &ClaimGroupPanelSynthesisDispatch,
    ) -> Result<ClaimGroupPanelSynthesisDispatchResult, HubStoreError>;

    /// Atomically persists or exactly replays one terminal provider result.
    ///
    /// # Errors
    ///
    /// Returns a store error for invalid binding, conflict, corruption, or availability failure.
    fn complete_group_panel_synthesis(
        &self,
        request: &CompleteGroupPanelSynthesis,
    ) -> Result<CompleteGroupPanelSynthesisResult, HubStoreError>;

    /// Fully validates one durable synthesis and its source panel.
    ///
    /// # Errors
    ///
    /// Returns a store error when the synthesis is missing, corrupt, or unavailable.
    fn inspect_group_panel_synthesis(
        &self,
        synthesis_id: &str,
    ) -> Result<GroupPanelSynthesisInspection, HubStoreError>;

    /// Lists bounded, content-free synthesis metadata.
    ///
    /// # Errors
    ///
    /// Returns a store error for invalid filters, corrupt metadata, or availability failure.
    fn list_group_panel_syntheses(
        &self,
        panel_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupPanelSynthesisRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupPanelSynthesisJournalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupPanelSynthesisJournalError {}
