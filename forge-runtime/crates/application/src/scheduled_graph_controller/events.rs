use crate::runtime_domain::{
    AppendScheduledGraphControllerDisposition, HubStoreError, SCHEDULED_GRAPH_CONTROLLER_VERSION,
    ScheduledGraphControllerEvent, ScheduledGraphControllerEventPayload,
    ScheduledGraphControllerJournal, ScheduledGraphControllerStore,
};

use super::{ScheduledGraphControllerServiceError, map_hub_error};

pub(super) fn next(
    journal: &ScheduledGraphControllerJournal,
    payload: ScheduledGraphControllerEventPayload,
    created_at_ms: u64,
) -> Result<ScheduledGraphControllerEvent, ScheduledGraphControllerServiceError> {
    let head = journal.head();
    ScheduledGraphControllerEvent {
        v: SCHEDULED_GRAPH_CONTROLLER_VERSION,
        controller_id: journal.header.controller_id.clone(),
        graph_run_id: journal.header.graph_run_id.clone(),
        sequence: head.sequence + 1,
        previous_event_sha256: Some(head.event_sha256.clone()),
        payload,
        created_at_ms,
        event_sha256: String::new(),
    }
    .seal()
    .map_err(|_| ScheduledGraphControllerServiceError::CorruptEvidence)
}

pub(super) fn append(
    store: &dyn ScheduledGraphControllerStore,
    journal: &ScheduledGraphControllerJournal,
    payload: ScheduledGraphControllerEventPayload,
    created_at_ms: u64,
) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
    let event = next(journal, payload, created_at_ms)?;
    let mut submitted = journal.clone();
    submitted.events.push(event.clone());
    submitted
        .validate()
        .map_err(|_| ScheduledGraphControllerServiceError::CorruptEvidence)?;
    let result = match store.append_scheduled_graph_controller_event(&event) {
        Ok(result) => result,
        Err(HubStoreError::Conflict { .. }) => {
            return Err(classify_concurrent_update(store, journal));
        }
        Err(error) => return Err(map_hub_error(&error)),
    };
    let valid = result.journal.validate().is_ok()
        && result.journal.header == journal.header
        && match result.disposition {
            AppendScheduledGraphControllerDisposition::Stored => {
                result.journal.events.len() == journal.events.len() + 1
                    && result.journal.events.starts_with(&journal.events)
                    && result.journal.head() == &event
            }
            AppendScheduledGraphControllerDisposition::Replayed => {
                result.journal.events.len() > journal.events.len()
                    && result.journal.events.starts_with(&journal.events)
                    && result.journal.events.get(journal.events.len()) == Some(&event)
            }
        };
    valid
        .then_some(result.journal)
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn classify_concurrent_update(
    store: &dyn ScheduledGraphControllerStore,
    journal: &ScheduledGraphControllerJournal,
) -> ScheduledGraphControllerServiceError {
    let current = match store.inspect_scheduled_graph_controller(&journal.header.graph_run_id) {
        Ok(current) => current,
        Err(error) => return map_hub_error(&error),
    };
    if current.validate().is_ok()
        && current.header == journal.header
        && current.events.len() > journal.events.len()
        && current.events.starts_with(&journal.events)
    {
        ScheduledGraphControllerServiceError::ConcurrentUpdate
    } else {
        ScheduledGraphControllerServiceError::CorruptEvidence
    }
}
