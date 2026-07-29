use forge_runtime_domain::{
    GROUP_CONTEXT_VERSION, GROUP_EXECUTION_PROTOCOL_VERSION, GROUP_EXECUTION_VERSION,
    GroupContextStats, GroupExecutionEvent, GroupExecutionEventKind, GroupExecutionInspection,
    GroupExecutionJournalCursor, GroupExecutionMode, GroupExecutionOutcome, GroupExecutionReceipt,
    GroupExecutionRecord, GroupExecutionRecovery, GroupExecutionStatus,
};

#[test]
fn exact_three_event_transcript_reaches_validated_terminal() {
    let inspection =
        GroupExecutionInspection::validate(record(GroupExecutionStatus::Completed), events())
            .expect("valid journal");

    assert_eq!(
        inspection.recovery,
        GroupExecutionRecovery::Terminal {
            outcome: GroupExecutionOutcome::SnapshotValidated
        }
    );
    assert_eq!(inspection.receipt, Some(receipt()));
}

#[test]
fn every_valid_prefix_is_incomplete_and_exposes_receipt_only_after_verification() {
    for length in 0..3 {
        let inspection = GroupExecutionInspection::validate(
            record(GroupExecutionStatus::Incomplete),
            events()[..length].to_vec(),
        )
        .expect("valid prefix");

        assert_eq!(
            inspection.recovery,
            GroupExecutionRecovery::Incomplete,
            "prefix length {length}"
        );
        assert_eq!(
            inspection.receipt,
            (length >= 2).then(receipt),
            "prefix length {length}"
        );
    }
}

#[test]
fn status_must_agree_with_the_derived_journal_state() {
    let incomplete_error =
        GroupExecutionInspection::validate(record(GroupExecutionStatus::Completed), Vec::new())
            .expect_err("completed metadata cannot have an empty journal");
    let completed_error =
        GroupExecutionInspection::validate(record(GroupExecutionStatus::Incomplete), events())
            .expect_err("terminal journal cannot remain incomplete");

    assert!(incomplete_error.message.contains("status"));
    assert!(completed_error.message.contains("status"));
}

#[test]
fn transitions_are_strict_and_terminal_is_final() {
    let mut skipped = events();
    skipped.remove(1);
    let skip_error =
        GroupExecutionInspection::validate(record(GroupExecutionStatus::Incomplete), skipped)
            .expect_err("verification cannot be skipped");
    assert!(
        skip_error.message.contains("transition") || skip_error.message.contains("sequence"),
        "{}",
        skip_error.message
    );

    let mut after_terminal = events();
    let mut extra = after_terminal[2].clone();
    extra.seq = 4;
    after_terminal.push(extra);
    let terminal_error =
        GroupExecutionInspection::validate(record(GroupExecutionStatus::Completed), after_terminal)
            .expect_err("terminal cannot accept another event");
    assert!(terminal_error.message.contains("terminal"));
}

#[test]
fn source_and_receipt_bindings_fail_closed() {
    let mut bad_start = events();
    let GroupExecutionEventKind::ExecutionStarted {
        snapshot_sha256, ..
    } = &mut bad_start[0].kind
    else {
        unreachable!()
    };
    *snapshot_sha256 = "33".repeat(32);
    assert!(
        GroupExecutionInspection::validate(
            record(GroupExecutionStatus::Incomplete),
            bad_start[..1].to_vec()
        )
        .expect_err("source mismatch")
        .message
        .contains("frozen source")
    );

    let mut bad_receipt = events();
    let GroupExecutionEventKind::SnapshotVerified { receipt } = &mut bad_receipt[1].kind else {
        unreachable!()
    };
    receipt.group_run_id = "another-run".into();
    assert!(
        GroupExecutionInspection::validate(
            record(GroupExecutionStatus::Incomplete),
            bad_receipt[..2].to_vec()
        )
        .expect_err("receipt mismatch")
        .message
        .contains("receipt")
    );
}

#[test]
fn malformed_receipt_metadata_is_rejected() {
    let mut malformed = events();
    let GroupExecutionEventKind::SnapshotVerified { receipt } = &mut malformed[1].kind else {
        unreachable!()
    };
    receipt.context_slice_sha256 = "UPPER".into();

    let error = GroupExecutionInspection::validate(
        record(GroupExecutionStatus::Incomplete),
        malformed[..2].to_vec(),
    )
    .expect_err("digest shape is validated");
    assert!(error.message.contains("receipt"));
}

