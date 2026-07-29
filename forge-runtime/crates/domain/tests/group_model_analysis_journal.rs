mod support;

use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatch, GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
    GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION, GROUP_MODEL_ANALYSIS_VERSION,
    GroupModelAnalysisDispatchAuthority, GroupModelAnalysisDispatchClaim, GroupModelAnalysisEvent,
    GroupModelAnalysisEventKind, GroupModelAnalysisInspection, GroupModelAnalysisJournalCursor,
    GroupModelAnalysisOutcome, GroupModelAnalysisPreparedReceipt, GroupModelAnalysisRecord,
    GroupModelAnalysisRecovery, GroupModelAnalysisResultArtifact, GroupModelAnalysisResultReceipt,
    GroupModelAnalysisStatus, MAX_GROUP_MODEL_ANALYSIS_EVENTS, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS, MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS,
};
use serde_json::json;
use support::group_model_analysis::{
    BODY, artifact, claim, config, crate_config, digest, events, inspect, record, request_config,
    request_digest, source,
};

#[test]
fn complete_transcripts_bind_both_terminal_outcomes() {
    for outcome in [
        GroupModelAnalysisOutcome::Completed,
        GroupModelAnalysisOutcome::Length,
    ] {
        let inspection = inspect(3, outcome).expect("valid terminal transcript");

        assert_eq!(
            inspection.recovery,
            GroupModelAnalysisRecovery::Terminal { outcome }
        );
        assert!(inspection.prepared.is_some());
        assert!(inspection.dispatch.is_some());
        assert!(inspection.completion.is_some());
        assert_eq!(
            inspection.result.as_ref().map(|value| value.result.outcome),
            Some(outcome)
        );
    }
}

#[test]
fn durable_inspection_rejects_an_empty_journal_but_cursor_can_be_unprepared() {
    let record = record(GroupModelAnalysisStatus::AwaitingConsent);
    let cursor = GroupModelAnalysisJournalCursor::new(&record).expect("transient cursor");

    assert_eq!(cursor.next_sequence(), 1);
    assert_eq!(cursor.recovery(), GroupModelAnalysisRecovery::Unprepared);
    cursor
        .validate_record(&record)
        .expect("an in-transaction cursor may still need preparation");

    let error = GroupModelAnalysisInspection::validate(record, Vec::new(), None)
        .expect_err("durable zero-event analysis must be rejected");
    assert!(error.message.contains("preparation event"));
}

#[test]
fn journal_prefixes_have_exact_status_and_recovery() {
    let awaiting = inspect(1, GroupModelAnalysisOutcome::Completed).expect("prepared prefix");
    assert_eq!(
        awaiting.recovery,
        GroupModelAnalysisRecovery::AwaitingConsent
    );

    let uncertain = inspect(2, GroupModelAnalysisOutcome::Completed).expect("claimed prefix");
    assert_eq!(
        uncertain.recovery,
        GroupModelAnalysisRecovery::DispatchUnknown {
            dispatch_id: "dispatch-1".into()
        }
    );

    for (event_count, bad_status) in [
        (1, GroupModelAnalysisStatus::DispatchUnknown),
        (2, GroupModelAnalysisStatus::AwaitingConsent),
        (3, GroupModelAnalysisStatus::DispatchUnknown),
    ] {
        let mut events = events(GroupModelAnalysisOutcome::Completed);
        events.truncate(event_count);
        let result = (event_count == 3).then(|| artifact(GroupModelAnalysisOutcome::Completed));
        assert!(
            GroupModelAnalysisInspection::validate(record(bad_status), events, result).is_err()
        );
    }
}

