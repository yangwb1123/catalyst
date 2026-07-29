use std::sync::{Mutex, MutexGuard};

use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatch, ClaimGroupModelAnalysisDispatchResult,
    CompleteGroupModelAnalysis, CompleteGroupModelAnalysisDisposition,
    CompleteGroupModelAnalysisResult, GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
    GROUP_MODEL_ANALYSIS_VERSION, GroupModelAnalysisDispatchAuthority,
    GroupModelAnalysisDispatchClaim, GroupModelAnalysisEvent, GroupModelAnalysisEventKind,
    GroupModelAnalysisInspection, GroupModelAnalysisPreparedReceipt, GroupModelAnalysisRecord,
    GroupModelAnalysisResultArtifact, GroupModelAnalysisResultReceipt, GroupModelAnalysisStatus,
    GroupModelAnalysisStore, HubEntity, HubStoreError, PrepareGroupModelAnalysis,
    PrepareGroupModelAnalysisDisposition, PrepareGroupModelAnalysisResult,
};

#[derive(Default)]
struct AnalysisState {
    record: Option<GroupModelAnalysisRecord>,
    events: Vec<GroupModelAnalysisEvent>,
    result: Option<GroupModelAnalysisResultArtifact>,
    request_body: Vec<u8>,
    prepared_request: Option<PrepareGroupModelAnalysis>,
    claim_calls: usize,
    complete_calls: usize,
    fail_completion: bool,
}

#[derive(Default)]
pub(crate) struct MemoryAnalysisStore {
    state: Mutex<AnalysisState>,
}

impl MemoryAnalysisStore {
    pub(crate) fn request_body(&self) -> Vec<u8> {
        self.lock().request_body.clone()
    }

    pub(crate) fn prepared_request(&self) -> PrepareGroupModelAnalysis {
        self.lock()
            .prepared_request
            .clone()
            .expect("analysis was prepared")
    }

    pub(crate) fn claim_calls(&self) -> usize {
        self.lock().claim_calls
    }

    pub(crate) fn complete_calls(&self) -> usize {
        self.lock().complete_calls
    }

    pub(crate) fn fail_completion(&self) {
        self.lock().fail_completion = true;
    }

    pub(crate) fn corrupt_system_prompt_sha256(&self) {
        self.lock()
            .record
            .as_mut()
            .expect("analysis was prepared")
            .config
            .system_prompt_sha256 = "0".repeat(64);
    }

    fn lock(&self) -> MutexGuard<'_, AnalysisState> {
        self.state.lock().expect("analysis state")
    }
}

impl GroupModelAnalysisStore for MemoryAnalysisStore {
    fn prepare_group_model_analysis(
        &self,
        request: &PrepareGroupModelAnalysis,
    ) -> Result<PrepareGroupModelAnalysisResult, HubStoreError> {
        let mut state = self.lock();
        if state.record.is_some() {
            return prepare_result(&state, PrepareGroupModelAnalysisDisposition::Replayed);
        }
        state.record = Some(record(request));
        state.request_body.clone_from(&request.request_body);
        state.prepared_request = Some(request.clone());
        state.events.push(prepared_event(request));
        prepare_result(&state, PrepareGroupModelAnalysisDisposition::Created)
    }

    fn claim_group_model_analysis_dispatch(
        &self,
        request: &ClaimGroupModelAnalysisDispatch,
    ) -> Result<ClaimGroupModelAnalysisDispatchResult, HubStoreError> {
        let mut state = self.lock();
        state.claim_calls = state.claim_calls.saturating_add(1);
        require_analysis(&state, &request.analysis_id)?;
        if state.events.len() >= 2 {
            return Ok(ClaimGroupModelAnalysisDispatchResult::AlreadyClaimed {
                inspection: inspection(&state)?,
            });
        }
        claim_analysis(&mut state, request)
    }

    fn complete_group_model_analysis(
        &self,
        request: &CompleteGroupModelAnalysis,
    ) -> Result<CompleteGroupModelAnalysisResult, HubStoreError> {
        let mut state = self.lock();
        state.complete_calls = state.complete_calls.saturating_add(1);
        if state.fail_completion {
            return Err(HubStoreError::Unavailable {
                message: "completion sentinel".into(),
            });
        }
        complete_analysis(&mut state, request)
    }

    fn inspect_group_model_analysis(
        &self,
        analysis_id: &str,
    ) -> Result<GroupModelAnalysisInspection, HubStoreError> {
        let state = self.lock();
        require_analysis(&state, analysis_id)?;
        inspection(&state)
    }

