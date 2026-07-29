use std::sync::Arc;

use forge_runtime_domain::{
    Cancellation, ClaimGroupModelAnalysisDispatch, ClaimGroupModelAnalysisDispatchResult,
    CompleteGroupModelAnalysis, CompleteGroupModelAnalysisResult, GROUP_MODEL_ANALYSIS_VERSION,
    GroupModelAnalysisDispatchAuthority, GroupModelAnalysisInspection, GroupModelAnalysisProvider,
    GroupModelAnalysisRecord, GroupModelAnalysisStore, GroupRunStore,
    MAX_GROUP_MODEL_ANALYSIS_LIST_LIMIT, ModelRequest, PreparedModelProvider,
};

use crate::{
    GroupModelAnalysisRequestCodec, GroupModelAnalysisServiceError,
    group_model_analysis_artifact::build_result_artifact,
    group_model_analysis_collector::{AnalysisModelLimits, collect_prepared_turn},
    group_model_analysis_error::PostClaimError,
    group_model_analysis_source::{
        expected_request, validate_analysis_source_binding, validate_expected_body, validate_source,
    },
    group_model_analysis_validation::{
        checked_inspection, validate_already_claimed, validate_claim_binding,
        validate_completion_result, validate_identifier, validate_list, validate_send_input,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SendGroupModelAnalysisResult {
    AlreadyClaimed {
        inspection: GroupModelAnalysisInspection,
    },
    Completed {
        completion: CompleteGroupModelAnalysisResult,
    },
}

/// A prepared-request transport that exposes its exact destination metadata.
///
/// The methods must describe the destination and model used by
/// [`PreparedModelProvider::stream_prepared`].
pub trait GroupModelAnalysisDispatchProvider: PreparedModelProvider {
    #[must_use]
    fn analysis_provider(&self) -> GroupModelAnalysisProvider;

    #[must_use]
    fn endpoint(&self) -> &str;

    #[must_use]
    fn model(&self) -> &str;
}

pub struct GroupModelAnalysisService {
    pub(crate) group_runs: Arc<dyn GroupRunStore>,
    pub(crate) analyses: Arc<dyn GroupModelAnalysisStore>,
    pub(crate) codec: Arc<dyn GroupModelAnalysisRequestCodec>,
}

impl GroupModelAnalysisService {
    #[must_use]
    pub fn new(
        group_runs: Arc<dyn GroupRunStore>,
        analyses: Arc<dyn GroupModelAnalysisStore>,
        codec: Arc<dyn GroupModelAnalysisRequestCodec>,
    ) -> Self {
        Self {
            group_runs,
            analyses,
            codec,
        }
    }

    /// Claims at most one dispatch and persists only one validated zero-tool result.
    ///
    /// # Errors
    ///
    /// Returns validation and pre-claim storage errors directly. Every failure
    /// after a dispatch authority commits returns `DispatchUnknown`; the method
    /// never retries or re-dispatches.
    pub async fn send(
        &self,
        claim_request: &ClaimGroupModelAnalysisDispatch,
        provider: &dyn GroupModelAnalysisDispatchProvider,
        cancellation: Cancellation,
        result_created_at_ms: u64,
    ) -> Result<SendGroupModelAnalysisResult, GroupModelAnalysisServiceError> {
        validate_send_input(claim_request, &cancellation, result_created_at_ms)?;
        let prepared =
            self.validate_dispatch_candidate(claim_request, provider, cancellation.clone())?;
        let claimed = self
            .analyses
            .claim_group_model_analysis_dispatch(claim_request)?;
        match claimed {
            ClaimGroupModelAnalysisDispatchResult::AlreadyClaimed { inspection } => {
                let inspection =
                    validate_already_claimed(claim_request, &prepared.analysis, inspection)?;
                Ok(SendGroupModelAnalysisResult::AlreadyClaimed { inspection })
            }
            ClaimGroupModelAnalysisDispatchResult::Claimed { authority } => {
                self.dispatch_claimed(
                    claim_request,
                    authority,
                    prepared,
                    provider,
                    &cancellation,
                    result_created_at_ms,
                )
                .await
            }
        }
    }

    /// Loads and independently rebuilds one complete durable analysis prefix.
    ///
    /// # Errors
    ///
    /// Returns a validation, corruption, not-found, or storage error.
    pub fn inspect(
        &self,
        analysis_id: &str,
    ) -> Result<GroupModelAnalysisInspection, GroupModelAnalysisServiceError> {
        validate_identifier(analysis_id)?;
        self.checked_inspect(analysis_id)
    }

    /// Lists bounded metadata without disclosing request or result bodies.
    ///
    /// # Errors
    ///
    /// Returns an input, metadata-corruption, or storage error.
    pub fn list(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupModelAnalysisRecord>, GroupModelAnalysisServiceError> {
        if let Some(group_run_id) = group_run_id {
            validate_identifier(group_run_id)?;
        }
        if !(1..=MAX_GROUP_MODEL_ANALYSIS_LIST_LIMIT).contains(&limit) {
            return Err(GroupModelAnalysisServiceError::InvalidInput);
        }
        let records = self
            .analyses
            .list_group_model_analyses(group_run_id, limit)?;
        validate_list(&records, group_run_id, limit)?;
        Ok(records)
    }

    fn validate_dispatch_candidate(
        &self,
        request: &ClaimGroupModelAnalysisDispatch,
        provider: &dyn GroupModelAnalysisDispatchProvider,
        cancellation: Cancellation,
    ) -> Result<DispatchCandidate, GroupModelAnalysisServiceError> {
        let inspection = self.checked_inspect(&request.analysis_id)?;
        let analysis = inspection.analysis;
        let snapshot = self.group_runs.inspect_group_run(&analysis.group_run_id)?;
        validate_source(&snapshot, &analysis.group_run_id)?;
        validate_analysis_source_binding(&analysis, &snapshot)?;
        validate_provider_binding(&analysis, provider)?;
        let expected_request = expected_request(&analysis, &snapshot, cancellation)?;
        let expected_body = self
            .codec
            .encode_request(&analysis.config.model, &expected_request)
            .map_err(|_| GroupModelAnalysisServiceError::RequestEncoding)?;
        validate_expected_body(&analysis, &expected_body)?;
        Ok(DispatchCandidate {
            analysis,
            expected_request,
            expected_body,
        })
    }

    async fn dispatch_claimed(
        &self,
        request: &ClaimGroupModelAnalysisDispatch,
        authority: GroupModelAnalysisDispatchAuthority,
        prepared: DispatchCandidate,
        provider: &dyn PreparedModelProvider,
        cancellation: &Cancellation,
        result_created_at_ms: u64,
    ) -> Result<SendGroupModelAnalysisResult, GroupModelAnalysisServiceError> {
        self.finish_claimed(
            request,
            authority,
            prepared,
            provider,
            cancellation,
            result_created_at_ms,
        )
        .await
        .map(|completion| SendGroupModelAnalysisResult::Completed { completion })
        .map_err(|_| GroupModelAnalysisServiceError::DispatchUnknown)
    }

    async fn finish_claimed(
        &self,
        request: &ClaimGroupModelAnalysisDispatch,
        authority: GroupModelAnalysisDispatchAuthority,
        prepared: DispatchCandidate,
        provider: &dyn PreparedModelProvider,
        cancellation: &Cancellation,
        result_created_at_ms: u64,
    ) -> Result<CompleteGroupModelAnalysisResult, PostClaimError> {
        if authority.version() != GROUP_MODEL_ANALYSIS_VERSION {
            return Err(PostClaimError::InconsistentStoreResult);
        }
        let (claim, body) = authority.into_parts();
        validate_claim_binding(&prepared.analysis, request, &claim)?;
        if body != prepared.expected_body {
            return Err(PostClaimError::InconsistentStoreResult);
        }
        self.codec
            .validate_exact_request(&claim.model, &prepared.expected_request, &body)?;
        let limits = limits(&prepared.analysis);
        let turn = collect_prepared_turn(provider, body, cancellation, limits).await?;
        if cancellation.is_cancelled() {
            return Err(PostClaimError::Cancelled);
        }
        let artifact = build_result_artifact(&claim, turn, result_created_at_ms)?;
        let completion = CompleteGroupModelAnalysis {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            artifact: artifact.clone(),
        };
        completion
            .validate()
            .map_err(|_| PostClaimError::Protocol)?;
        let result = self
            .analyses
            .complete_group_model_analysis(&completion)
            .map_err(|_| PostClaimError::Store)?;
        validate_completion_result(&artifact, result)
    }

    fn checked_inspect(
        &self,
        analysis_id: &str,
    ) -> Result<GroupModelAnalysisInspection, GroupModelAnalysisServiceError> {
        checked_inspection(self.analyses.inspect_group_model_analysis(analysis_id)?)
    }
}

fn validate_provider_binding(
    analysis: &GroupModelAnalysisRecord,
    provider: &dyn GroupModelAnalysisDispatchProvider,
) -> Result<(), GroupModelAnalysisServiceError> {
    let valid = provider.analysis_provider() == analysis.config.provider
        && provider.endpoint() == analysis.config.endpoint
        && provider.model() == analysis.config.model;
    valid
        .then_some(())
        .ok_or(GroupModelAnalysisServiceError::InvalidInput)
}

struct DispatchCandidate {
    analysis: GroupModelAnalysisRecord,
    expected_request: ModelRequest,
    expected_body: Vec<u8>,
}

fn limits(analysis: &GroupModelAnalysisRecord) -> AnalysisModelLimits {
    AnalysisModelLimits {
        output_bytes: analysis.config.max_model_output_bytes,
        events: analysis.config.max_model_events,
        output_tokens: analysis.config.max_output_tokens,
    }
}
