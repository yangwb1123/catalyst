#[cfg(test)]
#[path = "tests/group_panel_synthesis_atomicity.rs"]
mod atomicity_tests;
mod codec;
mod complete;
pub(super) mod read;
mod rows;
mod sql;
mod write;

use crate::{
    openai_responses,
    runtime_domain::{
        Cancellation, ClaimGroupPanelSynthesisDispatch, ClaimGroupPanelSynthesisDispatchResult,
        CompleteGroupPanelSynthesis, CompleteGroupPanelSynthesisDisposition,
        CompleteGroupPanelSynthesisResult, GROUP_PANEL_SYNTHESIS_CONFIG_DIGEST_DOMAIN,
        GROUP_PANEL_SYNTHESIS_EVENT_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT, GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN,
        GROUP_PANEL_SYNTHESIS_RESULT_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
        GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_DIGEST_DOMAIN,
        GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION, GROUP_PANEL_SYNTHESIS_VERSION,
        GroupAnalysisPanelInspection, GroupPanelSynthesisConfig,
        GroupPanelSynthesisDispatchAuthority, GroupPanelSynthesisDispatchClaim,
        GroupPanelSynthesisEvent, GroupPanelSynthesisEventKind, GroupPanelSynthesisInspection,
        GroupPanelSynthesisJournalCursor, GroupPanelSynthesisOutputTarget,
        GroupPanelSynthesisPreparedReceipt, GroupPanelSynthesisProvider, GroupPanelSynthesisRecord,
        GroupPanelSynthesisRecovery, GroupPanelSynthesisRequestConfig, GroupPanelSynthesisResult,
        GroupPanelSynthesisResultArtifact, GroupPanelSynthesisResultReceipt,
        GroupPanelSynthesisSource, GroupPanelSynthesisStatus, GroupPanelSynthesisWritebackTarget,
        HubEntity, HubStoreError, MAX_GROUP_PANEL_SYNTHESIS_CONFIG_JSON_BYTES,
        MAX_GROUP_PANEL_SYNTHESIS_CURSOR_JSON_BYTES, MAX_GROUP_PANEL_SYNTHESIS_EVENT_JSON_BYTES,
        MAX_GROUP_PANEL_SYNTHESIS_EVENTS, MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES,
        MAX_GROUP_PANEL_SYNTHESIS_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_PANEL_SYNTHESIS_JOURNAL_BYTES,
        MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT, MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES,
        MAX_GROUP_PANEL_SYNTHESIS_MODEL_EVENTS, MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES,
        MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS, MAX_GROUP_PANEL_SYNTHESIS_REQUEST_BYTES,
        MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES, MAX_GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_BYTES,
        Message, ModelRequest, PrepareGroupPanelSynthesis, PrepareGroupPanelSynthesisDisposition,
        PrepareGroupPanelSynthesisResult,
    },
};

use super::{
    SqliteHubStore, group_analysis_panel, group_context_build, group_run_codec, read_error,
    write_error,
};

impl crate::runtime_domain::GroupPanelSynthesisStore for SqliteHubStore {
    fn prepare_group_panel_synthesis(
        &self,
        request: &PrepareGroupPanelSynthesis,
    ) -> Result<PrepareGroupPanelSynthesisResult, HubStoreError> {
        write::prepare(&mut self.connect()?, request)
    }

    fn claim_group_panel_synthesis_dispatch(
        &self,
        request: &ClaimGroupPanelSynthesisDispatch,
    ) -> Result<ClaimGroupPanelSynthesisDispatchResult, HubStoreError> {
        write::claim(&mut self.connect()?, request)
    }

    fn complete_group_panel_synthesis(
        &self,
        request: &CompleteGroupPanelSynthesis,
    ) -> Result<CompleteGroupPanelSynthesisResult, HubStoreError> {
        complete::complete(&mut self.connect()?, request)
    }

    fn inspect_group_panel_synthesis(
        &self,
        synthesis_id: &str,
    ) -> Result<GroupPanelSynthesisInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, synthesis_id)
    }

    fn list_group_panel_syntheses(
        &self,
        panel_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupPanelSynthesisRecord>, HubStoreError> {
        read::list(&self.connect()?, panel_id, limit)
    }
}
