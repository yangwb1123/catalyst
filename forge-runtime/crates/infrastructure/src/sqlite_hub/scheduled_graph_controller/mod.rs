mod read;
mod write;

#[cfg(test)]
mod tests;

use crate::runtime_domain::{
    AppendScheduledGraphControllerResult, HubStoreError, ScheduledGraphControllerEvent,
    ScheduledGraphControllerHeader, ScheduledGraphControllerJournal, ScheduledGraphControllerStore,
};

use super::SqliteHubStore;

impl ScheduledGraphControllerStore for SqliteHubStore {
    fn start_scheduled_graph_controller(
        &self,
        header: &ScheduledGraphControllerHeader,
        event: &ScheduledGraphControllerEvent,
    ) -> Result<AppendScheduledGraphControllerResult, HubStoreError> {
        write::start(&mut self.connect()?, header, event)
    }

    fn append_scheduled_graph_controller_event(
        &self,
        event: &ScheduledGraphControllerEvent,
    ) -> Result<AppendScheduledGraphControllerResult, HubStoreError> {
        write::append(&mut self.connect()?, event)
    }

    fn inspect_scheduled_graph_controller(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphControllerJournal, HubStoreError> {
        read::inspect(&self.connect()?, graph_run_id)
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: crate::runtime_domain::HubEntity::ScheduledGraphController,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn not_found(graph_run_id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: crate::runtime_domain::HubEntity::ScheduledGraphController,
        id: graph_run_id.into(),
    }
}
