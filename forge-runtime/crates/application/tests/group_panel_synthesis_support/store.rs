use std::sync::{Mutex, MutexGuard};

use forge_runtime_domain::{
    ClaimGroupPanelSynthesisDispatch, ClaimGroupPanelSynthesisDispatchResult,
    CompleteGroupPanelSynthesis, CompleteGroupPanelSynthesisDisposition,
    CompleteGroupPanelSynthesisResult, GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
    GROUP_PANEL_SYNTHESIS_VERSION, GroupPanelSynthesisDispatchAuthority,
    GroupPanelSynthesisDispatchClaim, GroupPanelSynthesisEvent, GroupPanelSynthesisEventKind,
    GroupPanelSynthesisInspection, GroupPanelSynthesisPreparedReceipt, GroupPanelSynthesisRecord,
    GroupPanelSynthesisResultArtifact, GroupPanelSynthesisResultReceipt, GroupPanelSynthesisStatus,
    GroupPanelSynthesisStore, HubEntity, HubStoreError, PrepareGroupPanelSynthesis,
    PrepareGroupPanelSynthesisDisposition, PrepareGroupPanelSynthesisResult,
};

#[derive(Default)]
struct State {
    record: Option<GroupPanelSynthesisRecord>,
    events: Vec<GroupPanelSynthesisEvent>,
    result: Option<GroupPanelSynthesisResultArtifact>,
    request_body: Vec<u8>,
    prepared_request: Option<PrepareGroupPanelSynthesis>,
    claim_calls: usize,
    complete_calls: usize,
    fail_completion: bool,
}

#[derive(Default)]
pub(crate) struct MemorySynthesisStore {
    state: Mutex<State>,
}

impl MemorySynthesisStore {
    pub(crate) fn request_body(&self) -> Vec<u8> {
        self.lock().request_body.clone()
    }

    pub(crate) fn prepared_request(&self) -> PrepareGroupPanelSynthesis {
        self.lock()
            .prepared_request
            .clone()
            .expect("synthesis prepared")
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

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("synthesis state")
    }
}

impl GroupPanelSynthesisStore for MemorySynthesisStore {
    fn prepare_group_panel_synthesis(
        &self,
        request: &PrepareGroupPanelSynthesis,
    ) -> Result<PrepareGroupPanelSynthesisResult, HubStoreError> {
        let mut state = self.lock();
        if state.record.is_some() {
            return prepare_result(&state, PrepareGroupPanelSynthesisDisposition::Replayed);
        }
        state.record = Some(record(request));
        state.request_body.clone_from(&request.request_body);
        state.prepared_request = Some(request.clone());
        state.events.push(prepared_event(request));
        prepare_result(&state, PrepareGroupPanelSynthesisDisposition::Created)
    }

    fn claim_group_panel_synthesis_dispatch(
        &self,
        request: &ClaimGroupPanelSynthesisDispatch,
    ) -> Result<ClaimGroupPanelSynthesisDispatchResult, HubStoreError> {
        let mut state = self.lock();
        state.claim_calls = state.claim_calls.saturating_add(1);
        require_synthesis(&state, &request.synthesis_id)?;
        if state.events.len() >= 2 {
            return Ok(ClaimGroupPanelSynthesisDispatchResult::AlreadyClaimed {
                inspection: inspection(&state)?,
            });
        }
        claim_synthesis(&mut state, request)
    }

    fn complete_group_panel_synthesis(
        &self,
        request: &CompleteGroupPanelSynthesis,
    ) -> Result<CompleteGroupPanelSynthesisResult, HubStoreError> {
        let mut state = self.lock();
        state.complete_calls = state.complete_calls.saturating_add(1);
        if state.fail_completion {
            return Err(unavailable("completion sentinel"));
        }
        complete_synthesis(&mut state, request)
    }

    fn inspect_group_panel_synthesis(
        &self,
        synthesis_id: &str,
    ) -> Result<GroupPanelSynthesisInspection, HubStoreError> {
        let state = self.lock();
        require_synthesis(&state, synthesis_id)?;
        inspection(&state)
    }

