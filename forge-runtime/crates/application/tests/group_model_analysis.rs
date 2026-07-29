#[path = "group_model_analysis_support/fixture.rs"]
mod group_model_analysis_fixture;
#[path = "group_model_analysis_support/store.rs"]
mod group_model_analysis_support;

use forge_runtime_application::{
    GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT, GroupModelAnalysisServiceError,
    SendGroupModelAnalysisResult,
};
use forge_runtime_domain::{
    Cancellation, GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, GroupModelAnalysisOutcome,
    GroupModelAnalysisRecovery, MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS,
    MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES, Message, ModelEvent, ModelFinishReason,
    PrepareGroupModelAnalysisDisposition, ProviderError, ToolCall, Usage,
};
use serde_json::Value;

use group_model_analysis_fixture::{
    ANALYSIS_ID, GROUP_RUN_ID, PendingProvider, ScriptedProvider, canonical, claim_request, digest,
    harness, prepare_input,
};

#[test]
fn prepare_is_local_and_persists_the_fixed_exact_request() {
    let harness = harness();
    let prepared = harness
        .service
        .prepare(&prepare_input(64))
        .expect("prepare");

    assert_eq!(
        prepared.disposition,
        PrepareGroupModelAnalysisDisposition::Created
    );
    assert!(matches!(
        prepared.inspection.recovery,
        GroupModelAnalysisRecovery::AwaitingConsent
    ));
    let body = harness.analyses.request_body();
    let request: Value = serde_json::from_slice(&body).expect("request JSON");
    let candidate = harness.analyses.prepared_request();
    let config_bytes = canonical(&candidate.request_config).expect("canonical config");
    assert_eq!(request["system_prompt"], GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT);
    assert_eq!(
        request["messages"],
        serde_json::to_value([Message::User {
            text: harness.context_json.clone(),
        }])
        .expect("messages")
    );
    assert_eq!(request["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(request["tools"], serde_json::json!([]));
    assert_eq!(request["store"], false);
    assert_eq!(request["stream"], true);
    assert_eq!(candidate.config_json.as_bytes(), config_bytes);
    assert_eq!(
        candidate.config_sha256,
        digest(GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN, &config_bytes)
    );
    assert_eq!(candidate.request_body, body);
    assert_eq!(harness.codec.encode_calls(), 1);
    assert_eq!(
        harness.service.inspect(ANALYSIS_ID).expect("inspect"),
        prepared.inspection
    );
    assert_eq!(
        harness.service.list(Some(GROUP_RUN_ID), 1).expect("list"),
        vec![prepared.inspection.analysis]
    );
}

#[test]
fn inspect_and_list_reject_altered_application_owned_config() {
    let harness = prepared_harness(64);
    harness.analyses.corrupt_system_prompt_sha256();

    assert!(matches!(
        harness.service.inspect(ANALYSIS_ID),
        Err(GroupModelAnalysisServiceError::InconsistentStoreResult)
    ));
    assert!(matches!(
        harness.service.list(Some(GROUP_RUN_ID), 1),
        Err(GroupModelAnalysisServiceError::InconsistentStoreResult)
    ));
}

#[tokio::test]
async fn claimed_sender_dispatches_exact_bytes_and_stores_canonical_result() {
    let harness = prepared_harness(64);
    let exact_body = harness.analyses.request_body();
    let provider = ScriptedProvider::new(success_events(ModelFinishReason::Completed));

    let sent = harness
        .service
        .send(&claim_request(), &provider, Cancellation::default(), 30)
        .await
        .expect("send");
    let SendGroupModelAnalysisResult::Completed { completion } = sent else {
        panic!("claimed sender must complete");
    };
    let artifact = completion.inspection.result.expect("result");
    let result_bytes = canonical(&artifact.result).expect("canonical result");

    assert_eq!(provider.calls(), 1);
    assert_eq!(provider.first_body(), exact_body);
    assert_eq!(artifact.result.answer, "cross-project analysis");
    assert!(!artifact.result.answer.contains("DOSSIER-CONTEXT-SENTINEL"));
    assert_eq!(
        artifact.result.outcome,
        GroupModelAnalysisOutcome::Completed
    );
    assert_eq!(artifact.result_bytes, result_bytes.len());
    assert_eq!(
        artifact.result_sha256,
        digest(GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, &result_bytes)
    );
    assert_eq!(harness.analyses.claim_calls(), 1);
    assert_eq!(harness.analyses.complete_calls(), 1);
}

#[tokio::test]
async fn length_terminal_is_a_valid_distinct_completion() {
    let harness = prepared_harness(64);
    let provider = ScriptedProvider::new(success_events(ModelFinishReason::Length));

    let sent = harness
        .service
        .send(&claim_request(), &provider, Cancellation::default(), 30)
        .await
        .expect("length completion");
    let SendGroupModelAnalysisResult::Completed { completion } = sent else {
        panic!("length terminal must complete");
    };
    assert!(matches!(
        completion.inspection.recovery,
        GroupModelAnalysisRecovery::Terminal {
            outcome: GroupModelAnalysisOutcome::Length
        }
    ));
}

#[tokio::test]
async fn already_claimed_never_dispatches_a_second_time() {
    let harness = prepared_harness(64);
    let first = ScriptedProvider::new(success_events(ModelFinishReason::Completed));
    harness
        .service
        .send(&claim_request(), &first, Cancellation::default(), 30)
        .await
        .expect("first send");
    let second = ScriptedProvider::new(success_events(ModelFinishReason::Completed));

    let replay = harness
        .service
        .send(&claim_request(), &second, Cancellation::default(), 31)
        .await
        .expect("already claimed");

    assert!(matches!(
        replay,
        SendGroupModelAnalysisResult::AlreadyClaimed { .. }
    ));
    assert_eq!(first.calls(), 1);
    assert_eq!(second.calls(), 0);
    assert_eq!(harness.analyses.claim_calls(), 2);
    assert_eq!(harness.analyses.complete_calls(), 1);
}

#[tokio::test]
async fn every_post_claim_failure_stays_unknown_and_disables_resend() {
    assert_unknown(
        vec![Err(ProviderError::new("offline", "sentinel", true))],
        64,
        false,
        0,
    )
    .await;
    assert_unknown(vec![Ok(tool_call())], 64, false, 0).await;
    assert_unknown(tool_use_events(), 64, false, 0).await;
    assert_unknown(
        vec![Ok(ModelEvent::TextDelta {
            delta: "unterminated".into(),
        })],
        64,
        false,
        0,
    )
    .await;
    assert_unknown(output_limit_events(), 1, false, 0).await;
    assert_unknown(oversized_output_events(), 64, false, 0).await;
    assert_unknown(trailing_event_after_terminal(), 64, false, 0).await;
    assert_unknown(too_many_events(), 64, false, 0).await;
    assert_unknown(success_events(ModelFinishReason::Completed), 64, true, 1).await;
}

#[tokio::test]
async fn provider_target_mismatch_is_rejected_before_claim() {
    for provider in [
        ScriptedProvider::new(success_events(ModelFinishReason::Completed))
            .with_target("https://example.invalid/v1/responses", "gpt-test"),
        ScriptedProvider::new(success_events(ModelFinishReason::Completed))
            .with_target("https://api.openai.com/v1/responses", "different-model"),
    ] {
        let harness = prepared_harness(64);
        let error = harness
            .service
            .send(&claim_request(), &provider, Cancellation::default(), 30)
            .await
            .expect_err("target mismatch");

        assert!(matches!(
            error,
            GroupModelAnalysisServiceError::InvalidInput
        ));
        assert_eq!(harness.analyses.claim_calls(), 0);
        assert_eq!(provider.calls(), 0);
    }
}

#[tokio::test]
async fn cancellation_after_claim_stays_unknown() {
    let harness = prepared_harness(64);
    let provider = PendingProvider::default();
    let cancellation = Cancellation::default();
    let trigger = cancellation.clone();
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        trigger.cancel();
    });

    let error = harness
        .service
        .send(&claim_request(), &provider, cancellation, 30)
        .await
        .expect_err("cancelled dispatch");

    assert!(matches!(
        error,
        GroupModelAnalysisServiceError::DispatchUnknown
    ));
    assert_eq!(provider.calls(), 1);
    assert_eq!(harness.analyses.claim_calls(), 1);
    assert_eq!(harness.analyses.complete_calls(), 0);
    assert!(matches!(
        harness
            .service
            .inspect(ANALYSIS_ID)
            .expect("unknown inspection")
            .recovery,
        GroupModelAnalysisRecovery::DispatchUnknown { .. }
    ));
}

