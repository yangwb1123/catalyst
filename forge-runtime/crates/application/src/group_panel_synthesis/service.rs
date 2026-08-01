use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::runtime_domain::{
    Cancellation, ClaimGroupPanelSynthesisDispatch, ClaimGroupPanelSynthesisDispatchResult,
    CompleteGroupPanelSynthesis, CompleteGroupPanelSynthesisResult, GROUP_PANEL_SYNTHESIS_VERSION,
    GroupPanelSynthesisDispatchAuthority, GroupPanelSynthesisInspection,
    GroupPanelSynthesisProvider, GroupPanelSynthesisRecord, GroupPanelSynthesisStore,
    MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT, ModelRequest, PreparedModelProvider,
};

use crate::{
    GroupAnalysisPanelService, GroupModelAnalysisRequestCodec,
    group_model_analysis_collector::{PreparedTurnLimits, collect_zero_tool_turn},
};

use super::{
    artifact::build_result_artifact,
    error::{GroupPanelSynthesisServiceError, SynthesisPostClaimError},
    prepare::{model_request, public_config, request_config_for},
    source::{ValidatedPanel, validate_panel, validate_source_binding},
    validation::{
        checked_inspection, validate_already_claimed, validate_claim_binding,
        validate_completion_result, validate_expected_body, validate_identifier, validate_list,
        validate_send_input,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SendGroupPanelSynthesisResult {
    AlreadyClaimed {
        inspection: GroupPanelSynthesisInspection,
    },
    Completed {
        completion: CompleteGroupPanelSynthesisResult,
    },
}

pub trait GroupPanelSynthesisDispatchProvider: PreparedModelProvider {
    #[must_use]
    fn synthesis_provider(&self) -> GroupPanelSynthesisProvider;

    #[must_use]
    fn endpoint(&self) -> &str;

    #[must_use]
    fn model(&self) -> &str;
}

pub struct GroupPanelSynthesisService {
    pub(super) panels: Arc<GroupAnalysisPanelService>,
    pub(super) syntheses: Arc<dyn GroupPanelSynthesisStore>,
    pub(super) codec: Arc<dyn GroupModelAnalysisRequestCodec>,
}

impl GroupPanelSynthesisService {
    #[must_use]
    pub fn new(
        panels: Arc<GroupAnalysisPanelService>,
        syntheses: Arc<dyn GroupPanelSynthesisStore>,
        codec: Arc<dyn GroupModelAnalysisRequestCodec>,
    ) -> Self {
        Self {
            panels,
            syntheses,
            codec,
        }
    }

    /// Claims at most one dispatch and persists only a validated zero-tool result.
    ///
    /// # Errors
    ///
    /// Returns pre-claim errors directly. Every post-claim failure becomes
    /// `DispatchUnknown`; this method never retries or re-dispatches.
    pub async fn send(
        &self,
        claim_request: &ClaimGroupPanelSynthesisDispatch,
        confirm_off_machine: bool,
        provider: &dyn GroupPanelSynthesisDispatchProvider,
        cancellation: Cancellation,
        result_not_before_ms: u64,
    ) -> Result<SendGroupPanelSynthesisResult, GroupPanelSynthesisServiceError> {
        validate_send_input(
            claim_request,
            confirm_off_machine,
            &cancellation,
            result_not_before_ms,
        )?;
        let prepared =
            self.validate_dispatch_candidate(claim_request, provider, cancellation.clone())?;
        let claimed = self
            .syntheses
            .claim_group_panel_synthesis_dispatch(claim_request)?;
        match claimed {
            ClaimGroupPanelSynthesisDispatchResult::AlreadyClaimed { inspection } => {
                let inspection =
                    validate_already_claimed(claim_request, &prepared.synthesis, inspection)?;
                Ok(SendGroupPanelSynthesisResult::AlreadyClaimed { inspection })
            }
            ClaimGroupPanelSynthesisDispatchResult::Claimed { authority } => {
                self.dispatch_claimed(
                    claim_request,
                    authority,
                    prepared,
                    provider,
                    &cancellation,
                    result_not_before_ms,
                )
                .await
            }
        }
    }

    /// Fully validates one synthesis and its immutable panel source.
    ///
    /// # Errors
    ///
    /// Returns an input, source, corruption, not-found, or storage error.
    pub fn inspect(
        &self,
        synthesis_id: &str,
    ) -> Result<GroupPanelSynthesisInspection, GroupPanelSynthesisServiceError> {
        validate_identifier(synthesis_id)?;
        let inspection =
            checked_inspection(self.syntheses.inspect_group_panel_synthesis(synthesis_id)?)?;
        if inspection.synthesis.synthesis_id != synthesis_id {
            return Err(GroupPanelSynthesisServiceError::InconsistentStoreResult);
        }
        let panel = validate_panel(self.panels.as_ref(), &inspection.synthesis.panel_id)?;
        let prepared = inspection
            .prepared
            .as_ref()
            .ok_or(GroupPanelSynthesisServiceError::InconsistentStoreResult)?;
        validate_source_binding(&inspection.synthesis, prepared, &panel)?;
        Ok(inspection)
    }

    /// Lists bounded synthesis metadata without result or source bodies.
    ///
    /// # Errors
    ///
    /// Returns an input, metadata-consistency, or storage error.
    pub fn list(
        &self,
        panel_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupPanelSynthesisRecord>, GroupPanelSynthesisServiceError> {
        if let Some(panel_id) = panel_id {
            validate_identifier(panel_id)?;
        }
        if !(1..=MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT).contains(&limit) {
            return Err(GroupPanelSynthesisServiceError::InvalidInput);
        }
        let records = self.syntheses.list_group_panel_syntheses(panel_id, limit)?;
        validate_list(&records, panel_id, limit)?;
        Ok(records)
    }

    fn validate_dispatch_candidate(
        &self,
        request: &ClaimGroupPanelSynthesisDispatch,
        provider: &dyn GroupPanelSynthesisDispatchProvider,
        cancellation: Cancellation,
    ) -> Result<DispatchCandidate, GroupPanelSynthesisServiceError> {
        let inspection = self.inspect(&request.synthesis_id)?;
        let synthesis = inspection.synthesis;
        let panel = validate_panel(self.panels.as_ref(), &synthesis.panel_id)?;
        validate_provider_binding(&synthesis, provider)?;
        let expected_request = expected_request(&synthesis, &panel, cancellation)?;
        let expected_body = self
            .codec
            .encode_request(&synthesis.config.model, &expected_request)
            .map_err(|_| GroupPanelSynthesisServiceError::RequestEncoding)?;
        validate_expected_body(&synthesis, &expected_body)?;
        Ok(DispatchCandidate {
            synthesis,
            expected_request,
            expected_body,
        })
    }

    async fn dispatch_claimed(
        &self,
        request: &ClaimGroupPanelSynthesisDispatch,
        authority: GroupPanelSynthesisDispatchAuthority,
        prepared: DispatchCandidate,
        provider: &dyn PreparedModelProvider,
        cancellation: &Cancellation,
        result_not_before_ms: u64,
    ) -> Result<SendGroupPanelSynthesisResult, GroupPanelSynthesisServiceError> {
        self.finish_claimed(
            request,
            authority,
            prepared,
            provider,
            cancellation,
            result_not_before_ms,
        )
        .await
        .map(|completion| SendGroupPanelSynthesisResult::Completed { completion })
        .map_err(|_| GroupPanelSynthesisServiceError::DispatchUnknown)
    }

    async fn finish_claimed(
        &self,
        request: &ClaimGroupPanelSynthesisDispatch,
        authority: GroupPanelSynthesisDispatchAuthority,
        prepared: DispatchCandidate,
        provider: &dyn PreparedModelProvider,
        cancellation: &Cancellation,
        result_not_before_ms: u64,
    ) -> Result<CompleteGroupPanelSynthesisResult, SynthesisPostClaimError> {
        if authority.version() != GROUP_PANEL_SYNTHESIS_VERSION {
            return Err(SynthesisPostClaimError::InconsistentStoreResult);
        }
        let (claim, body) = authority.into_parts();
        validate_claim_binding(&prepared.synthesis, request, &claim)?;
        if body != prepared.expected_body {
            return Err(SynthesisPostClaimError::InconsistentStoreResult);
        }
        self.codec
            .validate_exact_request(&claim.model, &prepared.expected_request, &body)?;
        let turn =
            collect_zero_tool_turn(provider, body, cancellation, limits(&prepared.synthesis))
                .await?;
        if cancellation.is_cancelled() {
            return Err(SynthesisPostClaimError::Turn);
        }
        let result_created_at_ms = completion_time_ms(result_not_before_ms, claim.released_at_ms)?;
        let artifact = build_result_artifact(&claim, turn, result_created_at_ms)?;
        let completion = CompleteGroupPanelSynthesis {
            v: GROUP_PANEL_SYNTHESIS_VERSION,
            artifact: artifact.clone(),
        };
        completion
            .validate()
            .map_err(|_| SynthesisPostClaimError::Turn)?;
        let result = self
            .syntheses
            .complete_group_panel_synthesis(&completion)
            .map_err(|_| SynthesisPostClaimError::Store)?;
        validate_completion_result(&artifact, result)
    }
}

fn completion_time_ms(
    result_not_before_ms: u64,
    released_at_ms: u64,
) -> Result<u64, SynthesisPostClaimError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SynthesisPostClaimError::Turn)?;
    let now = u64::try_from(elapsed.as_millis()).map_err(|_| SynthesisPostClaimError::Turn)?;
    Ok(now.max(result_not_before_ms).max(released_at_ms))
}

fn expected_request(
    synthesis: &GroupPanelSynthesisRecord,
    panel: &ValidatedPanel,
    cancellation: Cancellation,
) -> Result<ModelRequest, GroupPanelSynthesisServiceError> {
    let config = request_config_for(&synthesis.config.model, synthesis.config.max_output_tokens);
    if public_config(&config) != synthesis.config {
        return Err(GroupPanelSynthesisServiceError::InconsistentStoreResult);
    }
    Ok(model_request(&config, &panel.manifest_json, cancellation))
}

fn validate_provider_binding(
    synthesis: &GroupPanelSynthesisRecord,
    provider: &dyn GroupPanelSynthesisDispatchProvider,
) -> Result<(), GroupPanelSynthesisServiceError> {
    let valid = provider.synthesis_provider() == synthesis.config.provider
        && provider.endpoint() == synthesis.config.endpoint
        && provider.model() == synthesis.config.model;
    valid
        .then_some(())
        .ok_or(GroupPanelSynthesisServiceError::InvalidInput)
}

struct DispatchCandidate {
    synthesis: GroupPanelSynthesisRecord,
    expected_request: ModelRequest,
    expected_body: Vec<u8>,
}

fn limits(synthesis: &GroupPanelSynthesisRecord) -> PreparedTurnLimits {
    PreparedTurnLimits {
        output_bytes: synthesis.config.max_model_output_bytes,
        events: synthesis.config.max_model_events,
        output_tokens: synthesis.config.max_output_tokens,
    }
}
