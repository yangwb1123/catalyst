use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use crate::runtime_domain::{
    ModelEvent, ModelEventStream, ModelFinishReason, PreparedModelProvider, PreparedModelRequest,
    ProviderError, ToolCall, Usage,
};
use futures_util::stream;
use serde_json::json;

use super::*;

struct ScriptedProvider {
    events: Mutex<Option<Vec<Result<ModelEvent, ProviderError>>>>,
    calls: AtomicUsize,
}

impl ScriptedProvider {
    fn new(events: Vec<Result<ModelEvent, ProviderError>>) -> Self {
        Self {
            events: Mutex::new(Some(events)),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }
}

impl PreparedModelProvider for ScriptedProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        self.calls.fetch_add(1, Ordering::AcqRel);
        assert_eq!(request.body(), b"exact-body");
        let events = self
            .events
            .lock()
            .expect("scripted events")
            .take()
            .unwrap_or_default();
        Box::pin(stream::iter(events))
    }
}

struct PendingProvider;

impl PreparedModelProvider for PendingProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        assert_eq!(request.body(), b"exact-body");
        Box::pin(stream::pending())
    }
}

#[tokio::test]
async fn completed_terminal_requires_usage_and_true_eof() {
    let provider = ScriptedProvider::new(completed_events());
    let evidence = collect(&provider, limits()).await;

    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::Completed
    );
    assert_eq!(evidence.output, "answer");
    assert_eq!(
        evidence.usage,
        Some(Usage {
            input_tokens: 11,
            output_tokens: 2,
        })
    );
    assert!(evidence.provider_poll_started);
    assert!(evidence.terminal_seen);
    assert!(evidence.stream_eof_seen);
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn terminal_without_usage_is_uncertain() {
    let provider = ScriptedProvider::new(vec![
        Ok(text("partial")),
        Ok(terminal(ModelFinishReason::Completed)),
    ]);
    let evidence = collect(&provider, limits()).await;

    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::MissingUsage
    );
    assert_eq!(evidence.output, "partial");
    assert_eq!(evidence.usage, None);
    assert!(evidence.terminal_seen);
    assert!(evidence.stream_eof_seen);
}

#[tokio::test]
async fn event_after_terminal_is_trailing_data() {
    let mut events = completed_events();
    events.push(Ok(text("must-not-be-appended")));
    let evidence = collect(&ScriptedProvider::new(events), limits()).await;

    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::TrailingData
    );
    assert_eq!(evidence.output, "answer");
    assert!(evidence.terminal_seen);
    assert!(!evidence.stream_eof_seen);
}

#[tokio::test]
async fn retryable_hint_does_not_change_provider_error_or_trigger_retry() {
    let provider = ScriptedProvider::new(vec![Err(ProviderError::new(
        "overloaded",
        "secret provider detail",
        true,
    ))]);
    let evidence = collect(&provider, limits()).await;

    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::ProviderError
    );
    assert!(evidence.output.is_empty());
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn provider_error_codes_map_to_every_transport_uncertainty_class() {
    let cases = [
        ("cancelled", GroupAgentNodeTerminalClassification::Cancelled),
        (
            "transport_error",
            GroupAgentNodeTerminalClassification::TransportError,
        ),
        (
            "stream_ended",
            GroupAgentNodeTerminalClassification::EofBeforeTerminal,
        ),
        (
            "provider_protocol",
            GroupAgentNodeTerminalClassification::ProtocolError,
        ),
        ("http_429", GroupAgentNodeTerminalClassification::HttpError),
        ("http_500", GroupAgentNodeTerminalClassification::HttpError),
        (
            "provider_failure",
            GroupAgentNodeTerminalClassification::ProviderError,
        ),
    ];
    for (code, expected) in cases {
        let provider = ScriptedProvider::new(vec![Err(ProviderError::new(
            code,
            "private provider detail",
            true,
        ))]);
        let evidence = collect(&provider, limits()).await;
        assert_eq!(evidence.classification, expected, "code {code}");
        assert_eq!(provider.calls(), 1);
    }
}