#[tokio::test]
async fn invalid_send_input_does_not_claim_or_dispatch() {
    let harness = prepared_harness(64);
    let provider = ScriptedProvider::new(success_events(ModelFinishReason::Completed));

    let error = harness
        .service
        .send(&claim_request(), &provider, Cancellation::default(), 19)
        .await
        .expect_err("completion before release");

    assert!(matches!(
        error,
        GroupModelAnalysisServiceError::InvalidInput
    ));
    assert_eq!(harness.analyses.claim_calls(), 0);
    assert_eq!(provider.calls(), 0);
}

async fn assert_unknown(
    events: Vec<Result<ModelEvent, ProviderError>>,
    max_output_tokens: u32,
    fail_completion: bool,
    expected_complete_calls: usize,
) {
    let harness = prepared_harness(max_output_tokens);
    if fail_completion {
        harness.analyses.fail_completion();
    }
    let provider = ScriptedProvider::new(events);
    let error = harness
        .service
        .send(&claim_request(), &provider, Cancellation::default(), 30)
        .await
        .expect_err("post-claim failure");
    assert!(matches!(
        error,
        GroupModelAnalysisServiceError::DispatchUnknown
    ));
    let rendered = error.to_string();
    assert!(!rendered.contains("sentinel"));
    assert!(!rendered.contains("completion sentinel"));
    assert_eq!(
        rendered,
        "Group Model Analysis dispatch outcome is unknown; automatic retry is disabled"
    );
    assert!(matches!(
        harness
            .service
            .inspect(ANALYSIS_ID)
            .expect("unknown inspection")
            .recovery,
        GroupModelAnalysisRecovery::DispatchUnknown { .. }
    ));
    assert_eq!(provider.calls(), 1);
    assert_eq!(harness.analyses.complete_calls(), expected_complete_calls);
    assert_no_resend(&harness).await;
}

