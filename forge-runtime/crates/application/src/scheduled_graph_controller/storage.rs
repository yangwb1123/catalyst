use crate::runtime_domain::{
    HubStoreError, ScheduledGraphControllerEventPayload, ScheduledGraphControllerHeader,
    ScheduledGraphControllerJournal,
};

use super::{
    ScheduledGraphControllerService, ScheduledGraphControllerServiceError, events, map_hub_error,
    validate_identifier,
};

impl ScheduledGraphControllerService {
    pub(super) fn event_time(
        &self,
        journal: Option<&ScheduledGraphControllerJournal>,
        fallback: u64,
    ) -> u64 {
        let sampled = self.sample_time(fallback);
        journal.map_or(sampled, |value| sampled.max(value.head().created_at_ms))
    }

    pub(super) fn append_event(
        &self,
        journal: &ScheduledGraphControllerJournal,
        payload: ScheduledGraphControllerEventPayload,
        fallback: u64,
    ) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
        events::append(
            self.hub.as_ref(),
            journal,
            payload,
            self.event_time(Some(journal), fallback),
        )
    }

    pub(super) fn load(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
        validate_identifier(graph_run_id)?;
        self.hub
            .inspect_scheduled_graph_controller(graph_run_id)
            .map_err(|error| map_hub_error(&error))
    }

    pub(super) fn existing(
        &self,
        graph_run_id: &str,
    ) -> Result<Option<ScheduledGraphControllerJournal>, ScheduledGraphControllerServiceError> {
        match self.hub.inspect_scheduled_graph_controller(graph_run_id) {
            Ok(journal) => Ok(Some(journal)),
            Err(HubStoreError::NotFound { .. }) => Ok(None),
            Err(error) => Err(map_hub_error(&error)),
        }
    }

    pub(super) fn classify_concurrent_start(
        &self,
        header: &ScheduledGraphControllerHeader,
    ) -> ScheduledGraphControllerServiceError {
        match self
            .hub
            .inspect_scheduled_graph_controller(&header.graph_run_id)
        {
            Ok(journal)
                if journal.validate().is_ok()
                    && journal.header.graph_run_id == header.graph_run_id =>
            {
                ScheduledGraphControllerServiceError::ConcurrentUpdate
            }
            Ok(_) => ScheduledGraphControllerServiceError::CorruptEvidence,
            Err(error) => map_hub_error(&error),
        }
    }

    pub(super) fn sample_time(&self, fallback: u64) -> u64 {
        self.clock.now_ms().max(fallback)
    }
}
