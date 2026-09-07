use std::io::Write;

use crate::runtime_domain::{
    EventSink, EventSinkError, LimitKind, RunOutcome, RuntimeEvent, RuntimeEventKind,
};

pub struct HumanEventSink<W> {
    writer: W,
    at_line_start: bool,
}

impl<W> HumanEventSink<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            at_line_start: true,
        }
    }

    #[cfg(test)]
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> EventSink for HumanEventSink<W> {
    fn emit(&mut self, event: &RuntimeEvent) -> Result<(), EventSinkError> {
        match &event.kind {
            RuntimeEventKind::RunStarted { .. } => {
                self.status(&format!("[run] {} started", safe_label(&event.run_id)))
            }
            RuntimeEventKind::TurnStarted { turn } => self.status(&format!("[turn] {turn}")),
            RuntimeEventKind::AssistantDelta { delta } => self.assistant_delta(delta),
            RuntimeEventKind::MessageCommitted { .. } => Ok(()),
            RuntimeEventKind::ToolStarted { call } => {
                self.status(&format!("[tool] {} started", safe_label(&call.name)))
            }
            RuntimeEventKind::ToolFinished {
                name,
                is_error,
                truncated,
                ..
            } => self.tool_finished(name, *is_error, *truncated),
            RuntimeEventKind::ToolRejected { call, code, .. } => self.status(&format!(
                "[tool] {} rejected ({})",
                safe_label(&call.name),
                safe_label(code)
            )),
            RuntimeEventKind::RuntimeError { code, .. } => {
                self.status(&format!("[run] error ({})", safe_label(code)))
            }
            RuntimeEventKind::RunFinished { outcome } => self.run_finished(outcome),
        }
    }
}

impl<W: Write> HumanEventSink<W> {
    fn assistant_delta(&mut self, delta: &str) -> Result<(), EventSinkError> {
        let safe = safe_stream_text(delta);
        for line in safe.split_inclusive('\n') {
            if self.at_line_start {
                self.writer
                    .write_all(b"[assistant] ")
                    .map_err(|error| write_error(&error))?;
            }
            self.writer
                .write_all(line.as_bytes())
                .map_err(|error| write_error(&error))?;
            self.at_line_start = line.ends_with('\n');
        }
        self.writer.flush().map_err(|error| write_error(&error))
    }

    fn status(&mut self, status: &str) -> Result<(), EventSinkError> {
        if !self.at_line_start {
            self.writer
                .write_all(b"\n")
                .map_err(|error| write_error(&error))?;
        }
        self.writer
            .write_all(status.as_bytes())
            .and_then(|()| self.writer.write_all(b"\n"))
            .and_then(|()| self.writer.flush())
            .map_err(|error| write_error(&error))?;
        self.at_line_start = true;
        Ok(())
    }

    fn tool_finished(
        &mut self,
        name: &str,
        is_error: bool,
        truncated: bool,
    ) -> Result<(), EventSinkError> {
        let result = if is_error { "failed" } else { "finished" };
        let suffix = if truncated { " (output truncated)" } else { "" };
        self.status(&format!("[tool] {} {result}{suffix}", safe_label(name)))
    }

    fn run_finished(&mut self, outcome: &RunOutcome) -> Result<(), EventSinkError> {
        let status = match outcome {
            RunOutcome::Completed { .. } => "[run] completed".into(),
            RunOutcome::Cancelled => "[run] cancelled".into(),
            RunOutcome::LimitExceeded { kind } => {
                format!("[run] limit exceeded ({})", limit_label(*kind))
            }
            RunOutcome::Failed { code, .. } => {
                format!("[run] failed ({})", safe_label(code))
            }
        };
        self.status(&status)
    }
}

fn limit_label(kind: LimitKind) -> &'static str {
    match kind {
        LimitKind::Turns => "turns",
        LimitKind::ToolCalls => "tool_calls",
        LimitKind::ModelOutput => "model_output",
    }
}

fn safe_label(value: &str) -> String {
    safe_text(value, false)
}

fn safe_stream_text(value: &str) -> String {
    safe_text(value, true)
}

fn safe_text(value: &str, preserve_layout: bool) -> String {
    let mut safe = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' | '\t' if preserve_layout => safe.push(character),
            '\u{1b}' => safe.push_str("\\x1b"),
            '\u{2028}' => safe.push_str("\\u{2028}"),
            '\u{2029}' => safe.push_str("\\u{2029}"),
            value if is_bidi_control(value) => safe.extend(value.escape_unicode()),
            value if value.is_control() => safe.extend(value.escape_default()),
            value => safe.push(value),
        }
    }
    safe
}

