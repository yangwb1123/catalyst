mod attempt_admission_support;

use attempt_admission_support::{counts, database, event, input, platform_id, reopen, request};
use forge_runtime_domain::{
    execution::attempt::AttemptRequest,
    platform_core_contract::{AttemptState, canonical_event_envelope_json, event_envelope_sha256},
};
use forge_runtime_infrastructure::sqlite_execution::{
    AdmissionDisposition, AttemptJournalError, PendingPage, request_sha256,
};

#[test]
fn admission_reopens_with_all_bindings_and_retained_pending_event() {
    let (_directory, path, mut journal) = database();
    let request = request(1);
    let event = event(&request, 1);
    let result = journal.admit(&request, &event).expect("create admission");
    assert_eq!(result.disposition, AdmissionDisposition::Created);
    assert_eq!(result.admission.cursor, 1);
    assert_eq!(result.admission.request, request);
    assert_eq!(result.admission.event, event);
    assert_eq!(
        result.admission.request.initial_state(),
        AttemptState::Requested
    );
    assert_eq!(
        result.admission.request_sha256,
        request_sha256(&request).unwrap()
    );
    assert_eq!(
        result.admission.event_sha256,
        event_envelope_sha256(&event).unwrap()
    );
    assert_eq!(
        result.admission.canonical_event_json,
        canonical_event_envelope_json(&event).unwrap()
    );
    drop(journal);
    let mut reopened = reopen(&path);
    let restored = reopened.get(&platform_id("atm", 1)).unwrap().unwrap();
    assert_eq!(restored, result.admission);
    let pending = reopened.pending(0, 64).unwrap();
    assert_eq!(
        (pending.head_cursor, pending.next_cursor, pending.has_more),
        (1, 1, false)
    );
    assert_eq!(pending.events.len(), 1);
    assert_eq!(pending.events[0].cursor, restored.cursor);
    assert_eq!(pending.events[0].event, restored.event);
    assert_eq!(pending.events[0].event_sha256, restored.event_sha256);
    assert_eq!(
        pending.events[0].canonical_event_json,
        restored.canonical_event_json
    );
    assert_eq!(reopened.pending(0, 64).unwrap(), pending);
    assert_eq!(counts(&path), (1, 1, 1));
}

#[test]
fn exact_replay_returns_original_result_before_and_after_reopen() {
    let (_directory, path, mut journal) = database();
    let request = request(1);
    let event = event(&request, 1);
    let created = journal.admit(&request, &event).unwrap();
    for _ in 0..3 {
        let replay = journal.admit(&request, &event).unwrap();
        assert_eq!(replay.disposition, AdmissionDisposition::Replayed);
        assert_eq!(replay.admission, created.admission);
    }
    drop(journal);
    let replay = reopen(&path).admit(&request, &event).unwrap();
    assert_eq!(replay.disposition, AdmissionDisposition::Replayed);
    assert_eq!(replay.admission, created.admission);
    assert_eq!(counts(&path), (1, 1, 1));
}

#[test]
fn normalized_set_order_replays_and_returned_values_are_owned() {
    let (_directory, path, mut journal) = database();
    let mut supplied = input(1);
    let request = AttemptRequest::try_from_input(&supplied).unwrap();
    let event = event(&request, 1);
    let created = journal.admit(&request, &event).unwrap();
    supplied.approval_refs.reverse();
    supplied.requested_effects.reverse();
    let normalized = AttemptRequest::try_from_input(&supplied).unwrap();
    assert_eq!(
        request_sha256(&request).unwrap(),
        request_sha256(&normalized).unwrap()
    );
    let replay = journal.admit(&normalized, &event).unwrap();
    assert_eq!(replay.disposition, AdmissionDisposition::Replayed);
    let mut returned = replay.admission;
    returned.request_sha256.clear();
    returned.event.actor_ref.actor_id = platform_id("acr", 2);
    returned.canonical_event_json.clear();
    let mut pending = journal.pending(0, 64).unwrap();
    pending.events[0].event_sha256.clear();
    assert_eq!(
        journal.get(&platform_id("atm", 1)).unwrap(),
        Some(created.admission)
    );
    assert_eq!(counts(&path), (1, 1, 1));
}

#[test]
fn same_key_changed_valid_request_or_event_metadata_conflicts() {
    let (_directory, path, mut journal) = database();
    let request = request(1);
    let original = event(&request, 1);
    let stored = journal.admit(&request, &original).unwrap().admission;
    let mut changed_input = input(1);
    changed_input.timeout_ms += 1;
    let changed = AttemptRequest::try_from_input(&changed_input).unwrap();
    assert_eq!(
        journal.admit(&changed, &event(&changed, 1)),
        Err(AttemptJournalError::Conflict)
    );
    for field in 0..5 {
        let mut changed = original.clone();
        match field {
            0 => changed.actor_ref.actor_id = platform_id("acr", 2),
            1 => changed.correlation_id = platform_id("cor", 2),
            2 => changed.occurred_at_unix_ms += 1,
            3 => {
                changed.event_id = platform_id("evt", 2);
                changed.message_id = platform_id("msg", 2);
            }
            _ => {
                changed.actor_ref.actor_type =
                    forge_runtime_domain::platform_core_contract::ActorType::Human;
            }
        }
        assert_eq!(
            journal.admit(&request, &changed),
            Err(AttemptJournalError::Conflict)
        );
    }
    assert_eq!(journal.get(&platform_id("atm", 1)).unwrap(), Some(stored));
    assert_eq!(counts(&path), (1, 1, 1));
}

