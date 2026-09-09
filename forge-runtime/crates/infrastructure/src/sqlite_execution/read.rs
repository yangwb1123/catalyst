use std::collections::BTreeSet;

use crate::runtime_domain::platform_core_contract::decode_canonical_event_envelope;
use rusqlite::{Connection, Row};

use super::{
    AttemptAdmission, AttemptJournalError, MAX_PAGE_BYTES, PendingEvent, PendingPage, budget,
    codec, event, schema, sqlite_error,
};

const ROWS_SQL: &str =
    "SELECT a.cursor,a.attempt_id,a.idempotency_key,a.request_json,a.request_sha256,
    e.cursor,e.event_id,e.message_id,e.event_json,e.event_sha256,o.cursor,o.event_id
    FROM attempt_admissions a LEFT JOIN attempt_events e ON e.cursor=a.cursor
    LEFT JOIN attempt_outbox o ON o.cursor=a.cursor ORDER BY a.cursor LIMIT 1025";

pub(super) struct Snapshot {
    pub admissions: Vec<AttemptAdmission>,
    pub total_bytes: usize,
}

pub(super) fn audit(connection: &Connection) -> Result<Snapshot, AttemptJournalError> {
    schema::validate(connection)?;
    audit_records(connection)
}

pub(super) fn audit_records(connection: &Connection) -> Result<Snapshot, AttemptJournalError> {
    let (count, total_bytes) = budget::audit(connection)?;
    let mut statement = connection.prepare(ROWS_SQL).map_err(sqlite_error)?;
    let mut rows = statement.query([]).map_err(sqlite_error)?;
    let mut admissions = Vec::with_capacity(count);
    while let Some(row) = rows.next().map_err(sqlite_error)? {
        let raw = RawAdmission::from_row(row).map_err(|_| AttemptJournalError::Corrupt)?;
        let expected_cursor =
            u64::try_from(admissions.len() + 1).map_err(|_| AttemptJournalError::Corrupt)?;
        admissions.push(decode(&raw, expected_cursor)?);
    }
    if admissions.len() != count {
        return Err(AttemptJournalError::Corrupt);
    }
    validate_unique(&admissions)?;
    Ok(Snapshot {
        admissions,
        total_bytes,
    })
}

fn validate_unique(admissions: &[AttemptAdmission]) -> Result<(), AttemptJournalError> {
    let mut attempts = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut events = BTreeSet::new();
    let mut messages = BTreeSet::new();
    for admission in admissions {
        if !attempts.insert(&admission.request.attempt_ref().entity_id)
            || !keys.insert(admission.request.idempotency_key())
            || !events.insert(&admission.event.event_id)
            || !messages.insert(&admission.event.message_id)
        {
            return Err(AttemptJournalError::Corrupt);
        }
    }
    Ok(())
}

fn decode(
    raw: &RawAdmission,
    expected_cursor: u64,
) -> Result<AttemptAdmission, AttemptJournalError> {
    let request = codec::decode(&raw.request_json)?;
    let envelope = decode_canonical_event_envelope(raw.event_json.as_bytes())
        .map_err(|_| AttemptJournalError::Corrupt)?;
    let mut candidate =
        event::prepare(&request, &envelope).map_err(|_| AttemptJournalError::Corrupt)?;
    let cursor = u64::try_from(raw.cursor).map_err(|_| AttemptJournalError::Corrupt)?;
    if cursor != expected_cursor
        || raw.event_cursor != raw.cursor
        || raw.outbox_cursor != raw.cursor
        || raw.attempt_id != request.attempt_ref().entity_id
        || raw.idempotency_key != request.idempotency_key()
        || raw.request_json != candidate.request_json
        || raw.request_sha256 != candidate.admission.request_sha256
        || raw.event_id != envelope.event_id
        || raw.outbox_event_id != envelope.event_id
        || raw.message_id != envelope.message_id
        || raw.event_json != candidate.admission.canonical_event_json
        || raw.event_sha256 != candidate.admission.event_sha256
    {
        return Err(AttemptJournalError::Corrupt);
    }
    candidate.admission.cursor = cursor;
    Ok(candidate.admission)
}

pub(super) fn page(
    snapshot: Snapshot,
    after_cursor: u64,
    limit: usize,
) -> Result<PendingPage, AttemptJournalError> {
    let head_cursor =
        u64::try_from(snapshot.admissions.len()).map_err(|_| AttemptJournalError::Corrupt)?;
    if after_cursor > head_cursor {
        return Err(AttemptJournalError::Invalid);
    }
    let mut next_cursor = after_cursor;
    let mut bytes = 0;
    let mut events = Vec::new();
    for admission in snapshot
        .admissions
        .into_iter()
        .filter(|entry| entry.cursor > after_cursor)
    {
        if events.len() == limit || bytes + admission.canonical_event_json.len() > MAX_PAGE_BYTES {
            break;
        }
        next_cursor = admission.cursor;
        bytes += admission.canonical_event_json.len();
        events.push(PendingEvent {
            cursor: admission.cursor,
            event: admission.event,
            canonical_event_json: admission.canonical_event_json,
            event_sha256: admission.event_sha256,
        });
    }
    Ok(PendingPage {
        head_cursor,
        next_cursor,
        has_more: next_cursor < head_cursor,
        events,
    })
}

struct RawAdmission {
    cursor: i64,
    attempt_id: String,
    idempotency_key: String,
    request_json: String,
    request_sha256: String,
    event_cursor: i64,
    event_id: String,
    message_id: String,
    event_json: String,
    event_sha256: String,
    outbox_cursor: i64,
    outbox_event_id: String,
}

impl RawAdmission {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            cursor: row.get(0)?,
            attempt_id: row.get(1)?,
            idempotency_key: row.get(2)?,
            request_json: row.get(3)?,
            request_sha256: row.get(4)?,
            event_cursor: row.get(5)?,
            event_id: row.get(6)?,
            message_id: row.get(7)?,
            event_json: row.get(8)?,
            event_sha256: row.get(9)?,
            outbox_cursor: row.get(10)?,
            outbox_event_id: row.get(11)?,
        })
    }
}
