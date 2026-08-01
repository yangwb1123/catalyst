use super::validation::{
    synthesis_error, validate_claim, validate_completion, validate_completion_receipt,
    validate_prepared, validate_record, validate_status,
};
use super::{
    GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION, GROUP_PANEL_SYNTHESIS_VERSION,
    GroupPanelSynthesisDispatchClaim, GroupPanelSynthesisEvent, GroupPanelSynthesisEventKind,
    GroupPanelSynthesisInspection, GroupPanelSynthesisJournalCursor,
    GroupPanelSynthesisJournalError, GroupPanelSynthesisJournalState,
    GroupPanelSynthesisPreparedReceipt, GroupPanelSynthesisRecord, GroupPanelSynthesisRecovery,
    GroupPanelSynthesisResultArtifact, GroupPanelSynthesisResultReceipt, GroupPanelSynthesisStatus,
    MAX_GROUP_PANEL_SYNTHESIS_EVENTS,
};

impl GroupPanelSynthesisJournalCursor {
    /// Creates an empty cursor bound to one synthesis record.
    ///
    /// # Errors
    ///
    /// Returns an error when durable record metadata violates the protocol.
    pub fn new(
        record: &GroupPanelSynthesisRecord,
    ) -> Result<Self, GroupPanelSynthesisJournalError> {
        validate_record(record)?;
        Ok(Self {
            v: GROUP_PANEL_SYNTHESIS_VERSION,
            synthesis_id: record.synthesis_id.clone(),
            panel_id: record.panel_id.clone(),
            group_run_id: record.group_run_id.clone(),
            source_snapshot_sha256: record.source_snapshot_sha256.clone(),
            panel_manifest_sha256: record.panel_manifest_sha256.clone(),
            config: record.config.clone(),
            config_sha256: record.config_sha256.clone(),
            request_sha256: record.request_sha256.clone(),
            request_bytes: record.request_bytes,
            protocol_version: record.protocol_version,
            created_at_ms: record.created_at_ms,
            next_sequence: 1,
            state: GroupPanelSynthesisJournalState::NeedPreparation,
        })
    }