async fn assert_no_resend(harness: &group_model_analysis_fixture::Harness) {
    let retry = ScriptedProvider::new(success_events(ModelFinishReason::Completed));
    let replay = harness
        .service
        .send(&claim_request(), &retry, Cancellation::default(), 31)
        .await
        .expect("already claimed after failure");
    assert!(matches!(
        replay,
        SendGroupModelAnalysisResult::AlreadyClaimed { .. }
    ));
    assert_eq!(retry.calls(), 0);
}

fn prepared_harness(max_output_tokens: u32) -> group_model_analysis_fixture::Harness {
    let harness = harness();
    harness
        .service
        .prepare(&prepare_input(max_output_tokens))
        .expect("prepare");
    harness
}

fn success_events(reason: ModelFinishReason) -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(ModelEvent::TextDelta {
            delta: "cross-project analysis".into(),
        }),
        Ok(ModelEvent::ProviderContext {
            provider: "openai".into(),
            items: vec![serde_json::json!({
                "reasoning": "DOSSIER-CONTEXT-SENTINEL"
            })],
        }),
        Ok(ModelEvent::Usage {
            usage: Usage {
                input_tokens: 20,
                output_tokens: 3,
            },
        }),
        Ok(ModelEvent::Finished { reason }),
    ]
}

fn tool_use_events() -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(ModelEvent::TextDelta {
            delta: "must not complete".into(),
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::ToolUse,
        }),
    ]
}

fn oversized_output_events() -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(ModelEvent::TextDelta {
            delta: "x".repeat(MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES + 1),
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Length,
        }),
    ]
}

fn trailing_event_after_terminal() -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(ModelEvent::TextDelta {
            delta: "must remain unknown".into(),
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
        Ok(ModelEvent::TextDelta {
            delta: "post-terminal sentinel".into(),
        }),
    ]
}

fn too_many_events() -> Vec<Result<ModelEvent, ProviderError>> {
    let mut events = (0..MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS)
        .map(|_| Ok(ModelEvent::TextDelta { delta: "x".into() }))
        .collect::<Vec<_>>();
    events.push(Ok(ModelEvent::Finished {
        reason: ModelFinishReason::Completed,
    }));
    events
}

fn output_limit_events() -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(ModelEvent::TextDelta {
            delta: "answer".into(),
        }),
        Ok(ModelEvent::Usage {
            usage: Usage {
                input_tokens: 1,
                output_tokens: 2,
            },
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ]
}

fn tool_call() -> ModelEvent {
    ModelEvent::ToolCall {
        call: ToolCall {
            id: "call-1".into(),
            name: "forbidden".into(),
            arguments: serde_json::json!({}),
        },
    }
}
