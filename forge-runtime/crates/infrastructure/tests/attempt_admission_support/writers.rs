use std::{
    sync::{Arc, Barrier},
    thread,
};

use forge_runtime_infrastructure::sqlite_execution::{AdmissionDisposition, AttemptJournalError};

use super::{counts, database, event, input, reopen, request};

#[test]
fn concurrent_exact_writers_converge_on_one_stable_admission() {
    let (_directory, path, journal) = database();
    let journals = [journal, reopen(&path), reopen(&path), reopen(&path)];
    let start = Arc::new(Barrier::new(journals.len()));
    let writers = journals
        .into_iter()
        .map(|mut journal| {
            let start = Arc::clone(&start);
            thread::spawn(move || {
                let request = request(1);
                let event = event(&request, 1);
                start.wait();
                journal.admit(&request, &event).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let results = writers
        .into_iter()
        .map(|writer| writer.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| result.disposition == AdmissionDisposition::Created)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| result.disposition == AdmissionDisposition::Replayed)
            .count(),
        3
    );
    for result in &results {
        assert_eq!(result.admission, results[0].admission);
        assert_eq!(result.admission.cursor, 1);
    }
    assert_eq!(counts(&path), (1, 1, 1));
}

#[test]
fn concurrent_distinct_writers_receive_contiguous_cursors() {
    let (_directory, path, journal) = database();
    let journals = [journal, reopen(&path), reopen(&path), reopen(&path)];
    let start = Arc::new(Barrier::new(journals.len()));
    let writers = journals
        .into_iter()
        .enumerate()
        .map(|(index, mut journal)| {
            let start = Arc::clone(&start);
            thread::spawn(move || {
                let request = request(index + 1);
                let event = event(&request, index + 1);
                start.wait();
                journal.admit(&request, &event).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let mut cursors = writers
        .into_iter()
        .map(|writer| {
            let result = writer.join().unwrap();
            assert_eq!(result.disposition, AdmissionDisposition::Created);
            result.admission.cursor
        })
        .collect::<Vec<_>>();
    cursors.sort_unstable();
    assert_eq!(cursors, [1, 2, 3, 4]);
    assert_eq!(reopen(&path).pending(0, 64).unwrap().events.len(), 4);
    assert_eq!(counts(&path), (4, 4, 4));
}

#[test]
fn concurrent_same_key_different_requests_have_one_winner_and_one_conflict() {
    let (_directory, path, journal) = database();
    let journals = [journal, reopen(&path)];
    let start = Arc::new(Barrier::new(2));
    let writers = journals
        .into_iter()
        .enumerate()
        .map(|(index, mut journal)| {
            let start = Arc::clone(&start);
            thread::spawn(move || {
                let mut supplied = input(index + 1);
                supplied.idempotency_key = "shared-idempotency-key".into();
                let request =
                    forge_runtime_domain::execution::attempt::AttemptRequest::try_from_input(
                        &supplied,
                    )
                    .unwrap();
                let event = event(&request, index + 1);
                start.wait();
                journal.admit(&request, &event)
            })
        })
        .collect::<Vec<_>>();
    let results = writers
        .into_iter()
        .map(|writer| writer.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(AttemptJournalError::Conflict))
            .count(),
        1
    );
    assert_eq!(counts(&path), (1, 1, 1));
    assert_eq!(reopen(&path).pending(0, 64).unwrap().head_cursor, 1);
}