    /// Applies one contiguous, semantically bound synthesis event.
    ///
    /// # Errors
    ///
    /// Returns an error without mutation for an invalid envelope or transition.
    pub fn append(
        &mut self,
        event: &GroupPanelSynthesisEvent,
    ) -> Result<(), GroupPanelSynthesisJournalError> {
        self.validate_internal()?;
        self.validate_envelope(event)?;
        let state = self.transition(event)?;
        let next = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| synthesis_error("synthesis event sequence exhausted"))?;
        self.state = state;
        self.next_sequence = next;
        Ok(())
    }

    /// Confirms that a restored cursor belongs to the supplied record.
    ///
    /// # Errors
    ///
    /// Returns an error for identity, source, request, or status divergence.
    pub fn validate_record(
        &self,
        record: &GroupPanelSynthesisRecord,
    ) -> Result<(), GroupPanelSynthesisJournalError> {
        validate_record(record)?;
        self.validate_internal()?;
        let matches = self.synthesis_id == record.synthesis_id
            && self.panel_id == record.panel_id
            && self.group_run_id == record.group_run_id
            && self.source_snapshot_sha256 == record.source_snapshot_sha256
            && self.panel_manifest_sha256 == record.panel_manifest_sha256
            && self.config == record.config
            && self.config_sha256 == record.config_sha256
            && self.request_sha256 == record.request_sha256
            && self.request_bytes == record.request_bytes
            && self.protocol_version == record.protocol_version
            && self.created_at_ms == record.created_at_ms;
        if !matches {
            return Err(synthesis_error(
                "synthesis cursor does not match its durable record",
            ));
        }
        validate_status(record.status, &self.recovery())
    }

    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    #[must_use]
    pub fn recovery(&self) -> GroupPanelSynthesisRecovery {
        match &self.state {
            GroupPanelSynthesisJournalState::NeedPreparation => {
                GroupPanelSynthesisRecovery::Unprepared
            }
            GroupPanelSynthesisJournalState::AwaitingConsent(_) => {
                GroupPanelSynthesisRecovery::AwaitingConsent
            }
            GroupPanelSynthesisJournalState::DispatchUnknown { claim, .. } => {
                GroupPanelSynthesisRecovery::DispatchUnknown {
                    dispatch_id: claim.dispatch_id.clone(),
                }
            }
            GroupPanelSynthesisJournalState::Terminal { completion, .. } => {
                GroupPanelSynthesisRecovery::Terminal {
                    outcome: completion.outcome,
                }
            }
        }
    }

    fn transition(
        &self,
        event: &GroupPanelSynthesisEvent,
    ) -> Result<GroupPanelSynthesisJournalState, GroupPanelSynthesisJournalError> {
        match (&self.state, &event.kind) {
            (
                GroupPanelSynthesisJournalState::NeedPreparation,
                GroupPanelSynthesisEventKind::SynthesisPrepared { receipt },
            ) => self.accept_prepared(receipt),
            (
                GroupPanelSynthesisJournalState::AwaitingConsent(prepared),
                GroupPanelSynthesisEventKind::ProviderDispatchReleased { claim },
            ) => self.accept_dispatch(prepared, claim),
            (
                GroupPanelSynthesisJournalState::DispatchUnknown { prepared, claim },
                GroupPanelSynthesisEventKind::SynthesisCompleted { receipt },
            ) => self.accept_completion(prepared, claim, receipt),
            (GroupPanelSynthesisJournalState::Terminal { .. }, _) => {
                Err(synthesis_error("synthesis event appears after completion"))
            }
            _ => Err(synthesis_error(
                "synthesis event is not valid in the current state",
            )),
        }
    }

    fn accept_prepared(
        &self,
        receipt: &GroupPanelSynthesisPreparedReceipt,
    ) -> Result<GroupPanelSynthesisJournalState, GroupPanelSynthesisJournalError> {
        validate_prepared(&self.record_view(), receipt)?;
        Ok(GroupPanelSynthesisJournalState::AwaitingConsent(
            receipt.clone(),
        ))
    }

    fn accept_dispatch(
        &self,
        prepared: &GroupPanelSynthesisPreparedReceipt,
        claim: &GroupPanelSynthesisDispatchClaim,
    ) -> Result<GroupPanelSynthesisJournalState, GroupPanelSynthesisJournalError> {
        validate_claim(&self.record_view(), claim)?;
        Ok(GroupPanelSynthesisJournalState::DispatchUnknown {
            prepared: prepared.clone(),
            claim: claim.clone(),
        })
    }

    fn accept_completion(
        &self,
        prepared: &GroupPanelSynthesisPreparedReceipt,
        claim: &GroupPanelSynthesisDispatchClaim,
        completion: &GroupPanelSynthesisResultReceipt,
    ) -> Result<GroupPanelSynthesisJournalState, GroupPanelSynthesisJournalError> {
        validate_completion_receipt(&self.record_view(), claim, completion)?;
        Ok(GroupPanelSynthesisJournalState::Terminal {
            prepared: prepared.clone(),
            claim: claim.clone(),
            completion: completion.clone(),
        })
    }

    fn validate_envelope(
        &self,
        event: &GroupPanelSynthesisEvent,
    ) -> Result<(), GroupPanelSynthesisJournalError> {
        if event.v != self.protocol_version || event.synthesis_id != self.synthesis_id {
            return Err(synthesis_error(
                "synthesis event envelope does not match its record",
            ));
        }
        if event.seq != self.next_sequence {
            return Err(synthesis_error(
                "synthesis event sequence is not contiguous",
            ));
        }
        Ok(())
    }

    fn validate_internal(&self) -> Result<(), GroupPanelSynthesisJournalError> {
        if self.v != GROUP_PANEL_SYNTHESIS_VERSION
            || self.protocol_version != GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION
        {
            return Err(synthesis_error("invalid synthesis cursor binding"));
        }
        let record = self.record_view();
        validate_record(&record)?;
        match &self.state {
            GroupPanelSynthesisJournalState::NeedPreparation if self.next_sequence == 1 => Ok(()),
            GroupPanelSynthesisJournalState::AwaitingConsent(prepared)
                if self.next_sequence == 2 =>
            {
                validate_prepared(&record, prepared)
            }
            GroupPanelSynthesisJournalState::DispatchUnknown { prepared, claim }
                if self.next_sequence == 3 =>
            {
                validate_prepared(&record, prepared)?;
                validate_claim(&record, claim)
            }
            GroupPanelSynthesisJournalState::Terminal {
                prepared,
                claim,
                completion,
            } if self.next_sequence == 4 => {
                validate_prepared(&record, prepared)?;
                validate_claim(&record, claim)?;
                validate_completion_receipt(&record, claim, completion)
            }
            _ => Err(synthesis_error(
                "synthesis cursor state disagrees with its next sequence",
            )),
        }
    }

    fn record_view(&self) -> GroupPanelSynthesisRecord {
        GroupPanelSynthesisRecord {
            v: self.v,
            synthesis_id: self.synthesis_id.clone(),
            panel_id: self.panel_id.clone(),
            group_run_id: self.group_run_id.clone(),
            status: status_for_recovery(&self.recovery()),
            source_snapshot_sha256: self.source_snapshot_sha256.clone(),
            panel_manifest_sha256: self.panel_manifest_sha256.clone(),
            config: self.config.clone(),
            config_sha256: self.config_sha256.clone(),
            request_sha256: self.request_sha256.clone(),
            request_bytes: self.request_bytes,
            protocol_version: self.protocol_version,
            created_at_ms: self.created_at_ms,
        }
    }
}