#[test]
fn event_order_envelope_and_terminal_are_strict_and_fail_without_mutation() {
    let awaiting_record = record(GroupModelAnalysisStatus::AwaitingConsent);
    let all = events(GroupModelAnalysisOutcome::Completed);
    let mut cursor = GroupModelAnalysisJournalCursor::new(&awaiting_record).expect("cursor");

    for bad in [
        GroupModelAnalysisEvent {
            seq: 2,
            ..all[0].clone()
        },
        GroupModelAnalysisEvent {
            v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION + 1,
            ..all[0].clone()
        },
        GroupModelAnalysisEvent {
            analysis_id: "other-analysis".into(),
            ..all[0].clone()
        },
        GroupModelAnalysisEvent {
            seq: 1,
            kind: all[2].kind.clone(),
            ..all[0].clone()
        },
    ] {
        assert!(cursor.append(&bad).is_err());
        assert_eq!(cursor.next_sequence(), 1);
        assert_eq!(cursor.recovery(), GroupModelAnalysisRecovery::Unprepared);
    }

    cursor.append(&all[0]).expect("prepare");
    assert!(cursor.append(&all[0]).is_err(), "duplicate sequence");
    cursor.append(&all[1]).expect("claim");
    cursor.append(&all[2]).expect("complete");
    assert_terminal_is_final(&mut cursor, all);
}

fn assert_terminal_is_final(
    cursor: &mut GroupModelAnalysisJournalCursor,
    mut all: Vec<GroupModelAnalysisEvent>,
) {
    let after_terminal = GroupModelAnalysisEvent {
        seq: 4,
        ..all[2].clone()
    };
    assert!(cursor.append(&after_terminal).is_err());
    assert_eq!(cursor.next_sequence(), 4);

    all.push(after_terminal);
    assert_eq!(all.len(), MAX_GROUP_MODEL_ANALYSIS_EVENTS + 1);
    assert!(
        GroupModelAnalysisInspection::validate(
            record(GroupModelAnalysisStatus::Completed),
            all,
            Some(artifact(GroupModelAnalysisOutcome::Completed)),
        )
        .is_err()
    );
}

#[test]
fn prepared_event_is_bound_to_source_config_and_request() {
    assert_prepared_rejected(|receipt| receipt.analysis_id = "other-analysis".into());
    assert_prepared_rejected(|receipt| receipt.source.group_run_id = "other-run".into());
    assert_prepared_rejected(|receipt| receipt.source.group_id = "bad\u{202e}id".into());
    assert_prepared_rejected(|receipt| receipt.source.snapshot_sha256 = digest('9'));
    assert_prepared_rejected(|receipt| receipt.source.context_slice_sha256 = "not-a-digest".into());
    assert_prepared_rejected(|receipt| receipt.source.snapshot_bytes = 0);
    assert_prepared_rejected(|receipt| receipt.config_sha256 = digest('8'));
    assert_prepared_rejected(|receipt| receipt.request_sha256 = digest('7'));
    assert_prepared_rejected(|receipt| receipt.request_bytes += 1);
}

#[test]
fn dispatch_claim_is_bound_and_rejects_untrusted_identifiers() {
    assert_claim_rejected(|claim| claim.analysis_id = "other-analysis".into());
    assert_claim_rejected(|claim| claim.dispatch_id.clear());
    assert_claim_rejected(|claim| claim.dispatch_id = "bad\nid".into());
    assert_claim_rejected(|claim| claim.dispatch_id = "bad\u{2066}id".into());
    assert_claim_rejected(|claim| {
        claim.dispatch_id = "x".repeat(MAX_GROUP_MODEL_ANALYSIS_ID_BYTES + 1);
    });
    assert_claim_rejected(|claim| claim.request_sha256 = digest('8'));
    assert_claim_rejected(|claim| claim.config_sha256 = digest('8'));
    assert_claim_rejected(|claim| claim.endpoint = "https://example.invalid/v1".into());
    assert_claim_rejected(|claim| claim.model = "other-model".into());
    assert_claim_rejected(|claim| claim.consent_version += 1);
    assert_claim_rejected(|claim| claim.released_at_ms = 9);
}

