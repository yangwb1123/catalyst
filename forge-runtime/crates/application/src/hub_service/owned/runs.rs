use super::{HubService, validate_owner};
use crate::{
    HubError, HubField,
    hub_validation::required_id,
    runtime_domain::run_observed::{RunObserved, RunObservedInput, observe_run},
    runtime_domain::{
        ConversationOwner, HubStoreError, MAX_OWNED_RUN_PAGE_LIMIT,
        MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT, OwnedRunCursor, OwnedRunPage, OwnedRunTimelinePage,
    },
};

impl HubService {
    /// Reads a bounded newest-first Run summary page for one owned Conversation.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors without exposing Run execution data.
    pub fn owned_run_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&OwnedRunCursor>,
        limit: usize,
    ) -> Result<OwnedRunPage, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        if !(1..=MAX_OWNED_RUN_PAGE_LIMIT).contains(&limit) {
            return Err(HubError::OutOfRange {
                field: HubField::OwnedRunPageLimit,
                min: 1,
                max: MAX_OWNED_RUN_PAGE_LIMIT,
            });
        }
        if let Some(cursor) = before {
            required_id(&cursor.run_id, HubField::RunId)?;
            if cursor.created_at_ms > i64::MAX as u64 {
                return Err(HubError::OutOfRange {
                    field: HubField::OwnedRunCursor,
                    min: 0,
                    max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
                });
            }
        }
        Ok(self
            .store
            .owned_run_page(owner, conversation_id, before, limit)?)
    }

    /// Reads one owner-visible Run summary and projects it into the bounded,
    /// content-free `forge.run.observed.v1` value. This operation only reads
    /// existing metadata and grants no Run, lease, reservation, or dispatch
    /// authority.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors when ownership, Run membership, or
    /// the stored scalar metadata cannot be validated.
    pub fn owned_run_observation(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<RunObserved, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        required_id(run_id, HubField::RunId)?;
        let run = self
            .store
            .owned_run_observation(owner, conversation_id, run_id)?;
        observe_run(RunObservedInput {
            owner: owner.clone(),
            conversation_id: conversation_id.to_owned(),
            run,
        })
        .map_err(|_| {
            HubError::Store(HubStoreError::Corrupt {
                message: "stored Run observation metadata is invalid".into(),
            })
        })
    }

    /// Reads a bounded payload-free event-type timeline for one owned Run.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors when ownership or the page cannot
    /// be checked.
    pub fn owned_run_timeline_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        run_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<OwnedRunTimelinePage, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        required_id(run_id, HubField::RunId)?;
        if !(1..=MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT).contains(&limit) {
            return Err(HubError::OutOfRange {
                field: HubField::OwnedRunTimelinePageLimit,
                min: 1,
                max: MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT,
            });
        }
        if after_sequence > i64::MAX as u64 {
            return Err(HubError::OutOfRange {
                field: HubField::OwnedRunTimelineSequence,
                min: 0,
                max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
            });
        }
        Ok(self.store.owned_run_timeline_page(
            owner,
            conversation_id,
            run_id,
            after_sequence,
            limit,
        )?)
    }
}
