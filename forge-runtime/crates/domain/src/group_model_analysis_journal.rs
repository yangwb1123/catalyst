use crate::{
    GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION, GROUP_MODEL_ANALYSIS_VERSION,
    GroupModelAnalysisDispatchClaim, GroupModelAnalysisEvent, GroupModelAnalysisEventKind,
    GroupModelAnalysisInspection, GroupModelAnalysisJournalCursor, GroupModelAnalysisJournalError,
    GroupModelAnalysisPreparedReceipt, GroupModelAnalysisRecord, GroupModelAnalysisRecovery,
    GroupModelAnalysisResultArtifact, GroupModelAnalysisResultReceipt, GroupModelAnalysisStatus,
    MAX_GROUP_MODEL_ANALYSIS_EVENTS,
};

use super::GroupModelAnalysisJournalState;
use super::validation::{
    analysis_error, validate_claim, validate_completion, validate_completion_receipt,
    validate_prepared, validate_record, validate_status,
};

impl GroupModelAnalysisJournalCursor {
    /// Creates an empty cursor bound to one analysis record.
    ///
    /// # Errors
    ///
    /// Returns an error when record metadata violates the protocol.
    pub fn new(record: &GroupModelAnalysisRecord) -> Result<Self, GroupModelAnalysisJournalError> {
        validate_record(record)?;
        Ok(Self {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            analysis_id: record.analysis_id.clone(),
            group_run_id: record.group_run_id.clone(),
            source_snapshot_sha256: record.source_snapshot_sha256.clone(),
            config: record.config.clone(),
            config_sha256: record.config_sha256.clone(),
            request_sha256: record.request_sha256.clone(),
            request_bytes: record.request_bytes,
            protocol_version: record.protocol_version,
            created_at_ms: record.created_at_ms,
            next_sequence: 1,
            state: GroupModelAnalysisJournalState::NeedPreparation,
        })
    }