#[test]
fn attempt_and_idempotency_identity_are_database_global() {
    let (_directory, path, mut journal) = database();
    let original = request(1);
    journal.admit(&original, &event(&original, 1)).unwrap();
    let mut same_attempt = input(1);
    same_attempt.idempotency_key = "new-attempt-request-key".into();
    let same_attempt = AttemptRequest::try_from_input(&same_attempt).unwrap();
    assert_eq!(
        journal.admit(&same_attempt, &event(&same_attempt, 2)),
        Err(AttemptJournalError::Conflict)
    );
    let mut same_key = input(2);
    same_key.idempotency_key = original.idempotency_key().into();
    let same_key = AttemptRequest::try_from_input(&same_key).unwrap();
    assert_eq!(
        journal.admit(&same_key, &event(&same_key, 2)),
        Err(AttemptJournalError::Conflict)
    );
    let independent = request(2);
    assert_eq!(
        journal
            .admit(&independent, &event(&independent, 2))
            .unwrap()
            .admission
            .cursor,
        2
    );
    assert_eq!(counts(&path), (2, 2, 2));
}

#[test]
fn event_and_message_identities_cannot_be_reused_by_another_attempt() {
    let (_directory, path, mut journal) = database();
    let original = request(1);
    journal.admit(&original, &event(&original, 1)).unwrap();
    let other = request(2);
    assert_eq!(
        journal.admit(&other, &event(&other, 1)),
        Err(AttemptJournalError::Conflict)
    );
    let mut mismatched = event(&other, 2);
    mismatched.message_id = platform_id("msg", 1);
    assert_eq!(
        journal.admit(&other, &mismatched),
        Err(AttemptJournalError::Invalid)
    );
    mismatched = event(&other, 2);
    mismatched.event_id = platform_id("evt", 1);
    assert_eq!(
        journal.admit(&other, &mismatched),
        Err(AttemptJournalError::Invalid)
    );
    assert_eq!(counts(&path), (1, 1, 1));
}

#[test]
fn pending_pages_follow_admission_order_and_return_a_snapshot_head() {
    let (_directory, _path, mut journal) = database();
    for index in [3, 1, 2] {
        let request = request(index);
        journal.admit(&request, &event(&request, index)).unwrap();
    }
    let first = journal.pending(0, 2).unwrap();
    assert_eq!(
        (first.head_cursor, first.next_cursor, first.has_more),
        (3, 2, true)
    );
    assert_eq!(cursors(&first), [1, 2]);
    assert_eq!(
        first.events[0].event.aggregate_ref.entity_id,
        platform_id("atm", 3)
    );
    assert_eq!(
        first.events[1].event.aggregate_ref.entity_id,
        platform_id("atm", 1)
    );
    let request = request(4);
    journal.admit(&request, &event(&request, 4)).unwrap();
    let next = journal.pending(first.next_cursor, 2).unwrap();
    assert_eq!(
        (next.head_cursor, next.next_cursor, next.has_more),
        (4, 4, false)
    );
    assert_eq!(cursors(&next), [3, 4]);
    let finished = journal.pending(next.next_cursor, 64).unwrap();
    assert_eq!(
        (
            finished.head_cursor,
            finished.next_cursor,
            finished.has_more
        ),
        (4, 4, false)
    );
    assert!(finished.events.is_empty());
}

fn cursors(page: &PendingPage) -> Vec<u64> {
    page.events.iter().map(|item| item.cursor).collect()
}

#[test]
fn empty_and_invalid_queries_are_bounded_without_writing() {
    let (_directory, path, mut journal) = database();
    assert!(journal.get(&platform_id("atm", 1)).unwrap().is_none());
    let empty = journal.pending(0, 1).unwrap();
    assert_eq!(
        (empty.head_cursor, empty.next_cursor, empty.has_more),
        (0, 0, false)
    );
    assert!(empty.events.is_empty());
    for (cursor, limit) in [(0, 0), (0, 65), (0, usize::MAX), (1, 1), (u64::MAX, 1)] {
        assert_eq!(
            journal.pending(cursor, limit),
            Err(AttemptJournalError::Invalid)
        );
    }
    for attempt_id in [
        String::new(),
        "atm_bad".into(),
        platform_id("wki", 1),
        "a".repeat(65_537),
    ] {
        assert_eq!(journal.get(&attempt_id), Err(AttemptJournalError::Invalid));
    }
    assert_eq!(counts(&path), (0, 0, 0));
}
