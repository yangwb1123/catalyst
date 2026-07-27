use std::time::{SystemTime, UNIX_EPOCH};

use forge_runtime_domain::{EventSink, PROTOCOL_VERSION, RuntimeEvent, RuntimeEventKind};

use crate::RuntimeError;

pub(crate) struct EventEmitter<'a> {
    sink: &'a mut dyn EventSink,
    session_id: String,
    run_id: String,
    next_sequence: u64,
}

impl<'a> EventEmitter<'a> {
    pub(crate) fn new(sink: &'a mut dyn EventSink, session_id: String, run_id: String) -> Self {
        Self {
            sink,
            session_id,
            run_id,
            next_sequence: 1,
        }
    }

    pub(crate) fn emit(&mut self, kind: RuntimeEventKind) -> Result<(), RuntimeError> {
        let event = RuntimeEvent {
            v: PROTOCOL_VERSION,
            session_id: self.session_id.clone(),
            run_id: self.run_id.clone(),
            seq: self.next_sequence,
            emitted_at_ms: unix_time_millis(),
            kind,
        };
        self.sink.emit(&event)?;
        self.next_sequence = self.next_sequence.saturating_add(1);
        Ok(())
    }
}

fn unix_time_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    u64::try_from(millis).unwrap_or(u64::MAX)
}
