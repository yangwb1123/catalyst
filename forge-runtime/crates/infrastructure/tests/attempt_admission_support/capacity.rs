use std::path::Path;

use forge_runtime_domain::{
    execution::attempt::AttemptRequest,
    platform_core_contract::{canonical_event_envelope_json, event_envelope_sha256},
};
use forge_runtime_infrastructure::sqlite_execution::{
    AdmissionDisposition, AttemptJournalError, request_sha256,
};
use rusqlite::{Connection, params};

use super::{counts, database, event, input, platform_id, reopen, request};

#[test]
fn full_journal_replays_before_capacity_rejection_and_keeps_page_limits() {
    let (_directory, path, mut journal) = database();
    let first = request(1);
    let original = journal.admit(&first, &event(&first, 1)).unwrap().admission;
    drop(journal);
    seed_remaining_admissions(&path);
    let mut journal = reopen(&path);
    let replay = journal.admit(&first, &event(&first, 1)).unwrap();
    assert_eq!(replay.disposition, AdmissionDisposition::Replayed);
    assert_eq!(replay.admission, original);
    let last = request(1024);
    assert_eq!(
        journal
            .admit(&last, &event(&last, 1024))
            .unwrap()
            .disposition,
        AdmissionDisposition::Replayed
    );
    let excess = request(1025);
    assert_eq!(
        journal.admit(&excess, &event(&excess, 1025)),
        Err(AttemptJournalError::Capacity)
    );
    assert_full_journal_queries(&mut journal);
    assert_eq!(counts(&path), (1024, 1024, 1024));
}

fn assert_full_journal_queries(
    journal: &mut forge_runtime_infrastructure::sqlite_execution::SqliteAttemptJournal,
) {
    let page = journal.pending(0, 64).unwrap();
    assert_eq!(
        (page.head_cursor, page.next_cursor, page.has_more),
        (1024, 64, true)
    );
    assert_eq!(page.events.len(), 64);
    assert!(
        page.events
            .iter()
            .map(|event| event.canonical_event_json.len())
            .sum::<usize>()
            <= 256 * 1024
    );
    let last = journal.pending(1023, 64).unwrap();
    assert_eq!(
        (last.head_cursor, last.next_cursor, last.has_more),
        (1024, 1024, false)
    );
    assert_eq!(last.events.len(), 1);
    assert_eq!(
        last.events[0].event.aggregate_ref.entity_id,
        platform_id("atm", 1024)
    );
    let mut changed = input(1);
    changed.timeout_ms += 1;
    let changed = AttemptRequest::try_from_input(&changed).unwrap();
    assert_eq!(
        journal.admit(&changed, &event(&changed, 1)),
        Err(AttemptJournalError::Conflict)
    );
    assert!(journal.get(&platform_id("atm", 1025)).unwrap().is_none());
}

fn seed_remaining_admissions(path: &Path) {
    let mut connection = Connection::open(path).unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    let template: String = connection
        .query_row(
            "SELECT request_json FROM attempt_admissions WHERE cursor=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 2..=1024 {
        seed_row(&transaction, &template, index);
    }
    transaction.commit().unwrap();
}

fn seed_row(connection: &Connection, template: &str, index: usize) {
    let request = request(index);
    let event = event(&request, index);
    let cursor = i64::try_from(index).unwrap();
    let request_json = template
        .replace(&platform_id("atm", 1), &platform_id("atm", index))
        .replace("attempt-request-key-0001", request.idempotency_key());
    connection.execute(
        "INSERT INTO attempt_admissions(cursor,attempt_id,idempotency_key,request_json,request_sha256) \
         VALUES (?1,?2,?3,?4,?5)",
        params![cursor, request.attempt_ref().entity_id, request.idempotency_key(), request_json,
            request_sha256(&request).unwrap()],
    ).unwrap();
    connection
        .execute(
            "INSERT INTO attempt_events(cursor,event_id,message_id,event_json,event_sha256) \
         VALUES (?1,?2,?3,?4,?5)",
            params![
                cursor,
                event.event_id,
                event.message_id,
                canonical_event_envelope_json(&event).unwrap(),
                event_envelope_sha256(&event).unwrap()
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO attempt_outbox(cursor,event_id) VALUES (?1,?2)",
            params![cursor, event.event_id],
        )
        .unwrap();
}
