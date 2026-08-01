#[allow(dead_code, unused_imports)]
#[path = "group_analysis_panel_support/mod.rs"]
mod group_analysis_panel_support;
#[path = "group_panel_synthesis_support/mod.rs"]
mod support;

use forge_runtime_application::{
    GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT, GroupPanelSynthesisServiceError,
    SendGroupPanelSynthesisResult,
};
use forge_runtime_domain::{
    Cancellation, GroupAnalysisPanelManifest, GroupModelAnalysisOutcome,
    GroupPanelSynthesisOutcome, GroupPanelSynthesisOutputTarget, GroupPanelSynthesisRecovery,
    GroupPanelSynthesisWritebackTarget, ModelEvent, ModelFinishReason,
    PrepareGroupPanelSynthesisDisposition, ProviderError, Usage,
};

use support::{
    MODEL, PANEL_ID, SYNTHESIS_ID, ScriptedProvider, claim_request, harness, only_user_message,
    prepare_input, replace_analysis,
};

#[test]
fn prepare_persists_one_exact_panel_manifest_request_and_fixed_targets() {
    let harness = harness();
    let prepared = harness.service.prepare(&prepare_input()).expect("prepare");
    let candidate = harness.syntheses.prepared_request();
    let body = harness.syntheses.request_body();
    let request: serde_json::Value = serde_json::from_slice(&body).expect("request JSON");
    let manifest_text = only_user_message(&request).expect("one user message");
    let manifest: GroupAnalysisPanelManifest =
        serde_json::from_str(&manifest_text).expect("canonical panel manifest");

    assert_eq!(
        request["system_prompt"],
        GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT
    );
    assert_eq!(request["tools"], serde_json::json!([]));
    assert_eq!(request["store"], false);
    assert_eq!(request["stream"], true);
    assert_eq!(manifest.contributions.len(), 2);
    assert!(manifest_text.contains("frontend findings"));
    assert!(manifest_text.contains("backend findings"));
    assert!(!manifest_text.contains("\"context_json\""));
    assert_eq!(
        candidate.config.output_target,
        GroupPanelSynthesisOutputTarget::LocalArtifact
    );
    assert_eq!(
        candidate.config.writeback_target,
        GroupPanelSynthesisWritebackTarget::None
    );
    assert_eq!(candidate.request_body, body);
    assert_eq!(prepared.inspection.synthesis.panel_id, PANEL_ID);
    assert!(matches!(
        prepared.inspection.recovery,
        GroupPanelSynthesisRecovery::AwaitingConsent
    ));
}

#[test]
fn prepare_replay_keeps_the_original_identity_time_and_request() {
    let harness = harness();
    let first = harness.service.prepare(&prepare_input()).expect("prepare");
    let mut retry = prepare_input();
    retry.synthesis_id = "ignored-retry-candidate".into();
    retry.created_at_ms += 1;

    let replay = harness.service.prepare(&retry).expect("exact replay");

    assert_eq!(
        replay.disposition,
        PrepareGroupPanelSynthesisDisposition::Replayed
    );
    assert_eq!(replay.inspection, first.inspection);
}

#[tokio::test]
async fn explicit_consent_dispatches_exact_bytes_and_stores_canonical_result() {
    let harness = prepared_harness();
    let exact = harness.syntheses.request_body();
    let provider = ScriptedProvider::new(success_events(ModelFinishReason::Completed));
    let sent = harness
        .service
        .send(
            &claim_request(),
            true,
            &provider,
            Cancellation::default(),
            80,
        )
        .await
        .expect("send");
    let SendGroupPanelSynthesisResult::Completed { completion } = sent else {
        panic!("fresh claim completes");
    };
    let artifact = completion.inspection.result.expect("result");

    assert_eq!(provider.calls(), 1);
    assert_eq!(provider.first_body(), exact);
    assert_eq!(artifact.result.answer, "moderated panel synthesis");
    assert_eq!(
        artifact.result.outcome,
        GroupPanelSynthesisOutcome::Completed
    );
    assert!(!artifact.result.answer.contains("PRIVATE-CONTEXT"));
    assert_eq!(harness.syntheses.complete_calls(), 1);
}

#[tokio::test]
async fn absent_consent_or_wrong_target_never_claims_or_dispatches() {
    let harness = prepared_harness();
    let provider = ScriptedProvider::new(success_events(ModelFinishReason::Completed));
    let error = harness
        .service
        .send(
            &claim_request(),
            false,
            &provider,
            Cancellation::default(),
            80,
        )
        .await
        .expect_err("consent required");
    assert!(matches!(
        error,
        GroupPanelSynthesisServiceError::InvalidInput
    ));
    assert_eq!(harness.syntheses.claim_calls(), 0);
    assert_eq!(provider.calls(), 0);

    let wrong = ScriptedProvider::new(success_events(ModelFinishReason::Completed))
        .with_target("https://example.invalid/v1/responses", MODEL);
    assert!(matches!(
        harness
            .service
            .send(&claim_request(), true, &wrong, Cancellation::default(), 80)
            .await,
        Err(GroupPanelSynthesisServiceError::InvalidInput)
    ));
    assert_eq!(harness.syntheses.claim_calls(), 0);
    assert_eq!(wrong.calls(), 0);
}