    /// Applies one contiguous, semantically bound event.
    ///
    /// # Errors
    ///
    /// Returns an error without mutation for a gap, duplicate, or bad binding.
    pub fn append(
        &mut self,
        event: &GroupModelAnalysisEvent,
    ) -> Result<(), GroupModelAnalysisJournalError> {
        self.validate_internal()?;
        self.validate_envelope(event)?;
        let state = self.transition(event)?;
        let next = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| analysis_error("analysis event sequence exhausted"))?;
        self.state = state;
        self.next_sequence = next;
        Ok(())
    }

    /// Confirms that a restored cursor belongs to the supplied record.
    ///
    /// # Errors
    ///
    /// Returns an error for identity, request, version, or status divergence.
    pub fn validate_record(
        &self,
        record: &GroupModelAnalysisRecord,
    ) -> Result<(), GroupModelAnalysisJournalError> {
        validate_record(record)?;
        self.validate_internal()?;
        let matches = self.analysis_id == record.analysis_id
            && self.group_run_id == record.group_run_id
            && self.source_snapshot_sha256 == record.source_snapshot_sha256
            && self.config == record.config
            && self.config_sha256 == record.config_sha256
            && self.request_sha256 == record.request_sha256
            && self.request_bytes == record.request_bytes
            && self.protocol_version == record.protocol_version
            && self.created_at_ms == record.created_at_ms;
        if !matches {
            return Err(analysis_error(
                "analysis cursor does not match its durable record",
            ));
        }
        validate_status(record.status, &self.recovery())
    }

    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    #[must_use]
    pub fn recovery(&self) -> GroupModelAnalysisRecovery {
        match &self.state {
            GroupModelAnalysisJournalState::NeedPreparation => {
                GroupModelAnalysisRecovery::Unprepared
            }
            GroupModelAnalysisJournalState::AwaitingConsent(_) => {
                GroupModelAnalysisRecovery::AwaitingConsent
            }
            GroupModelAnalysisJournalState::DispatchUnknown { claim, .. } => {
                GroupModelAnalysisRecovery::DispatchUnknown {
                    dispatch_id: claim.dispatch_id.clone(),
                }
            }
            GroupModelAnalysisJournalState::Terminal { completion, .. } => {
                GroupModelAnalysisRecovery::Terminal {
                    outcome: completion.outcome,
                }
            }
        }
    }

    fn transition(
        &self,
        event: &GroupModelAnalysisEvent,
    ) -> Result<GroupModelAnalysisJournalState, GroupModelAnalysisJournalError> {
        match (&self.state, &event.kind) {
            (
                GroupModelAnalysisJournalState::NeedPreparation,
                GroupModelAnalysisEventKind::AnalysisPrepared { receipt },
            ) => self.accept_prepared(receipt),
            (
                GroupModelAnalysisJournalState::AwaitingConsent(prepared),
                GroupModelAnalysisEventKind::ProviderDispatchReleased { claim },
            ) => self.accept_dispatch(prepared, claim),
            (
                GroupModelAnalysisJournalState::DispatchUnknown { prepared, claim },
                GroupModelAnalysisEventKind::AnalysisCompleted { receipt },
            ) => self.accept_completion(prepared, claim, receipt),
            (GroupModelAnalysisJournalState::Terminal { .. }, _) => {
                Err(analysis_error("analysis event appears after completion"))
            }
            _ => Err(analysis_error(
                "analysis event is not valid in the current state",
            )),
        }
    }

    fn accept_prepared(
        &self,
        receipt: &GroupModelAnalysisPreparedReceipt,
    ) -> Result<GroupModelAnalysisJournalState, GroupModelAnalysisJournalError> {
        validate_prepared(&self.record_view(), receipt)?;
        Ok(GroupModelAnalysisJournalState::AwaitingConsent(
            receipt.clone(),
        ))
    }

    fn accept_dispatch(
        &self,
        prepared: &GroupModelAnalysisPreparedReceipt,
        claim: &GroupModelAnalysisDispatchClaim,
    ) -> Result<GroupModelAnalysisJournalState, GroupModelAnalysisJournalError> {
        validate_claim(&self.record_view(), claim)?;
        Ok(GroupModelAnalysisJournalState::DispatchUnknown {
            prepared: prepared.clone(),
            claim: claim.clone(),
        })
    }

    fn accept_completion(
        &self,
        prepared: &GroupModelAnalysisPreparedReceipt,
        claim: &GroupModelAnalysisDispatchClaim,
        completion: &GroupModelAnalysisResultReceipt,
    ) -> Result<GroupModelAnalysisJournalState, GroupModelAnalysisJournalError> {
        validate_completion_receipt(&self.record_view(), claim, completion)?;
        Ok(GroupModelAnalysisJournalState::Terminal {
            prepared: prepared.clone(),
            claim: claim.clone(),
            completion: completion.clone(),
        })
    }

    fn validate_envelope(
        &self,
        event: &GroupModelAnalysisEvent,
    ) -> Result<(), GroupModelAnalysisJournalError> {
        if event.v != self.protocol_version || event.analysis_id != self.analysis_id {
            return Err(analysis_error(
                "analysis event envelope does not match its record",
            ));
        }
        if event.seq != self.next_sequence {
            return Err(analysis_error("analysis event sequence is not contiguous"));
        }
        Ok(())
    }

    fn validate_internal(&self) -> Result<(), GroupModelAnalysisJournalError> {
        if self.v != GROUP_MODEL_ANALYSIS_VERSION
            || self.protocol_version != GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION
        {
            return Err(analysis_error("invalid analysis cursor binding"));
        }
        let record = self.record_view();
        validate_record(&record)?;
        match &self.state {
            GroupModelAnalysisJournalState::NeedPreparation if self.next_sequence == 1 => Ok(()),
            GroupModelAnalysisJournalState::AwaitingConsent(prepared)
                if self.next_sequence == 2 =>
            {
                validate_prepared(&record, prepared)
            }
            GroupModelAnalysisJournalState::DispatchUnknown { prepared, claim }
                if self.next_sequence == 3 =>
            {
                validate_prepared(&record, prepared)?;
                validate_claim(&record, claim)
            }
            GroupModelAnalysisJournalState::Terminal {
                prepared,
                claim,
                completion,
            } if self.next_sequence == 4 => {
                validate_prepared(&record, prepared)?;
                validate_claim(&record, claim)?;
                validate_completion_receipt(&record, claim, completion)
            }
            _ => Err(analysis_error(
                "analysis cursor state disagrees with its next sequence",
            )),
        }
    }

    fn record_view(&self) -> GroupModelAnalysisRecord {
        GroupModelAnalysisRecord {
            v: self.v,
            analysis_id: self.analysis_id.clone(),
            group_run_id: self.group_run_id.clone(),
            status: status_for_recovery(&self.recovery()),
            source_snapshot_sha256: self.source_snapshot_sha256.clone(),
            config: self.config.clone(),
            config_sha256: self.config_sha256.clone(),
            request_sha256: self.request_sha256.clone(),
            request_bytes: self.request_bytes,
            protocol_version: self.protocol_version,
            created_at_ms: self.created_at_ms,
        }
    }
}