    fn list_group_model_analyses(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupModelAnalysisRecord>, HubStoreError> {
        Ok(self
            .lock()
            .record
            .iter()
            .filter(|record| group_run_id.is_none_or(|id| id == record.group_run_id))
            .take(limit)
            .cloned()
            .collect())
    }
}

fn claim_analysis(
    state: &mut AnalysisState,
    request: &ClaimGroupModelAnalysisDispatch,
) -> Result<ClaimGroupModelAnalysisDispatchResult, HubStoreError> {
    let record = state.record.as_ref().expect("prepared record").clone();
    let claim = dispatch_claim(&record, request);
    let authority = GroupModelAnalysisDispatchAuthority::new(
        &record,
        claim.clone(),
        state.request_body.clone(),
    )
    .map_err(|error| journal_error(&error))?;
    state.events.push(GroupModelAnalysisEvent {
        v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        analysis_id: record.analysis_id.clone(),
        seq: 2,
        kind: GroupModelAnalysisEventKind::ProviderDispatchReleased { claim },
    });
    state.record.as_mut().expect("prepared record").status =
        GroupModelAnalysisStatus::DispatchUnknown;
    Ok(ClaimGroupModelAnalysisDispatchResult::Claimed { authority })
}

fn complete_analysis(
    state: &mut AnalysisState,
    request: &CompleteGroupModelAnalysis,
) -> Result<CompleteGroupModelAnalysisResult, HubStoreError> {
    if state.result.is_some() {
        return Ok(CompleteGroupModelAnalysisResult {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            disposition: CompleteGroupModelAnalysisDisposition::Replayed,
            inspection: inspection(state)?,
        });
    }
    let claim = inspection(state)?
        .dispatch
        .ok_or_else(|| corrupt("completion has no dispatch"))?;
    state.events.push(completed_event(&claim, request));
    state.record.as_mut().expect("prepared record").status = GroupModelAnalysisStatus::Completed;
    state.result = Some(request.artifact.clone());
    Ok(CompleteGroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        disposition: CompleteGroupModelAnalysisDisposition::Created,
        inspection: inspection(state)?,
    })
}

fn prepare_result(
    state: &AnalysisState,
    disposition: PrepareGroupModelAnalysisDisposition,
) -> Result<PrepareGroupModelAnalysisResult, HubStoreError> {
    Ok(PrepareGroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        disposition,
        inspection: inspection(state)?,
    })
}

fn record(request: &PrepareGroupModelAnalysis) -> GroupModelAnalysisRecord {
    GroupModelAnalysisRecord {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: request.analysis_id.clone(),
        group_run_id: request.source.group_run_id.clone(),
        status: GroupModelAnalysisStatus::AwaitingConsent,
        source_snapshot_sha256: request.source.snapshot_sha256.clone(),
        config: request.config.clone(),
        config_sha256: request.config_sha256.clone(),
        request_sha256: request.request_sha256.clone(),
        request_bytes: request.request_body.len(),
        protocol_version: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        created_at_ms: request.created_at_ms,
    }
}

fn prepared_event(request: &PrepareGroupModelAnalysis) -> GroupModelAnalysisEvent {
    GroupModelAnalysisEvent {
        v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        analysis_id: request.analysis_id.clone(),
        seq: 1,
        kind: GroupModelAnalysisEventKind::AnalysisPrepared {
            receipt: GroupModelAnalysisPreparedReceipt {
                v: GROUP_MODEL_ANALYSIS_VERSION,
                analysis_id: request.analysis_id.clone(),
                source: request.source.clone(),
                config_sha256: request.config_sha256.clone(),
                request_sha256: request.request_sha256.clone(),
                request_bytes: request.request_body.len(),
            },
        },
    }
}

fn dispatch_claim(
    record: &GroupModelAnalysisRecord,
    request: &ClaimGroupModelAnalysisDispatch,
) -> GroupModelAnalysisDispatchClaim {
    GroupModelAnalysisDispatchClaim {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: request.analysis_id.clone(),
        dispatch_id: request.dispatch_id.clone(),
        request_sha256: record.request_sha256.clone(),
        config_sha256: record.config_sha256.clone(),
        provider: record.config.provider,
        endpoint: record.config.endpoint.clone(),
        model: record.config.model.clone(),
        consent_version: request.consent_version,
        released_at_ms: request.released_at_ms,
    }
}

fn completed_event(
    claim: &GroupModelAnalysisDispatchClaim,
    request: &CompleteGroupModelAnalysis,
) -> GroupModelAnalysisEvent {
    let artifact = &request.artifact;
    GroupModelAnalysisEvent {
        v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        analysis_id: claim.analysis_id.clone(),
        seq: 3,
        kind: GroupModelAnalysisEventKind::AnalysisCompleted {
            receipt: GroupModelAnalysisResultReceipt {
                v: artifact.result.v,
                analysis_id: claim.analysis_id.clone(),
                dispatch_id: claim.dispatch_id.clone(),
                request_sha256: claim.request_sha256.clone(),
                outcome: artifact.result.outcome,
                result_sha256: artifact.result_sha256.clone(),
                result_bytes: artifact.result_bytes,
                usage: artifact.result.usage,
                created_at_ms: artifact.created_at_ms,
            },
        },
    }
}

fn inspection(state: &AnalysisState) -> Result<GroupModelAnalysisInspection, HubStoreError> {
    GroupModelAnalysisInspection::validate(
        state
            .record
            .clone()
            .ok_or_else(|| corrupt("analysis is not prepared"))?,
        state.events.clone(),
        state.result.clone(),
    )
    .map_err(|error| journal_error(&error))
}

fn require_analysis(state: &AnalysisState, analysis_id: &str) -> Result<(), HubStoreError> {
    match &state.record {
        Some(record) if record.analysis_id == analysis_id => Ok(()),
        _ => Err(HubStoreError::NotFound {
            entity: HubEntity::GroupModelAnalysis,
            id: analysis_id.into(),
        }),
    }
}

fn journal_error(error: &forge_runtime_domain::GroupModelAnalysisJournalError) -> HubStoreError {
    corrupt(&error.message)
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