fn is_bidi_control(value: char) -> bool {
    matches!(
        value,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn write_error(error: &std::io::Error) -> EventSinkError {
    EventSinkError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use crate::runtime_domain::{
        EventSink, LimitKind, PROTOCOL_VERSION, RunOutcome, RuntimeEvent, RuntimeEventKind,
        ToolCall,
    };
    use serde_json::json;

    use super::HumanEventSink;

    #[test]
    fn streams_assistant_text_and_starts_status_on_a_new_line() {
        let mut sink = HumanEventSink::new(Vec::new());
        emit(
            &mut sink,
            RuntimeEventKind::RunStarted {
                prompt: "secret".into(),
            },
        );
        emit(
            &mut sink,
            RuntimeEventKind::AssistantDelta {
                delta: "hello ".into(),
            },
        );
        emit(
            &mut sink,
            RuntimeEventKind::AssistantDelta {
                delta: "world".into(),
            },
        );
        emit(
            &mut sink,
            RuntimeEventKind::RunFinished {
                outcome: RunOutcome::Completed {
                    answer: "secret answer".into(),
                },
            },
        );

        assert_eq!(
            text(sink),
            "[run] run-1 started\n[assistant] hello world\n[run] completed\n"
        );
    }

    #[test]
    fn tool_status_never_discloses_arguments_results_or_rejection_message() {
        let call = ToolCall {
            id: "call-secret".into(),
            name: "read_file".into(),
            arguments: json!({"path": "secret.txt"}),
        };
        let mut sink = HumanEventSink::new(Vec::new());
        emit(
            &mut sink,
            RuntimeEventKind::ToolStarted { call: call.clone() },
        );
        emit(
            &mut sink,
            RuntimeEventKind::ToolFinished {
                call_id: call.id.clone(),
                name: call.name.clone(),
                output: "secret result".into(),
                is_error: false,
                truncated: true,
            },
        );
        emit(
            &mut sink,
            RuntimeEventKind::ToolRejected {
                call,
                code: "denied".into(),
                message: "secret rejection detail".into(),
            },
        );

        let output = text(sink);
        assert_eq!(
            output,
            "[tool] read_file started\n[tool] read_file finished (output truncated)\n[tool] read_file rejected (denied)\n"
        );
        for secret in [
            "call-secret",
            "secret.txt",
            "secret result",
            "secret rejection",
        ] {
            assert!(!output.contains(secret));
        }
    }

    #[test]
    fn run_status_is_concise_and_hides_error_messages() {
        let mut sink = HumanEventSink::new(Vec::new());
        emit(
            &mut sink,
            RuntimeEventKind::RuntimeError {
                code: "provider_error".into(),
                message: "credential secret".into(),
            },
        );
        emit(
            &mut sink,
            RuntimeEventKind::RunFinished {
                outcome: RunOutcome::LimitExceeded {
                    kind: LimitKind::ToolCalls,
                },
            },
        );
        emit(
            &mut sink,
            RuntimeEventKind::RunFinished {
                outcome: RunOutcome::Failed {
                    code: "failed".into(),
                    message: "private detail".into(),
                },
            },
        );

        let output = text(sink);
        assert_eq!(
            output,
            "[run] error (provider_error)\n[run] limit exceeded (tool_calls)\n[run] failed (failed)\n"
        );
        assert!(!output.contains("credential secret"));
        assert!(!output.contains("private detail"));
    }

    #[test]
    fn untrusted_terminal_controls_are_escaped_but_newlines_stream() {
        let mut sink = HumanEventSink::new(Vec::new());
        emit(
            &mut sink,
            RuntimeEventKind::AssistantDelta {
                delta: "one\n\u{1b}[2Jtwo\rthree".into(),
            },
        );
        emit(
            &mut sink,
            RuntimeEventKind::ToolStarted {
                call: ToolCall {
                    id: "call-1".into(),
                    name: "bad\n\u{1b}[2J".into(),
                    arguments: json!({}),
                },
            },
        );

        assert_eq!(
            text(sink),
            "[assistant] one\n[assistant] \\x1b[2Jtwo\\rthree\n[tool] bad\\n\\x1b[2J started\n"
        );
    }

    #[test]
    fn assistant_lines_cannot_impersonate_runtime_status_across_chunks() {
        let mut sink = HumanEventSink::new(Vec::new());
        for delta in ["opening\n[run] com", "pleted\n", "[tool] exec finished"] {
            emit(
                &mut sink,
                RuntimeEventKind::AssistantDelta {
                    delta: delta.into(),
                },
            );
        }
        emit(
            &mut sink,
            RuntimeEventKind::RunFinished {
                outcome: RunOutcome::Completed {
                    answer: "private".into(),
                },
            },
        );

        assert_eq!(
            text(sink),
            "[assistant] opening\n[assistant] [run] completed\n\
             [assistant] [tool] exec finished\n[run] completed\n"
        );
    }

    #[test]
    fn writer_failure_becomes_event_sink_error() {
        let mut sink = HumanEventSink::new(FailingWriter);
        let error = sink
            .emit(&event(RuntimeEventKind::RunStarted { prompt: "x".into() }))
            .expect_err("writer failure should propagate");
        assert!(error.message.contains("sink write failed"));
    }

    fn emit(sink: &mut HumanEventSink<Vec<u8>>, kind: RuntimeEventKind) {
        sink.emit(&event(kind)).expect("event should render");
    }

    fn event(kind: RuntimeEventKind) -> RuntimeEvent {
        RuntimeEvent {
            v: PROTOCOL_VERSION,
            session_id: "session-1".into(),
            run_id: "run-1".into(),
            seq: 1,
            emitted_at_ms: 0,
            kind,
        }
    }

    fn text(sink: HumanEventSink<Vec<u8>>) -> String {
        String::from_utf8(sink.into_inner()).expect("sink output should be UTF-8")
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("sink write failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
