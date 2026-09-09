use rusqlite::Connection;

use super::{
    AttemptJournalError, MAX_ADMISSIONS, MAX_EVENT_BYTES, MAX_REQUEST_BYTES, MAX_TOTAL_BYTES,
    sqlite_error,
};

pub(super) fn audit(connection: &Connection) -> Result<(usize, usize), AttemptJournalError> {
    let (admissions, request_bytes) = record_sizes(
        connection,
        "SELECT CASE WHEN length(CAST(attempt_id AS BLOB))=30
         AND length(CAST(idempotency_key AS BLOB)) BETWEEN 16 AND 128
         AND length(CAST(request_sha256 AS BLOB))=64
         THEN length(CAST(request_json AS BLOB)) ELSE -1 END
         FROM attempt_admissions ORDER BY cursor LIMIT 1025",
        MAX_REQUEST_BYTES,
    )?;
    let (events, event_bytes) = record_sizes(
        connection,
        "SELECT CASE WHEN length(CAST(event_id AS BLOB))=30 AND length(CAST(message_id AS BLOB))=30
         AND length(CAST(event_sha256 AS BLOB))=64 THEN length(CAST(event_json AS BLOB)) ELSE -1 END
         FROM attempt_events ORDER BY cursor LIMIT 1025",
        MAX_EVENT_BYTES,
    )?;
    let (outbox, _) = record_sizes(
        connection,
        "SELECT CASE WHEN length(CAST(event_id AS BLOB))=30 THEN 1 ELSE -1 END
         FROM attempt_outbox ORDER BY cursor LIMIT 1025",
        1,
    )?;
    let total = request_bytes
        .checked_add(event_bytes)
        .ok_or(AttemptJournalError::Corrupt)?;
    if admissions != events || events != outbox || total > MAX_TOTAL_BYTES {
        return Err(AttemptJournalError::Corrupt);
    }
    Ok((admissions, total))
}

fn record_sizes(
    connection: &Connection,
    query: &str,
    maximum: usize,
) -> Result<(usize, usize), AttemptJournalError> {
    let mut statement = connection.prepare(query).map_err(sqlite_error)?;
    let mut rows = statement.query([]).map_err(sqlite_error)?;
    let mut count = 0;
    let mut bytes = 0_usize;
    while let Some(row) = rows.next().map_err(sqlite_error)? {
        let length: i64 = row.get(0).map_err(|_| AttemptJournalError::Corrupt)?;
        let length = usize::try_from(length).map_err(|_| AttemptJournalError::Corrupt)?;
        count += 1;
        if !(1..=maximum).contains(&length) || count > MAX_ADMISSIONS {
            return Err(AttemptJournalError::Corrupt);
        }
        bytes = bytes
            .checked_add(length)
            .ok_or(AttemptJournalError::Corrupt)?;
        if bytes > MAX_TOTAL_BYTES {
            return Err(AttemptJournalError::Corrupt);
        }
    }
    Ok((count, bytes))
}
