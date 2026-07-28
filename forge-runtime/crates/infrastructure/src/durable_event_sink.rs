use crate::runtime_domain::{EventSink, EventSinkError, RunStore, RuntimeEvent};

pub struct DurableFirstEventSink<'a> {
    store: &'a dyn RunStore,
    downstream: &'a mut dyn EventSink,
}

impl<'a> DurableFirstEventSink<'a> {
    #[must_use]
    pub fn new(store: &'a dyn RunStore, downstream: &'a mut dyn EventSink) -> Self {
        Self { store, downstream }
    }
}

impl EventSink for DurableFirstEventSink<'_> {
    fn emit(&mut self, event: &RuntimeEvent) -> Result<(), EventSinkError> {
        self.store.append_event(event).map_err(|error| {
            EventSinkError::new(format!("durable Run event append failed: {error}"))
        })?;
        self.downstream.emit(event)
    }
}