#[test]
fn completion_receipt_and_result_are_exactly_bound() {
    assert_completion_rejected(
        |receipt| receipt.dispatch_id = "other-dispatch".into(),
        |_| {},
    );
    assert_completion_rejected(|receipt| receipt.request_sha256 = digest('8'), |_| {});
    assert_completion_rejected(
        |receipt| receipt.outcome = GroupModelAnalysisOutcome::Length,
        |_| {},
    );
    assert_completion_rejected(|receipt| receipt.result_sha256 = digest('8'), |_| {});
    assert_completion_rejected(|receipt| receipt.result_bytes += 1, |_| {});
    assert_completion_rejected(|receipt| receipt.usage.output_tokens += 1, |_| {});
    assert_completion_rejected(|receipt| receipt.created_at_ms = 19, |_| {});
    assert_completion_rejected(
        |_| {},
        |artifact| artifact.result.dispatch_id = "other-dispatch".into(),
    );
    assert_completion_rejected(
        |_| {},
        |artifact| artifact.result.answer = "x".repeat(MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES + 1),
    );
    assert_completion_rejected(|_| {}, |artifact| artifact.created_at_ms = 19);

    let all = events(GroupModelAnalysisOutcome::Completed);
    assert!(
        GroupModelAnalysisInspection::validate(
            record(GroupModelAnalysisStatus::Completed),
            all.clone(),
            None,
        )
        .is_err(),
        "terminal journal requires its result artifact"
    );
    assert!(
        GroupModelAnalysisInspection::validate(
            record(GroupModelAnalysisStatus::DispatchUnknown),
            all[..2].to_vec(),
            Some(artifact(GroupModelAnalysisOutcome::Completed)),
        )
        .is_err(),
        "a result cannot predate the terminal event"
    );
}

#[test]
fn restored_cursor_revalidates_embedded_evidence() {
    let awaiting_record = record(GroupModelAnalysisStatus::AwaitingConsent);
    let mut awaiting =
        GroupModelAnalysisJournalCursor::new(&awaiting_record).expect("awaiting cursor");
    awaiting
        .append(&events(GroupModelAnalysisOutcome::Completed)[0])
        .expect("prepared");

    assert_cursor_tamper_rejected(&awaiting, &awaiting_record, |value| {
        value["next_sequence"] = json!(3);
    });
    assert_cursor_tamper_rejected(&awaiting, &awaiting_record, |value| {
        value["state"]["AwaitingConsent"]["analysis_id"] = json!("other-analysis");
    });
    assert_cursor_tamper_rejected(&awaiting, &awaiting_record, |value| {
        value["request_sha256"] = json!(digest('8'));
    });
    assert_cursor_tamper_rejected(&awaiting, &awaiting_record, |value| {
        value["config"]["endpoint"] = json!("https://example.invalid");
    });

    let dispatch_record = record(GroupModelAnalysisStatus::DispatchUnknown);
    let mut dispatch =
        GroupModelAnalysisJournalCursor::new(&dispatch_record).expect("dispatch cursor");
    for event in &events(GroupModelAnalysisOutcome::Completed)[..2] {
        dispatch.append(event).expect("valid prefix");
    }
    assert_cursor_tamper_rejected(&dispatch, &dispatch_record, |value| {
        value["state"]["DispatchUnknown"]["claim"]["released_at_ms"] = json!(9);
    });

    let terminal_record = record(GroupModelAnalysisStatus::Completed);
    let mut terminal =
        GroupModelAnalysisJournalCursor::new(&terminal_record).expect("terminal cursor");
    for event in events(GroupModelAnalysisOutcome::Completed) {
        terminal.append(&event).expect("valid event");
    }
    assert_cursor_tamper_rejected(&terminal, &terminal_record, |value| {
        value["state"]["Terminal"]["completion"]["result_bytes"] = json!(0);
    });
}

#[test]
fn public_validation_enforces_limits_and_untrusted_id_policy() {
    let mut config = config();
    config.model.clear();
    assert!(config.validate().is_err());
    config = crate_config();
    config.max_output_tokens = MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS + 1;
    assert!(config.validate().is_err());
    config = crate_config();
    config.max_model_output_bytes = MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES + 1;
    assert!(config.validate().is_err());
    config = crate_config();
    config.max_model_events = MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS + 1;
    assert!(config.validate().is_err());

    let mut private = request_config();
    private.system_prompt.clear();
    assert!(private.validate().is_err());

    for bad in ["", " \t", "bad\nid", "bad\u{202e}id"] {
        let mut source = source();
        source.group_id = bad.into();
        assert!(source.validate().is_err());
    }
    let mut source = source();
    source.group_id = "x".repeat(MAX_GROUP_MODEL_ANALYSIS_ID_BYTES + 1);
    assert!(source.validate().is_err());

    let mut request = ClaimGroupModelAnalysisDispatch {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: "analysis-1".into(),
        dispatch_id: "dispatch-1".into(),
        consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
        released_at_ms: 20,
    };
    request.dispatch_id = "bad\u{061c}id".into();
    assert!(request.validate().is_err());

    let mut result = artifact(GroupModelAnalysisOutcome::Completed);
    result.result.usage.input_tokens = u64::MAX;
    assert!(result.validate().is_err());
}

