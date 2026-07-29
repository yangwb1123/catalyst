mod codec;
mod complete;
mod read;
mod rows;
mod sql;
mod write;

use crate::{
    openai_responses,
    runtime_domain::{
        Cancellation, ClaimGroupModelAnalysisDispatch, ClaimGroupModelAnalysisDispatchResult,
        CompleteGroupModelAnalysis, CompleteGroupModelAnalysisDisposition,
        CompleteGroupModelAnalysisResult, GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN,
        GROUP_MODEL_ANALYSIS_EVENT_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT, GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN,
        GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_RESULT_VERSION,
        GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_DIGEST_DOMAIN,
        GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION, GROUP_MODEL_ANALYSIS_VERSION,
        GroupModelAnalysisConfig, GroupModelAnalysisDispatchAuthority,
        GroupModelAnalysisDispatchClaim, GroupModelAnalysisEvent, GroupModelAnalysisEventKind,
        GroupModelAnalysisInspection, GroupModelAnalysisJournalCursor,
        GroupModelAnalysisPreparedReceipt, GroupModelAnalysisProvider, GroupModelAnalysisRecord,
        GroupModelAnalysisRecovery, GroupModelAnalysisRequestConfig, GroupModelAnalysisResult,
        GroupModelAnalysisResultArtifact, GroupModelAnalysisResultReceipt,
        GroupModelAnalysisSource, GroupModelAnalysisStatus, GroupRunSnapshot, HubEntity,
        HubStoreError, MAX_GROUP_MODEL_ANALYSIS_CONFIG_JSON_BYTES,
        MAX_GROUP_MODEL_ANALYSIS_CURSOR_JSON_BYTES, MAX_GROUP_MODEL_ANALYSIS_EVENT_JSON_BYTES,
        MAX_GROUP_MODEL_ANALYSIS_EVENTS, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES,
        MAX_GROUP_MODEL_ANALYSIS_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_MODEL_ANALYSIS_JOURNAL_BYTES,
        MAX_GROUP_MODEL_ANALYSIS_LIST_LIMIT, MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES,
        MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS, MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES,
        MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS, MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES,
        MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES, MAX_GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_BYTES,
        Message, ModelRequest, PrepareGroupModelAnalysis, PrepareGroupModelAnalysisDisposition,
        PrepareGroupModelAnalysisResult,
    },
};

use super::{
    SqliteHubStore, group_context_build, group_run_codec, group_run_read, read_error, write_error,
};

impl crate::runtime_domain::GroupModelAnalysisStore for SqliteHubStore {
    fn prepare_group_model_analysis(
        &self,
        request: &PrepareGroupModelAnalysis,
    ) -> Result<PrepareGroupModelAnalysisResult, HubStoreError> {
        write::prepare(&mut self.connect()?, request)
    }

    fn claim_group_model_analysis_dispatch(
        &self,
        request: &ClaimGroupModelAnalysisDispatch,
    ) -> Result<ClaimGroupModelAnalysisDispatchResult, HubStoreError> {
        write::claim(&mut self.connect()?, request)
    }

    fn complete_group_model_analysis(
        &self,
        request: &CompleteGroupModelAnalysis,
    ) -> Result<CompleteGroupModelAnalysisResult, HubStoreError> {
        complete::complete(&mut self.connect()?, request)
    }

    fn inspect_group_model_analysis(
        &self,
        analysis_id: &str,
    ) -> Result<GroupModelAnalysisInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, analysis_id)
    }

    fn list_group_model_analyses(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupModelAnalysisRecord>, HubStoreError> {
        read::list(&self.connect()?, group_run_id, limit)
    }
}
