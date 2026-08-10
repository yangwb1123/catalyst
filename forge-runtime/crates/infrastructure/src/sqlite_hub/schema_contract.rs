use rusqlite::Connection;

#[path = "schema_contract/full_contract.rs"]
mod full_contract;

pub(super) fn validate_version(
    connection: &Connection,
    version: i64,
) -> Result<(), super::HubStoreError> {
    full_contract::validate_version(connection, version)
}

pub(super) fn validate_migration_source(
    connection: &Connection,
    version: i64,
) -> Result<bool, super::HubStoreError> {
    match full_contract::validate_version(connection, version) {
        Ok(()) => Ok(false),
        Err(canonical_error) if version == 25 => {
            full_contract::validate_endpoint_only_v25(connection)
                .map(|()| true)
                .map_err(|_| canonical_error)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn sqlite_error(error: rusqlite::Error) -> super::HubStoreError {
    full_contract::sqlite_error(error)
}
