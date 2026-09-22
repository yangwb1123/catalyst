use super::{EXECUTION_LEASE_ABI_VERSION, LeaseError, LeaseGrant, LeaseState, TerminalDisposition};

const TTL: u64 = 10_000;

fn grant() -> LeaseGrant {
    LeaseGrant::issue(
        "attempt-1".into(),
        "runner-1".into(),
        1,
        "fence-1".into(),
        100,
        TTL,
    )
    .expect("valid grant")
}

fn completed() -> TerminalDisposition {
    TerminalDisposition::Completed {
        receipt_sha256: "a".repeat(64),
    }
}

fn lease_state() -> LeaseState {
    LeaseState::new(grant()).expect("valid state")
}

#[test]
fn grant_binds_identity_and_active_window() {
    let lease = grant();
    assert_eq!(lease.v, EXECUTION_LEASE_ABI_VERSION);
    assert!(lease.is_active(100));
    assert!(lease.is_active(10_099));
    assert!(!lease.is_active(10_100));
    lease
        .validate_proof(&lease.proof(), 10_099)
        .expect("current proof is accepted");
}

#[test]
fn stale_epoch_and_token_cannot_submit_after_renewal() {
    let mut state = lease_state();
    let old_proof = state.grant().proof();
    state
        .renew(1_000, "fence-2".into(), TTL)
        .expect("renew before expiry");
    assert_eq!(state.grant().epoch, 2);
    assert_eq!(state.grant().fencing_token, "fence-2");
    assert_eq!(
        state.submit_terminal(old_proof, completed(), 2_000),
        Err(LeaseError::EpochMismatch)
    );
}

#[test]
fn foreign_attempt_target_and_token_are_rejected() {
    let lease = grant();
    let mut proof = lease.proof();
    proof.attempt_id = "foreign-attempt".into();
    assert_eq!(
        lease.validate_proof(&proof, 200),
        Err(LeaseError::AttemptMismatch)
    );
    proof = lease.proof();
    proof.target_id = "foreign-target".into();
    assert_eq!(
        lease.validate_proof(&proof, 200),
        Err(LeaseError::TargetMismatch)
    );
    proof = lease.proof();
    proof.fencing_token = "foreign-token".into();
    assert_eq!(
        lease.validate_proof(&proof, 200),
        Err(LeaseError::FencingTokenMismatch)
    );
}

#[test]
fn expired_lease_rejects_renewal_and_terminal_submission() {
    let mut state = lease_state();
    assert_eq!(
        state.renew(10_100, "fence-2".into(), TTL),
        Err(LeaseError::LeaseExpired)
    );
    assert_eq!(
        state.submit_terminal(state.grant().proof(), completed(), 10_100),
        Err(LeaseError::LeaseExpired)
    );
}

#[test]
fn terminal_receipt_replays_exactly_and_conflicts_on_changed_outcome() {
    let mut state = lease_state();
    let first = state
        .submit_terminal(state.grant().proof(), completed(), 200)
        .expect("first terminal receipt");
    assert!(!first.replayed);
    let replay = state
        .submit_terminal(state.grant().proof(), completed(), 20_000)
        .expect("exact terminal replay");
    assert!(replay.replayed);
    assert_eq!(replay.receipt, first.receipt);
    assert_eq!(
        state.submit_terminal(
            state.grant().proof(),
            TerminalDisposition::Failed {
                reason: "late".into()
            },
            20_000,
        ),
        Err(LeaseError::TerminalAlreadyRecorded)
    );
}

#[test]
fn uncertain_outcome_is_terminal_and_never_implies_retry() {
    let mut state = lease_state();
    let result = state
        .submit_terminal(
            state.grant().proof(),
            TerminalDisposition::Uncertain {
                reason: "transport ended after effect boundary".into(),
            },
            200,
        )
        .expect("uncertain receipt");
    assert!(result.receipt.disposition.is_uncertain());
    assert!(state.terminal().is_some());
    assert_eq!(
        state.renew(201, "fence-2".into(), TTL),
        Err(LeaseError::TerminalAlreadyRecorded)
    );
}

#[test]
fn bounds_and_monotonic_time_fail_closed() {
    assert_eq!(
        LeaseGrant::issue("attempt".into(), "target".into(), 0, "token".into(), 1, TTL,),
        Err(LeaseError::InvalidEpoch)
    );
    assert_eq!(
        LeaseGrant::issue(
            "attempt".into(),
            "target".into(),
            1,
            "token".into(),
            u64::MAX,
            TTL,
        ),
        Err(LeaseError::TimeOverflow)
    );
    let lease = grant();
    assert_eq!(
        lease.renew(99, "fence-2".into(), TTL),
        Err(LeaseError::TimeWentBackwards)
    );
}

#[test]
fn terminal_disposition_rejects_fields_from_other_variants() {
    let digest = "a".repeat(64);
    let completed_with_reason =
        format!(r#"{{"kind":"completed","receipt_sha256":"{digest}","reason":"late"}}"#);
    assert!(serde_json::from_str::<TerminalDisposition>(&completed_with_reason).is_err());
    assert!(serde_json::from_str::<TerminalDisposition>(
        r#"{"kind":"failed","reason":"late","receipt_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#
    )
    .is_err());
}

#[test]
fn lease_error_display_is_the_shared_machine_code() {
    assert_eq!(LeaseError::EpochMismatch.to_string(), "epoch_mismatch");
    assert_eq!(
        LeaseError::TerminalAlreadyRecorded.code(),
        "terminal_already_recorded"
    );
}
