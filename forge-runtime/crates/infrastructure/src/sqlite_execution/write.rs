use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use super::{
    AdmissionDisposition, AdmissionResult, AttemptAdmission, AttemptJournalError, MAX_ADMISSIONS,
    MAX_TOTAL_BYTES, event::Candidate, read, sqlite_error,
};

pub(super) fn admit(
    connection: &mut Connection,
    candidate: Candidate,
) -> Result<AdmissionResult, AttemptJournalError> {
    admit_with_before_commit(connection, candidate, |_| Ok(()))
}

pub(super) fn admit_with_before_commit(
    connection: &mut Connection,
    candidate: Candidate,
    before_commit: impl FnOnce(&Transaction<'_>) -> Result<(), AttemptJournalError>,
) -> Result<AdmissionResult, AttemptJournalError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error)?;
    let snapshot = read::audit(&transaction)?;
    if let Some(existing) = snapshot.admissions.iter().find(|entry| {
        entry.request.idempotency_key() == candidate.admission.request.idempotency_key()
    }) {
        let result = replay(existing, &candidate)?;
        drop(candidate);
        before_commit(&transaction)?;
        transaction.commit().map_err(sqlite_error)?;
        return Ok(result);
    }
    reject_collisions(&snapshot.admissions, &candidate.admission)?;
    let added_bytes = candidate.request_json.len() + candidate.admission.canonical_event_json.len();
    if snapshot.admissions.len() == MAX_ADMISSIONS
        || snapshot.total_bytes + added_bytes > MAX_TOTAL_BYTES
    {
        return Err(AttemptJournalError::Capacity);
    }
    let cursor =
        i64::try_from(snapshot.admissions.len() + 1).map_err(|_| AttemptJournalError::Capacity)?;
    insert(&transaction, &candidate, cursor)?;
    drop(candidate);
    let committed = read::audit(&transaction)?;
    let admission = committed
        .admissions
        .into_iter()
        .last()
        .ok_or(AttemptJournalError::Corrupt)?;
    before_commit(&transaction)?;
    transaction.commit().map_err(sqlite_error)?;
    Ok(AdmissionResult {
        disposition: AdmissionDisposition::Created,
        admission,
    })
}

fn replay(
    existing: &AttemptAdmission,
    candidate: &Candidate,
) -> Result<AdmissionResult, AttemptJournalError> {
    let stored =
        super::codec::encode(&existing.request).map_err(|_| AttemptJournalError::Corrupt)?;
    if stored.json != candidate.request_json
        || existing.canonical_event_json != candidate.admission.canonical_event_json
    {
        return Err(AttemptJournalError::Conflict);
    }
    Ok(AdmissionResult {
        disposition: AdmissionDisposition::Replayed,
        admission: existing.clone(),
    })
}

fn reject_collisions(
    admissions: &[AttemptAdmission],
    candidate: &AttemptAdmission,
) -> Result<(), AttemptJournalError> {
    if admissions.iter().any(|entry| {
        entry.request.attempt_ref() == candidate.request.attempt_ref()
            || entry.event.event_id == candidate.event.event_id
            || entry.event.message_id == candidate.event.message_id
    }) {
        return Err(AttemptJournalError::Conflict);
    }
    Ok(())
}

fn insert(
    transaction: &Transaction<'_>,
    candidate: &Candidate,
    cursor: i64,
) -> Result<(), AttemptJournalError> {
    let admission = &candidate.admission;
    let changed = transaction.execute(
        "INSERT INTO attempt_admissions(cursor,attempt_id,idempotency_key,request_json,request_sha256) VALUES(?1,?2,?3,?4,?5)",
        params![cursor, admission.request.attempt_ref().entity_id, admission.request.idempotency_key(),
            candidate.request_json, admission.request_sha256],
    ).map_err(sqlite_error)?;
    if changed != 1 {
        return Err(AttemptJournalError::Corrupt);
    }
    let changed = transaction.execute(
        "INSERT INTO attempt_events(cursor,event_id,message_id,event_json,event_sha256) VALUES(?1,?2,?3,?4,?5)",
        params![cursor, admission.event.event_id, admission.event.message_id,
            admission.canonical_event_json, admission.event_sha256],
    ).map_err(sqlite_error)?;
    if changed != 1 {
        return Err(AttemptJournalError::Corrupt);
    }
    let changed = transaction
        .execute(
            "INSERT INTO attempt_outbox(cursor,event_id) VALUES(?1,?2)",
            params![cursor, admission.event.event_id],
        )
        .map_err(sqlite_error)?;
    if changed != 1 {
        return Err(AttemptJournalError::Corrupt);
    }
    Ok(())
}
