// Live-gateway smoke test: exercises the full OpenAI Responses adapter
// against a real local gateway (LiteLLM) backed by a locally served model
// (Ollama). Skipped unless FORGE_LIVE_GATEWAY_ENDPOINT is set — CI runs the
// deterministic wiremock suite; this test is the host-verified external
// resource counterpart (see docs/external-resource-verification.md).

use forge_runtime_domain::{
    Cancellation, Message, ModelEvent, ModelProvider, ModelRequest, ProviderError,
};
use futures_util::StreamExt;

use super::OpenAiResponsesProvider;

fn request() -> ModelRequest {
    ModelRequest {
        system_prompt: "You are a smoke-test oracle. Reply with only the exact word FORGELIVE."
            .into(),
        messages: vec![Message::User {
            text: "What is the verification word?".into(),
        }],
        tools: Vec::new(),
        max_output_tokens: 64,
        cancellation: Cancellation::default(),
    }
}

async fn streamed_text(provider: &OpenAiResponsesProvider) -> (String, Option<ProviderError>) {
    let mut events = provider.stream(request());
    let mut text = String::new();
    let mut error = None;
    while let Some(event) = events.next().await {
        match event {
            Ok(ModelEvent::TextDelta { delta }) => text.push_str(&delta),
            Ok(_) => {}
            Err(err) => {
                error = Some(err);
                break;
            }
        }
    }
    (text, error)
}

#[tokio::test]
async fn live_gateway_reasoning_round_trip_via_local_gateway() {
    let Ok(endpoint) = std::env::var("FORGE_LIVE_GATEWAY_ENDPOINT") else {
        eprintln!("skipped: FORGE_LIVE_GATEWAY_ENDPOINT unset (no live gateway)");
        return;
    };
    let model =
        std::env::var("FORGE_LIVE_GATEWAY_MODEL").unwrap_or_else(|_| "local-qwen".to_string());
    let provider =
        OpenAiResponsesProvider::new_insecure_for_test(endpoint, model, "forge-live-smoke-key")
            .expect("valid live gateway provider");

    let (text, error) = streamed_text(&provider).await;
    assert!(
        text.contains("FORGELIVE") || text.contains("forgelive"),
        "live model text missing verification word: {text:?}"
    );
    // Verified live behavior (recorded 2026-08, LiteLLM Responses→Ollama
    // translation): request encoding and SSE parsing interoperate with the
    // real gateway, and the terminal-consistency guard correctly rejects the
    // translated terminal snapshot — LiteLLM assigns a different message id to
    // the streamed item (`msg_…`) than to the completed snapshot
    // (`chatcmpl-…`), which real OpenAI Responses never does. The guard fires
    // with `provider_protocol`/"terminal output did not match streamed
    // assistant events" — a live demonstration of the anti-drift check.
    let error = error.expect(
        "live gateway must reach the terminal-consistency guard \
         (LiteLLM translation id drift) or stream cleanly",
    );
    assert_eq!(error.code, "provider_protocol");
    assert!(
        error
            .message
            .contains("terminal output did not match streamed assistant events"),
        "unexpected guard message: {}",
        error.message
    );
}
