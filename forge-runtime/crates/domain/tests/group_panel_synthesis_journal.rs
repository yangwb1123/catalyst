use forge_runtime_domain::{
    ClaimGroupPanelSynthesisDispatch, GROUP_ANALYSIS_PANEL_VERSION,
    GROUP_PANEL_SYNTHESIS_CONSENT_VERSION, GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
    GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT, GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN,
    GROUP_PANEL_SYNTHESIS_RESULT_VERSION, GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION,
    GROUP_PANEL_SYNTHESIS_VERSION, GroupPanelSynthesisConfig, GroupPanelSynthesisDispatchAuthority,
    GroupPanelSynthesisDispatchClaim, GroupPanelSynthesisEvent, GroupPanelSynthesisEventKind,
    GroupPanelSynthesisInspection, GroupPanelSynthesisJournalCursor, GroupPanelSynthesisOutcome,
    GroupPanelSynthesisOutputTarget, GroupPanelSynthesisPreparedReceipt,
    GroupPanelSynthesisProvider, GroupPanelSynthesisRecord, GroupPanelSynthesisRecovery,
    GroupPanelSynthesisResult, GroupPanelSynthesisResultArtifact, GroupPanelSynthesisResultReceipt,
    GroupPanelSynthesisSource, GroupPanelSynthesisStatus, GroupPanelSynthesisWritebackTarget,
    MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES, Usage,
};
use sha2::{Digest, Sha256};

const BODY: &[u8] = br#"{"input":"panel"}"#;

#[test]
fn journal_prefixes_and_both_terminal_outcomes_are_exact() {
    let awaiting = inspection(1, GroupPanelSynthesisOutcome::Completed).expect("awaiting");
    assert_eq!(
        awaiting.recovery,
        GroupPanelSynthesisRecovery::AwaitingConsent
    );
    let unknown = inspection(2, GroupPanelSynthesisOutcome::Completed).expect("unknown");
    assert_eq!(
        unknown.recovery,
        GroupPanelSynthesisRecovery::DispatchUnknown {
            dispatch_id: "dispatch-1".into()
        }
    );
    for outcome in [
        GroupPanelSynthesisOutcome::Completed,
        GroupPanelSynthesisOutcome::Length,
    ] {
        let terminal = inspection(3, outcome).expect("terminal");
        assert_eq!(
            terminal.recovery,
            GroupPanelSynthesisRecovery::Terminal { outcome }
        );
        assert_eq!(
            terminal.result.as_ref().map(|value| value.result.outcome),
            Some(outcome)
        );
    }
}

#[test]
fn event_order_binding_and_terminal_state_fail_closed() {
    let all = events(GroupPanelSynthesisOutcome::Completed);
    let awaiting = record(GroupPanelSynthesisStatus::AwaitingConsent);
    let mut cursor = GroupPanelSynthesisJournalCursor::new(&awaiting).expect("cursor");
    let mut gap = all[0].clone();
    gap.seq = 2;
    assert!(cursor.append(&gap).is_err());
    assert_eq!(cursor.next_sequence(), 1);
    assert_eq!(cursor.recovery(), GroupPanelSynthesisRecovery::Unprepared);
    for event in &all {
        cursor.append(event).expect("contiguous event");
    }
    let mut extra = all[2].clone();
    extra.seq = 4;
    assert!(cursor.append(&extra).is_err());

    let mut wrong_source = all[..1].to_vec();
    let GroupPanelSynthesisEventKind::SynthesisPrepared { receipt } = &mut wrong_source[0].kind
    else {
        unreachable!()
    };
    receipt.source.panel_manifest_sha256 = digest('9');
    assert!(GroupPanelSynthesisInspection::validate(awaiting, wrong_source, None).is_err());
}

#[test]
fn dispatch_authority_rehashes_and_consumes_exact_bytes() {
    let record = record(GroupPanelSynthesisStatus::DispatchUnknown);
    let claim = claim();
    let authority =
        GroupPanelSynthesisDispatchAuthority::new(&record, claim.clone(), BODY.to_vec())
            .expect("authority");
    assert_eq!(authority.claim(), &claim);
    let (released, body) = authority.into_parts();
    assert_eq!(released, claim);
    assert_eq!(body, BODY);

    let wrong = vec![b'x'; BODY.len()];
    assert!(GroupPanelSynthesisDispatchAuthority::new(&record, released, wrong).is_err());
}

#[test]
fn validation_pins_targets_consent_source_and_result_bounds() {
    let mut config = config();
    config.writeback_target = serde_json::from_str("\"none\"").expect("none");
    config.validate().expect("fixed targets");
    config.model.clear();
    assert!(config.validate().is_err());

    let mut source = source();
    source.analysis_count = 1;
    assert!(source.validate().is_err());

    let mut request = ClaimGroupPanelSynthesisDispatch {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: "synthesis-1".into(),
        dispatch_id: "dispatch-1".into(),
        consent_version: GROUP_PANEL_SYNTHESIS_CONSENT_VERSION + 1,
        released_at_ms: 20,
    };
    assert!(request.validate().is_err());
    request.consent_version = GROUP_PANEL_SYNTHESIS_CONSENT_VERSION;
    request.validate().expect("valid consent claim");

    let mut result = artifact(GroupPanelSynthesisOutcome::Completed);
    result.result.answer = "x".repeat(MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES + 1);
    assert!(result.validate().is_err());
}