    fn list_group_panel_syntheses(
        &self,
        panel_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupPanelSynthesisRecord>, HubStoreError> {
        Ok(self
            .lock()
            .record
            .iter()
            .filter(|record| panel_id.is_none_or(|id| id == record.panel_id))
            .take(limit)
            .cloned()
            .collect())
    }
}

fn claim_synthesis(
    state: &mut State,
    request: &ClaimGroupPanelSynthesisDispatch,
) -> Result<ClaimGroupPanelSynthesisDispatchResult, HubStoreError> {
    let record = state.record.as_ref().expect("record").clone();
    let claim = dispatch_claim(&record, request);
    let authority = GroupPanelSynthesisDispatchAuthority::new(
        &record,
        claim.clone(),
        state.request_body.clone(),
    )
    .map_err(|error| corrupt(&error.message))?;
    state.events.push(GroupPanelSynthesisEvent {
        v: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        synthesis_id: record.synthesis_id.clone(),
        seq: 2,
        kind: GroupPanelSynthesisEventKind::ProviderDispatchReleased { claim },
    });
    state.record.as_mut().expect("record").status = GroupPanelSynthesisStatus::DispatchUnknown;
    Ok(ClaimGroupPanelSynthesisDispatchResult::Claimed { authority })
}

fn complete_synthesis(
    state: &mut State,
    request: &CompleteGroupPanelSynthesis,
) -> Result<CompleteGroupPanelSynthesisResult, HubStoreError> {
    if state.result.is_some() {
        return Ok(CompleteGroupPanelSynthesisResult {
            v: GROUP_PANEL_SYNTHESIS_VERSION,
            disposition: CompleteGroupPanelSynthesisDisposition::Replayed,
            inspection: inspection(state)?,
        });
    }
    let claim = inspection(state)?
        .dispatch
        .ok_or_else(|| corrupt("completion has no dispatch"))?;
    state.events.push(completed_event(&claim, request));
    state.record.as_mut().expect("record").status = GroupPanelSynthesisStatus::Completed;
    state.result = Some(request.artifact.clone());
    Ok(CompleteGroupPanelSynthesisResult {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        disposition: CompleteGroupPanelSynthesisDisposition::Created,
        inspection: inspection(state)?,
    })
}

fn prepare_result(
    state: &State,
    disposition: PrepareGroupPanelSynthesisDisposition,
) -> Result<PrepareGroupPanelSynthesisResult, HubStoreError> {
    Ok(PrepareGroupPanelSynthesisResult {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        disposition,
        inspection: inspection(state)?,
    })
}

fn record(request: &PrepareGroupPanelSynthesis) -> GroupPanelSynthesisRecord {
    GroupPanelSynthesisRecord {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: request.synthesis_id.clone(),
        panel_id: request.source.panel_id.clone(),
        group_run_id: request.source.group_run_id.clone(),
        status: GroupPanelSynthesisStatus::AwaitingConsent,
        source_snapshot_sha256: request.source.source_snapshot_sha256.clone(),
        panel_manifest_sha256: request.source.panel_manifest_sha256.clone(),
        config: request.config.clone(),
        config_sha256: request.config_sha256.clone(),
        request_sha256: request.request_sha256.clone(),
        request_bytes: request.request_body.len(),
        protocol_version: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        created_at_ms: request.created_at_ms,
    }
}

fn prepared_event(request: &PrepareGroupPanelSynthesis) -> GroupPanelSynthesisEvent {
    GroupPanelSynthesisEvent {
        v: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        synthesis_id: request.synthesis_id.clone(),
        seq: 1,
        kind: GroupPanelSynthesisEventKind::SynthesisPrepared {
            receipt: GroupPanelSynthesisPreparedReceipt {
                v: GROUP_PANEL_SYNTHESIS_VERSION,
                synthesis_id: request.synthesis_id.clone(),
                source: request.source.clone(),
                config_sha256: request.config_sha256.clone(),
                request_sha256: request.request_sha256.clone(),
                request_bytes: request.request_body.len(),
            },
        },
    }
}

fn dispatch_claim(
    record: &GroupPanelSynthesisRecord,
    request: &ClaimGroupPanelSynthesisDispatch,
) -> GroupPanelSynthesisDispatchClaim {
    GroupPanelSynthesisDispatchClaim {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: request.synthesis_id.clone(),
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
    claim: &GroupPanelSynthesisDispatchClaim,
    request: &CompleteGroupPanelSynthesis,
) -> GroupPanelSynthesisEvent {
    let artifact = &request.artifact;
    GroupPanelSynthesisEvent {
        v: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        synthesis_id: claim.synthesis_id.clone(),
        seq: 3,
        kind: GroupPanelSynthesisEventKind::SynthesisCompleted {
            receipt: GroupPanelSynthesisResultReceipt {
                v: artifact.result.v,
                synthesis_id: claim.synthesis_id.clone(),
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

fn inspection(state: &State) -> Result<GroupPanelSynthesisInspection, HubStoreError> {
    GroupPanelSynthesisInspection::validate(
        state
            .record
            .clone()
            .ok_or_else(|| corrupt("synthesis is not prepared"))?,
        state.events.clone(),
        state.result.clone(),
    )
    .map_err(|error| corrupt(&error.message))
}

fn require_synthesis(state: &State, synthesis_id: &str) -> Result<(), HubStoreError> {
    match &state.record {
        Some(record) if record.synthesis_id == synthesis_id => Ok(()),
        _ => Err(HubStoreError::NotFound {
            entity: HubEntity::GroupPanelSynthesis,
            id: synthesis_id.into(),
        }),
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
