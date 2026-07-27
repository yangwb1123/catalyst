use std::io::Write;

use forge_runtime_domain::{EventSink, EventSinkError, RuntimeEvent};

pub struct JsonlEventSink<W> {
    writer: W,
}

impl<W> JsonlEventSink<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> EventSink for JsonlEventSink<W> {
    fn emit(&mut self, event: &RuntimeEvent) -> Result<(), EventSinkError> {
        serde_json::to_writer(&mut self.writer, event)
            .map_err(|error| EventSinkError::new(error.to_string()))?;
        self.writer
            .write_all(b"\n")
            .and_then(|()| self.writer.flush())
            .map_err(|error| EventSinkError::new(error.to_string()))
    }
}

#[derive(Default)]
pub struct MemoryEventSink {
    events: Vec<RuntimeEvent>,
}

impl MemoryEventSink {
    #[must_use]
    pub fn events(&self) -> &[RuntimeEvent] {
        &self.events
    }

    #[must_use]
    pub fn into_events(self) -> Vec<RuntimeEvent> {
        self.events
    }
}

impl EventSink for MemoryEventSink {
    fn emit(&mut self, event: &RuntimeEvent) -> Result<(), EventSinkError> {
        self.events.push(event.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use forge_runtime_domain::{PROTOCOL_VERSION, RuntimeEvent, RuntimeEventKind};

    use super::JsonlEventSink;
    use forge_runtime_domain::EventSink;

    #[test]
    fn jsonl_sink_writes_one_lf_delimited_object() {
        let mut sink = JsonlEventSink::new(Vec::new());
        sink.emit(&RuntimeEvent {
            v: PROTOCOL_VERSION,
            session_id: "session-1".into(),
            run_id: "run-1".into(),
            seq: 1,
            emitted_at_ms: 0,
            kind: RuntimeEventKind::TurnStarted { turn: 1 },
        })
        .expect("event is written");

        let bytes = sink.into_inner();
        let text = String::from_utf8(bytes).expect("output is UTF-8");
        assert!(text.ends_with('\n'));
        assert_eq!(text.lines().count(), 1);
    }
}
