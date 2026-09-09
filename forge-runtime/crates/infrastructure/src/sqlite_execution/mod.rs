//! Requested-only Attempt persistence over an explicitly caller-opened database.
//!
//! Filesystem selection, permissions, descriptor safety and SQLite/VFS trust belong
//! to the caller. Stored declarations do not authorize execution or prove effects.

mod budget;
mod codec;
mod codec_model;
mod event;
mod read;
mod schema;
mod schema_sql;
mod types;
mod write;

#[cfg(test)]
mod codec_tests;
#[cfg(test)]
mod corruption_tests;
#[cfg(test)]
mod crash_fixture;
#[cfg(test)]
mod crash_tests;

use crate::runtime_domain::execution::attempt::AttemptRequest;
use crate::runtime_domain::platform_core_contract::{EventEnvelope, validate_platform_id};
use rusqlite::{Connection, TransactionBehavior};

pub use types::{
    AdmissionDisposition, AdmissionResult, AttemptAdmission, AttemptJournalError, PendingEvent,
    PendingPage,
};

pub(super) const MAX_ADMISSIONS: usize = 1_024;
pub(super) const MAX_REQUEST_BYTES: usize = 64 * 1_024;
pub(super) const MAX_EVENT_BYTES: usize = 16 * 1_024;
pub(super) const MAX_TOTAL_BYTES: usize = 32 * 1_024 * 1_024;
pub(super) const MAX_PAGE_BYTES: usize = 256 * 1_024;

/// A private owned connection containing only requested admissions and pending events.
#[derive(Debug)]
pub struct SqliteAttemptJournal {
    connection: Connection,
}

impl SqliteAttemptJournal {
    /// Initializes an empty database or verifies this exact storage profile.
    ///
    /// # Errors
    /// Rejects unsupported connections, foreign/corrupt profiles and `SQLite` failures.
    pub fn from_connection(mut connection: Connection) -> Result<Self, AttemptJournalError> {
        schema::open(&mut connection)?;
        Ok(Self { connection })
    }

    /// Atomically stores a requested admission or returns its original exact replay.
    ///
    /// # Errors
    /// Returns invalid input, identity conflict, capacity, corruption or availability errors.
    pub fn admit(
        &mut self,
        request: &AttemptRequest,
        event: &EventEnvelope,
    ) -> Result<AdmissionResult, AttemptJournalError> {
        let candidate = event::prepare(request, event)?;
        write::admit(&mut self.connection, candidate)
    }

    /// Returns an owned admission from a fully validated transaction snapshot.
    ///
    /// # Errors
    /// Rejects malformed Attempt IDs, corrupt storage and unavailable `SQLite` operations.
    pub fn get(
        &mut self,
        attempt_id: &str,
    ) -> Result<Option<AttemptAdmission>, AttemptJournalError> {
        if !attempt_id.starts_with("atm_") || validate_platform_id(attempt_id).is_err() {
            return Err(AttemptJournalError::Invalid);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(sqlite_error)?;
        let snapshot = read::audit(&transaction)?;
        let result = snapshot
            .admissions
            .into_iter()
            .find(|entry| entry.request.attempt_ref().entity_id == attempt_id);
        transaction.commit().map_err(sqlite_error)?;
        Ok(result)
    }

    /// Returns retained pending events with a store-local cursor and snapshot head.
    ///
    /// # Errors
    /// Rejects invalid cursor/page bounds, corrupt storage and `SQLite` failures.
    pub fn pending(
        &mut self,
        after_cursor: u64,
        limit: usize,
    ) -> Result<PendingPage, AttemptJournalError> {
        if !(1..=64).contains(&limit) {
            return Err(AttemptJournalError::Invalid);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(sqlite_error)?;
        let snapshot = read::audit(&transaction)?;
        let page = read::page(snapshot, after_cursor, limit)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(page)
    }
}

/// Computes the private request-record digest needed for the creation-event binding.
/// This is pure preparation and reserves no identity or execution authority.
///
/// # Errors
/// Returns `Invalid` if the record exceeds the bounded storage encoding.
pub fn request_sha256(request: &AttemptRequest) -> Result<String, AttemptJournalError> {
    codec::encode(request).map(|record| record.sha256)
}

// A map_err adapter consumes and discards the original diagnostic.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn sqlite_error(error: rusqlite::Error) -> AttemptJournalError {
    use rusqlite::{Error, ErrorCode};
    match error {
        Error::SqliteFailure(code, _)
            if matches!(
                code.code,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
            ) =>
        {
            AttemptJournalError::Corrupt
        }
        _ => AttemptJournalError::Unavailable,
    }
}