fn inspection(
    count: usize,
    outcome: GroupPanelSynthesisOutcome,
) -> Result<GroupPanelSynthesisInspection, forge_runtime_domain::GroupPanelSynthesisJournalError> {
    let status = match count {
        0 | 1 => GroupPanelSynthesisStatus::AwaitingConsent,
        2 => GroupPanelSynthesisStatus::DispatchUnknown,
        _ => GroupPanelSynthesisStatus::Completed,
    };
    let mut transcript = events(outcome);
    transcript.truncate(count);
    let result = (count == 3).then(|| artifact(outcome));
    GroupPanelSynthesisInspection::validate(record(status), transcript, result)
}

fn record(status: GroupPanelSynthesisStatus) -> GroupPanelSynthesisRecord {
    GroupPanelSynthesisRecord {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: "synthesis-1".into(),
        panel_id: "panel-1".into(),
        group_run_id: "group-run-1".into(),
        status,
        source_snapshot_sha256: digest('3'),
        panel_manifest_sha256: digest('4'),
        config: config(),
        config_sha256: digest('5'),
        request_sha256: request_digest(BODY),
        request_bytes: BODY.len(),
        protocol_version: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        created_at_ms: 10,
    }
}

fn config() -> GroupPanelSynthesisConfig {
    GroupPanelSynthesisConfig {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        provider: GroupPanelSynthesisProvider::OpenAiResponses,
        endpoint: GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT.into(),
        model: "gpt-test".into(),
        system_prompt_version: GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION,
        system_prompt_sha256: digest('1'),
        max_output_tokens: 64,
        max_model_output_bytes: 4_096,
        max_model_events: 128,
        output_target: GroupPanelSynthesisOutputTarget::LocalArtifact,
        writeback_target: GroupPanelSynthesisWritebackTarget::None,
    }
}

fn source() -> GroupPanelSynthesisSource {
    GroupPanelSynthesisSource {
        panel_version: GROUP_ANALYSIS_PANEL_VERSION,
        panel_id: "panel-1".into(),
        group_run_id: "group-run-1".into(),
        group_id: "group-1".into(),
        source_snapshot_sha256: digest('3'),
        panel_manifest_sha256: digest('4'),
        panel_manifest_bytes: 512,
        analysis_count: 2,
    }
}

fn prepared() -> GroupPanelSynthesisPreparedReceipt {
    GroupPanelSynthesisPreparedReceipt {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: "synthesis-1".into(),
        source: source(),
        config_sha256: digest('5'),
        request_sha256: request_digest(BODY),
        request_bytes: BODY.len(),
    }
}

fn claim() -> GroupPanelSynthesisDispatchClaim {
    GroupPanelSynthesisDispatchClaim {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: "synthesis-1".into(),
        dispatch_id: "dispatch-1".into(),
        request_sha256: request_digest(BODY),
        config_sha256: digest('5'),
        provider: GroupPanelSynthesisProvider::OpenAiResponses,
        endpoint: GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT.into(),
        model: "gpt-test".into(),
        consent_version: GROUP_PANEL_SYNTHESIS_CONSENT_VERSION,
        released_at_ms: 20,
    }
}

fn artifact(outcome: GroupPanelSynthesisOutcome) -> GroupPanelSynthesisResultArtifact {
    GroupPanelSynthesisResultArtifact {
        result: GroupPanelSynthesisResult {
            v: GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
            synthesis_id: "synthesis-1".into(),
            dispatch_id: "dispatch-1".into(),
            request_sha256: request_digest(BODY),
            outcome,
            answer: "moderator synthesis".into(),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 3,
            },
        },
        result_sha256: digest('6'),
        result_bytes: 128,
        created_at_ms: 30,
    }
}

fn completion(outcome: GroupPanelSynthesisOutcome) -> GroupPanelSynthesisResultReceipt {
    let artifact = artifact(outcome);
    GroupPanelSynthesisResultReceipt {
        v: GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
        synthesis_id: "synthesis-1".into(),
        dispatch_id: "dispatch-1".into(),
        request_sha256: request_digest(BODY),
        outcome,
        result_sha256: artifact.result_sha256,
        result_bytes: artifact.result_bytes,
        usage: artifact.result.usage,
        created_at_ms: artifact.created_at_ms,
    }
}

fn events(outcome: GroupPanelSynthesisOutcome) -> Vec<GroupPanelSynthesisEvent> {
    vec![
        event(
            1,
            GroupPanelSynthesisEventKind::SynthesisPrepared {
                receipt: prepared(),
            },
        ),
        event(
            2,
            GroupPanelSynthesisEventKind::ProviderDispatchReleased { claim: claim() },
        ),
        event(
            3,
            GroupPanelSynthesisEventKind::SynthesisCompleted {
                receipt: completion(outcome),
            },
        ),
    ]
}

fn event(seq: u64, kind: GroupPanelSynthesisEventKind) -> GroupPanelSynthesisEvent {
    GroupPanelSynthesisEvent {
        v: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        synthesis_id: "synthesis-1".into(),
        seq,
        kind,
    }
}

fn request_digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn digest(character: char) -> String {
    character.to_string().repeat(64)
}