#[test]
fn serialized_cursor_resumes_at_the_exact_next_event() {
    let mut cursor =
        GroupExecutionJournalCursor::new(&record(GroupExecutionStatus::Incomplete)).expect("new");
    cursor.append(&events()[0]).expect("start");
    let encoded = serde_json::to_string(&cursor).expect("encode cursor");
    let mut restored: GroupExecutionJournalCursor =
        serde_json::from_str(&encoded).expect("decode cursor");

    restored
        .validate_record(&record(GroupExecutionStatus::Incomplete))
        .expect("record binding");
    assert_eq!(restored.next_sequence(), 2);
    restored.append(&events()[1]).expect("verification");
    assert_eq!(restored.receipt(), Some(&receipt()));
}

#[test]
fn restored_cursor_rejects_state_sequence_receipt_and_status_tampering() {
    let incomplete = record(GroupExecutionStatus::Incomplete);
    let cursor = GroupExecutionJournalCursor::new(&incomplete).expect("new cursor");
    let mut skipped = serde_json::to_value(&cursor).expect("cursor value");
    skipped["next_sequence"] = serde_json::json!(2);
    let skipped: GroupExecutionJournalCursor =
        serde_json::from_value(skipped).expect("shape remains decodable");
    assert!(skipped.validate_record(&incomplete).is_err());

    let mut unbound = serde_json::to_value(&cursor).expect("cursor value");
    unbound["execution_id"] = serde_json::json!("");
    let mut unbound: GroupExecutionJournalCursor =
        serde_json::from_value(unbound).expect("shape remains decodable");
    let mut unbound_event = events()[0].clone();
    unbound_event.execution_id.clear();
    assert!(unbound.append(&unbound_event).is_err());

    let mut verified = GroupExecutionJournalCursor::new(&incomplete).expect("new cursor");
    for event in &events()[..2] {
        verified.append(event).expect("valid prefix");
    }
    let mut rebound = serde_json::to_value(&verified).expect("cursor value");
    rebound["state"]["NeedFinish"]["group_run_id"] = serde_json::json!("other-run");
    let rebound: GroupExecutionJournalCursor =
        serde_json::from_value(rebound).expect("shape remains decodable");
    assert!(rebound.validate_record(&incomplete).is_err());

    let completed = record(GroupExecutionStatus::Completed);
    assert!(verified.validate_record(&completed).is_err());
}

#[test]
fn envelope_identity_version_and_sequence_are_enforced() {
    for mutate in [
        |event: &mut GroupExecutionEvent| event.v += 1,
        |event: &mut GroupExecutionEvent| event.execution_id = "other".into(),
        |event: &mut GroupExecutionEvent| event.seq += 1,
    ] {
        let mut first = events()[0].clone();
        mutate(&mut first);
        let error = GroupExecutionInspection::validate(
            record(GroupExecutionStatus::Incomplete),
            vec![first],
        )
        .expect_err("invalid envelope");
        assert!(
            error.message.contains("envelope") || error.message.contains("sequence"),
            "{}",
            error.message
        );
    }
}

fn record(status: GroupExecutionStatus) -> GroupExecutionRecord {
    GroupExecutionRecord {
        v: GROUP_EXECUTION_VERSION,
        execution_id: "group-execution-1".into(),
        group_run_id: "group-run-1".into(),
        mode: GroupExecutionMode::OfflineSnapshotValidation,
        status,
        source_snapshot_sha256: "22".repeat(32),
        protocol_version: GROUP_EXECUTION_PROTOCOL_VERSION,
        created_at_ms: 10,
    }
}

fn receipt() -> GroupExecutionReceipt {
    GroupExecutionReceipt {
        v: GROUP_EXECUTION_VERSION,
        execution_id: "group-execution-1".into(),
        group_run_id: "group-run-1".into(),
        group_id: "group-1".into(),
        context_version: GROUP_CONTEXT_VERSION,
        context_slice_sha256: "11".repeat(32),
        snapshot_sha256: "22".repeat(32),
        snapshot_bytes: 100,
        stats: GroupContextStats {
            member_count: 2,
            conversation_count: 3,
            prompt_count: 4,
            content_bytes: 12,
            truncated_prompt_count: 1,
            ..GroupContextStats::default()
        },
    }
}

fn events() -> Vec<GroupExecutionEvent> {
    vec![
        event(
            1,
            GroupExecutionEventKind::ExecutionStarted {
                group_run_id: "group-run-1".into(),
                snapshot_sha256: "22".repeat(32),
            },
        ),
        event(
            2,
            GroupExecutionEventKind::SnapshotVerified { receipt: receipt() },
        ),
        event(
            3,
            GroupExecutionEventKind::ExecutionFinished {
                outcome: GroupExecutionOutcome::SnapshotValidated,
            },
        ),
    ]
}

fn event(seq: u64, kind: GroupExecutionEventKind) -> GroupExecutionEvent {
    GroupExecutionEvent {
        v: GROUP_EXECUTION_PROTOCOL_VERSION,
        execution_id: "group-execution-1".into(),
        seq,
        kind,
    }
}