#[tokio::test]
async fn clean_eof_before_any_terminal_is_uncertain() {
    let evidence = collect(&ScriptedProvider::new(Vec::new()), limits()).await;
    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::EofBeforeTerminal
    );
    assert!(evidence.stream_eof_seen);
    assert!(!evidence.terminal_seen);
}

#[tokio::test]
async fn every_local_budget_fails_closed() {
    let output = collect(
        &ScriptedProvider::new(vec![Ok(text("12345"))]),
        DispatchCollectionLimits {
            model_output_bytes: 4,
            ..limits()
        },
    )
    .await;
    let events = collect(
        &ScriptedProvider::new(vec![Ok(text("a")), Ok(text("b"))]),
        DispatchCollectionLimits {
            events: 1,
            ..limits()
        },
    )
    .await;
    let usage = collect(
        &ScriptedProvider::new(vec![Ok(usage(1, 3))]),
        DispatchCollectionLimits {
            output_tokens: 2,
            ..limits()
        },
    )
    .await;

    for evidence in [output, events, usage] {
        assert_eq!(
            evidence.classification,
            GroupAgentNodeTerminalClassification::LocalLimit
        );
    }
}

#[tokio::test]
async fn result_bytes_are_bounded_independently_from_model_stream_bytes() {
    let evidence = collect(
        &ScriptedProvider::new(vec![Ok(text("12345"))]),
        DispatchCollectionLimits {
            model_output_bytes: 1024,
            result_bytes: 4,
            ..limits()
        },
    )
    .await;

    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::LocalLimit
    );
    assert!(evidence.output.is_empty());
}

#[tokio::test]
async fn tool_calls_are_never_executed() {
    let provider = ScriptedProvider::new(vec![Ok(ModelEvent::ToolCall {
        call: ToolCall {
            id: "call-1".into(),
            name: "forbidden".into(),
            arguments: json!({}),
        },
    })]);
    let evidence = collect(&provider, limits()).await;

    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::ToolCall
    );
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn cancellation_wakes_a_pending_provider() {
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    let run = collect_dispatch(
        &PendingProvider,
        b"exact-body".to_vec(),
        &cancellation,
        limits(),
    );
    let cancel_soon = async move {
        tokio::task::yield_now().await;
        cancel.cancel();
    };
    let (evidence, ()) = tokio::join!(run, cancel_soon);

    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::Cancelled
    );
    assert!(evidence.provider_poll_started);
}

#[tokio::test]
async fn timeout_bounds_a_pending_provider() {
    let evidence = collect(
        &PendingProvider,
        DispatchCollectionLimits {
            timeout_ms: 1,
            ..limits()
        },
    )
    .await;

    assert_eq!(
        evidence.classification,
        GroupAgentNodeTerminalClassification::Timeout
    );
    assert!(evidence.provider_poll_started);
}

async fn collect(
    provider: &dyn PreparedModelProvider,
    limits: DispatchCollectionLimits,
) -> CollectedDispatchEvidence {
    collect_dispatch(
        provider,
        b"exact-body".to_vec(),
        &Cancellation::default(),
        limits,
    )
    .await
}

fn limits() -> DispatchCollectionLimits {
    DispatchCollectionLimits {
        model_output_bytes: 1024,
        result_bytes: 1024,
        output_tokens: 32,
        events: 16,
        timeout_ms: 1_000,
    }
}

fn completed_events() -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(text("answer")),
        Ok(usage(11, 2)),
        Ok(terminal(ModelFinishReason::Completed)),
    ]
}

fn text(delta: &str) -> ModelEvent {
    ModelEvent::TextDelta {
        delta: delta.into(),
    }
}

fn usage(input_tokens: u64, output_tokens: u64) -> ModelEvent {
    ModelEvent::Usage {
        usage: Usage {
            input_tokens,
            output_tokens,
        },
    }
}

fn terminal(reason: ModelFinishReason) -> ModelEvent {
    ModelEvent::Finished { reason }
}