impl GroupModelAnalysisInspection {
    /// Rebuilds one complete analysis prefix and validates its result binding.
    ///
    /// # Errors
    ///
    /// Returns an error for event, cursor-derived, status, or result divergence.
    pub fn validate(
        analysis: GroupModelAnalysisRecord,
        events: Vec<GroupModelAnalysisEvent>,
        result: Option<GroupModelAnalysisResultArtifact>,
    ) -> Result<Self, GroupModelAnalysisJournalError> {
        if events.is_empty() {
            return Err(analysis_error(
                "durable analysis is missing its preparation event",
            ));
        }
        if events.len() > MAX_GROUP_MODEL_ANALYSIS_EVENTS {
            return Err(analysis_error(
                "durable analysis exceeds its event count limit",
            ));
        }
        let mut cursor = GroupModelAnalysisJournalCursor::new(&analysis)?;
        for event in &events {
            cursor.append(event)?;
        }
        let recovery = cursor.recovery();
        validate_status(analysis.status, &recovery)?;
        let (prepared, dispatch, completion) = state_evidence(&cursor.state);
        validate_result_presence(
            &analysis,
            dispatch.as_ref(),
            completion.as_ref(),
            result.as_ref(),
        )?;
        Ok(Self {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            analysis,
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
    Option<GroupModelAnalysisPreparedReceipt>,
    Option<GroupModelAnalysisDispatchClaim>,
    Option<GroupModelAnalysisResultReceipt>,
);

fn state_evidence(state: &GroupModelAnalysisJournalState) -> Evidence {
    match state {
        GroupModelAnalysisJournalState::NeedPreparation => (None, None, None),
        GroupModelAnalysisJournalState::AwaitingConsent(prepared) => {
            (Some(prepared.clone()), None, None)
        }
        GroupModelAnalysisJournalState::DispatchUnknown { prepared, claim } => {
            (Some(prepared.clone()), Some(claim.clone()), None)
        }
        GroupModelAnalysisJournalState::Terminal {
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
    record: &GroupModelAnalysisRecord,
    claim: Option<&GroupModelAnalysisDispatchClaim>,
    completion: Option<&GroupModelAnalysisResultReceipt>,
    result: Option<&GroupModelAnalysisResultArtifact>,
) -> Result<(), GroupModelAnalysisJournalError> {
    match (claim, completion, result) {
        (None | Some(_), None, None) => Ok(()),
        (Some(claim), Some(completion), Some(result)) => {
            validate_completion(record, claim, completion, result)
        }
        _ => Err(analysis_error(
            "analysis result presence disagrees with its journal",
        )),
    }
}

fn status_for_recovery(recovery: &GroupModelAnalysisRecovery) -> GroupModelAnalysisStatus {
    match recovery {
        GroupModelAnalysisRecovery::Unprepared | GroupModelAnalysisRecovery::AwaitingConsent => {
            GroupModelAnalysisStatus::AwaitingConsent
        }
        GroupModelAnalysisRecovery::DispatchUnknown { .. } => {
            GroupModelAnalysisStatus::DispatchUnknown
        }
        GroupModelAnalysisRecovery::Terminal { .. } => GroupModelAnalysisStatus::Completed,
    }
}