#[test]
fn dispatch_authority_rehashes_bytes_and_is_consumed_for_access() {
    assert_eq!(
        request_digest(b"{}"),
        "7b4f2f50fec11ef6c145658b7f493d37dc69d6d3e2f2638538a6d67c90b23080"
    );
    let record = record(GroupModelAnalysisStatus::DispatchUnknown);
    let dispatch_claim = claim();
    let authority =
        GroupModelAnalysisDispatchAuthority::new(&record, dispatch_claim.clone(), BODY.to_vec())
            .expect("bound authority");
    assert_eq!(authority.version(), GROUP_MODEL_ANALYSIS_VERSION);
    assert_eq!(authority.claim(), &dispatch_claim);
    let (released_claim, released_body) = authority.into_parts();
    assert_eq!(released_claim, dispatch_claim);
    assert_eq!(released_body, BODY);

    let same_length_wrong_bytes = vec![b'x'; BODY.len()];
    assert!(
        GroupModelAnalysisDispatchAuthority::new(
            &record,
            released_claim.clone(),
            same_length_wrong_bytes,
        )
        .is_err(),
        "byte count alone must not grant dispatch authority"
    );
    assert!(
        GroupModelAnalysisDispatchAuthority::new(&record, released_claim, BODY[..1].to_vec())
            .is_err()
    );
    let mut wrong_claim = claim();
    wrong_claim.dispatch_id = "other-dispatch".into();
    // A fresh opaque dispatch identifier is valid; an actual binding mismatch must be used.
    wrong_claim.request_sha256 = digest('8');
    assert!(GroupModelAnalysisDispatchAuthority::new(&record, wrong_claim, BODY.to_vec()).is_err());
}

fn assert_prepared_rejected(change: impl FnOnce(&mut GroupModelAnalysisPreparedReceipt)) {
    let record = record(GroupModelAnalysisStatus::AwaitingConsent);
    let mut event = events(GroupModelAnalysisOutcome::Completed).remove(0);
    let GroupModelAnalysisEventKind::AnalysisPrepared { receipt } = &mut event.kind else {
        unreachable!()
    };
    change(receipt);
    assert!(GroupModelAnalysisInspection::validate(record, vec![event], None).is_err());
}

fn assert_claim_rejected(change: impl FnOnce(&mut GroupModelAnalysisDispatchClaim)) {
    let record = record(GroupModelAnalysisStatus::DispatchUnknown);
    let mut transcript = events(GroupModelAnalysisOutcome::Completed);
    transcript.truncate(2);
    let GroupModelAnalysisEventKind::ProviderDispatchReleased { claim } = &mut transcript[1].kind
    else {
        unreachable!()
    };
    change(claim);
    assert!(GroupModelAnalysisInspection::validate(record, transcript, None).is_err());
}

fn assert_completion_rejected(
    receipt_change: impl FnOnce(&mut GroupModelAnalysisResultReceipt),
    artifact_change: impl FnOnce(&mut GroupModelAnalysisResultArtifact),
) {
    let mut transcript = events(GroupModelAnalysisOutcome::Completed);
    let GroupModelAnalysisEventKind::AnalysisCompleted { receipt } = &mut transcript[2].kind else {
        unreachable!()
    };
    receipt_change(receipt);
    let mut artifact = artifact(GroupModelAnalysisOutcome::Completed);
    artifact_change(&mut artifact);
    assert!(
        GroupModelAnalysisInspection::validate(
            record(GroupModelAnalysisStatus::Completed),
            transcript,
            Some(artifact),
        )
        .is_err()
    );
}

fn assert_cursor_tamper_rejected(
    cursor: &GroupModelAnalysisJournalCursor,
    record: &GroupModelAnalysisRecord,
    change: impl FnOnce(&mut serde_json::Value),
) {
    let mut value = serde_json::to_value(cursor).expect("serialize cursor");
    change(&mut value);
    let restored: GroupModelAnalysisJournalCursor =
        serde_json::from_value(value).expect("tamper retains cursor shape");
    assert!(restored.validate_record(record).is_err());
}
