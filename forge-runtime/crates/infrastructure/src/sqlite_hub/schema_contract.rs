use rusqlite::Connection;

#[path = "schema_contract/full_contract.rs"]
mod full_contract;

pub(super) fn validate_version(
    connection: &Connection,
    version: i64,
) -> Result<(), super::HubStoreError> {
    full_contract::validate_version(connection, version)
}

pub(super) fn sqlite_error(error: rusqlite::Error) -> super::HubStoreError {
    full_contract::sqlite_error(error)
}