impl GroupPanelSynthesisInspection {
    /// Rebuilds a durable synthesis prefix and validates any terminal result.
    ///
    /// # Errors
    ///
    /// Returns an error for journal, status, evidence, or result divergence.
    pub fn validate(
        synthesis: GroupPanelSynthesisRecord,
        events: Vec<GroupPanelSynthesisEvent>,
        result: Option<GroupPanelSynthesisResultArtifact>,
    ) -> Result<Self, GroupPanelSynthesisJournalError> {
        if events.is_empty() {
            return Err(synthesis_error(
                "durable synthesis is missing its preparation event",
            ));
        }
        if events.len() > MAX_GROUP_PANEL_SYNTHESIS_EVENTS {
            return Err(synthesis_error(
                "durable synthesis exceeds its event count limit",
            ));
        }
        let mut cursor = GroupPanelSynthesisJournalCursor::new(&synthesis)?;
        for event in &events {
            cursor.append(event)?;
        }
        let recovery = cursor.recovery();
        validate_status(synthesis.status, &recovery)?;
        let (prepared, dispatch, completion) = state_evidence(&cursor.state);
        validate_result_presence(
            &synthesis,
            dispatch.as_ref(),
            completion.as_ref(),
            result.as_ref(),
        )?;
        Ok(Self {
            v: GROUP_PANEL_SYNTHESIS_VERSION,
            synthesis,
            events,
            recovery,
            prepared,
            dispatch,
            completion,
            result,
        })
    }
}

type Evidence = (
    Option<GroupPanelSynthesisPreparedReceipt>,
    Option<GroupPanelSynthesisDispatchClaim>,
    Option<GroupPanelSynthesisResultReceipt>,
);

fn state_evidence(state: &GroupPanelSynthesisJournalState) -> Evidence {
    match state {
        GroupPanelSynthesisJournalState::NeedPreparation => (None, None, None),
        GroupPanelSynthesisJournalState::AwaitingConsent(prepared) => {
            (Some(prepared.clone()), None, None)
        }
        GroupPanelSynthesisJournalState::DispatchUnknown { prepared, claim } => {
            (Some(prepared.clone()), Some(claim.clone()), None)
        }
        GroupPanelSynthesisJournalState::Terminal {
            prepared,
            claim,
            completion,
        } => (
            Some(prepared.clone()),
            Some(claim.clone()),
            Some(completion.clone()),
        ),
    }
}

fn validate_result_presence(
    record: &GroupPanelSynthesisRecord,
    claim: Option<&GroupPanelSynthesisDispatchClaim>,
    completion: Option<&GroupPanelSynthesisResultReceipt>,
    result: Option<&GroupPanelSynthesisResultArtifact>,
) -> Result<(), GroupPanelSynthesisJournalError> {
    match (claim, completion, result) {
        (None | Some(_), None, None) => Ok(()),
        (Some(claim), Some(completion), Some(result)) => {
            validate_completion(record, claim, completion, result)
        }
        _ => Err(synthesis_error(
            "synthesis result presence disagrees with its journal",
        )),
    }
}

fn status_for_recovery(recovery: &GroupPanelSynthesisRecovery) -> GroupPanelSynthesisStatus {
    match recovery {
        GroupPanelSynthesisRecovery::Unprepared | GroupPanelSynthesisRecovery::AwaitingConsent => {
            GroupPanelSynthesisStatus::AwaitingConsent
        }
        GroupPanelSynthesisRecovery::DispatchUnknown { .. } => {
            GroupPanelSynthesisStatus::DispatchUnknown
        }
        GroupPanelSynthesisRecovery::Terminal { .. } => GroupPanelSynthesisStatus::Completed,
    }
}
