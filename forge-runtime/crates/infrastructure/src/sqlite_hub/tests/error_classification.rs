use rusqlite::{Error as SqliteError, ffi};

use crate::runtime_domain::{HubEntity, HubStoreError};

use super::{read_error, write_error};

#[test]
fn data_corruption_errors_are_never_reported_as_unavailable() {
    for code in [ffi::SQLITE_CORRUPT, ffi::SQLITE_NOTADB] {
        assert!(matches!(
            read_error(sqlite_error(code)),
            HubStoreError::Corrupt { .. }
        ));
        assert!(matches!(
            write_error(HubEntity::GroupAnalysisPanel, sqlite_error(code)),
            HubStoreError::Corrupt { .. }
        ));
    }
}

fn sqlite_error(code: i32) -> SqliteError {
    SqliteError::SqliteFailure(ffi::Error::new(code), None)
}
