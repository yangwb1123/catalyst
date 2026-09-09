use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use rusqlite::Connection;

use super::{
    AdmissionDisposition, AttemptJournalError, SqliteAttemptJournal, crash_fixture, event, write,
};

const DATABASE: &str = "FORGE_TEST_ADMISSION_CRASH_DATABASE";
const PHASE: &str = "FORGE_TEST_ADMISSION_CRASH_PHASE";
const CHILD_TEST: &str = "sqlite_execution::crash_tests::admission_crash_child";

#[test]
fn precommit_error_rolls_back_without_consuming_the_key() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("error.db");
    let mut journal = open(&path);
    let request = crash_fixture::request();
    let envelope = crash_fixture::event(&request);
    let candidate = event::prepare(&request, &envelope).unwrap();
    assert_eq!(
        write::admit_with_before_commit(&mut journal.connection, candidate, |_| Err(
            AttemptJournalError::Unavailable
        )),
        Err(AttemptJournalError::Unavailable)
    );
    assert_eq!(journal.pending(0, 64).unwrap().head_cursor, 0);
    assert_eq!(
        journal.admit(&request, &envelope).unwrap().disposition,
        AdmissionDisposition::Created
    );
}

#[test]
fn commit_constraint_failure_rolls_back_the_whole_admission() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("commit-error.db");
    let mut journal = open(&path);
    let request = crash_fixture::request();
    let envelope = crash_fixture::event(&request);
    let candidate = event::prepare(&request, &envelope).unwrap();
    let result = write::admit_with_before_commit(&mut journal.connection, candidate, |tx| {
        tx.execute_batch("PRAGMA defer_foreign_keys=ON; DELETE FROM attempt_events;")
            .unwrap();
        Ok(())
    });
    assert_eq!(result, Err(AttemptJournalError::Unavailable));
    drop(journal);
    let mut reopened = open(&path);
    assert_eq!(reopened.pending(0, 64).unwrap().head_cursor, 0);
    assert_eq!(
        reopened.admit(&request, &envelope).unwrap().disposition,
        AdmissionDisposition::Created
    );
}

#[test]
fn kill_after_all_inserts_before_commit_rolls_back_every_row() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("precommit.db");
    drop(open(&path));
    kill_at_boundary(&path, "precommit");
    let request = crash_fixture::request();
    let event = crash_fixture::event(&request);
    let mut reopened = open(&path);
    assert!(
        reopened
            .get(&request.attempt_ref().entity_id)
            .unwrap()
            .is_none()
    );
    assert_eq!(reopened.pending(0, 64).unwrap().head_cursor, 0);
    let admitted = reopened.admit(&request, &event).unwrap();
    assert_eq!(admitted.disposition, AdmissionDisposition::Created);
    assert_eq!(admitted.admission.cursor, 1);
    assert_eq!(reopened.pending(0, 64).unwrap().events.len(), 1);
}

#[test]
fn kill_after_commit_before_reply_preserves_exact_replay() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("postcommit.db");
    drop(open(&path));
    kill_at_boundary(&path, "postcommit");
    let request = crash_fixture::request();
    let event = crash_fixture::event(&request);
    let mut reopened = open(&path);
    let retained = reopened
        .get(&request.attempt_ref().entity_id)
        .unwrap()
        .unwrap();
    let replay = reopened.admit(&request, &event).unwrap();
    assert_eq!(replay.disposition, AdmissionDisposition::Replayed);
    assert_eq!(replay.admission, retained);
    assert_eq!(replay.admission.event, event);
    let page = reopened.pending(0, 64).unwrap();
    assert_eq!(page.head_cursor, 1);
    assert_eq!(page.events.len(), 1);
    assert_eq!(
        page.events[0].canonical_event_json,
        retained.canonical_event_json
    );
}

// This test is the explicitly selected subprocess fixture, not a production hook.
#[test]
fn admission_crash_child() {
    let Some(path) = std::env::var_os(DATABASE) else {
        return;
    };
    let phase = std::env::var(PHASE).unwrap();
    let mut journal = open(Path::new(&path));
    let request = crash_fixture::request();
    let event = crash_fixture::event(&request);
    if phase == "precommit" {
        let candidate = event::prepare(&request, &event).unwrap();
        write::admit_with_before_commit(&mut journal.connection, candidate, |transaction| {
            for table in ["attempt_admissions", "attempt_events", "attempt_outbox"] {
                let count: i64 = transaction
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .unwrap();
                assert_eq!(count, 1);
            }
            signal_and_block();
            Ok(())
        })
        .unwrap();
    } else {
        assert_eq!(phase, "postcommit");
        assert_eq!(
            journal.admit(&request, &event).unwrap().disposition,
            AdmissionDisposition::Created
        );
        signal_and_block();
    }
}

fn open(path: &Path) -> SqliteAttemptJournal {
    SqliteAttemptJournal::from_connection(Connection::open(path).unwrap()).unwrap()
}

fn signal_and_block() {
    println!("ADMISSION_CRASH_BOUNDARY");
    std::io::stdout().flush().unwrap();
    let mut byte = [0_u8];
    std::io::stdin().read_exact(&mut byte).unwrap();
    panic!("parent must kill the child at the declared boundary");
}

fn kill_at_boundary(path: &Path, phase: &str) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD_TEST, "--nocapture"])
        .env(DATABASE, path)
        .env(PHASE, phase)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_boundary(&mut child);
    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
}

fn wait_for_boundary(child: &mut Child) {
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap().contains("ADMISSION_CRASH_BOUNDARY") {
                let _ = sender.send(());
                break;
            }
        }
    });
    let ready = receiver.recv_timeout(Duration::from_secs(30));
    if ready.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    reader.join().unwrap();
    assert!(
        ready.is_ok(),
        "child did not reach the selected commit boundary"
    );
}