#[tokio::test]
async fn every_post_claim_failure_is_unknown_and_cannot_resend() {
    let harness = prepared_harness();
    let failing = ScriptedProvider::new(vec![Err(ProviderError::new(
        "offline",
        "PRIVATE-CONTEXT",
        true,
    ))]);
    let error = harness
        .service
        .send(
            &claim_request(),
            true,
            &failing,
            Cancellation::default(),
            80,
        )
        .await
        .expect_err("post-claim error");
    assert!(matches!(
        error,
        GroupPanelSynthesisServiceError::DispatchUnknown
    ));
    assert!(!error.to_string().contains("PRIVATE-CONTEXT"));

    let retry = ScriptedProvider::new(success_events(ModelFinishReason::Completed));
    let replay = harness
        .service
        .send(&claim_request(), true, &retry, Cancellation::default(), 81)
        .await
        .expect("already claimed");
    assert!(matches!(
        replay,
        SendGroupPanelSynthesisResult::AlreadyClaimed { .. }
    ));
    assert_eq!(retry.calls(), 0);
}

#[tokio::test]
async fn completion_store_failure_is_unknown_and_cannot_authorize_retry() {
    let store_failure = prepared_harness();
    store_failure.syntheses.fail_completion();
    let provider = ScriptedProvider::new(success_events(ModelFinishReason::Completed));
    assert!(matches!(
        store_failure
            .service
            .send(
                &claim_request(),
                true,
                &provider,
                Cancellation::default(),
                80
            )
            .await,
        Err(GroupPanelSynthesisServiceError::DispatchUnknown)
    ));
    assert_eq!(store_failure.syntheses.complete_calls(), 1);
}

#[tokio::test]
async fn terminal_without_usage_stays_dispatch_unknown() {
    let harness = prepared_harness();
    let provider = ScriptedProvider::new(vec![
        Ok(ModelEvent::TextDelta {
            delta: "answer without metering".into(),
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ]);
    assert!(matches!(
        harness
            .service
            .send(
                &claim_request(),
                true,
                &provider,
                Cancellation::default(),
                80
            )
            .await,
        Err(GroupPanelSynthesisServiceError::DispatchUnknown)
    ));
    assert_eq!(provider.calls(), 1);
    assert_eq!(harness.syntheses.complete_calls(), 0);
    assert!(matches!(
        harness
            .service
            .inspect(SYNTHESIS_ID)
            .expect("inspect")
            .recovery,
        GroupPanelSynthesisRecovery::DispatchUnknown { .. }
    ));
}

#[test]
fn inspect_revalidates_every_panel_source_analysis() {
    let harness = prepared_harness();
    let changed = group_analysis_panel_support::completed_analysis(
        "analysis-a",
        &harness.snapshot,
        GroupModelAnalysisOutcome::Completed,
        "changed after synthesis preparation",
    );
    replace_analysis(&harness, "analysis-a", changed);

    assert!(matches!(
        harness.service.inspect(SYNTHESIS_ID),
        Err(GroupPanelSynthesisServiceError::InconsistentStoreResult)
    ));
}

#[test]
fn list_is_bounded_and_filtered_metadata() {
    let harness = prepared_harness();
    let listed = harness
        .service
        .list(Some(PANEL_ID), 1)
        .expect("bounded list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].synthesis_id, SYNTHESIS_ID);
    assert!(matches!(
        harness.service.list(Some(PANEL_ID), 0),
        Err(GroupPanelSynthesisServiceError::InvalidInput)
    ));
}

fn prepared_harness() -> support::Harness {
    let harness = harness();
    harness.service.prepare(&prepare_input()).expect("prepare");
    harness
}

fn success_events(reason: ModelFinishReason) -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(ModelEvent::TextDelta {
            delta: "moderated panel synthesis".into(),
        }),
        Ok(ModelEvent::ProviderContext {
            provider: "openai".into(),
            items: vec![serde_json::json!({"reasoning": "PRIVATE-CONTEXT"})],
        }),
        Ok(ModelEvent::Usage {
            usage: Usage {
                input_tokens: 30,
                output_tokens: 4,
            },
        }),
        Ok(ModelEvent::Finished { reason }),
    ]
}
