use std::sync::OnceLock;

use rusqlite::Connection;

use super::{
    ExpectedSchema, HubStoreError, MIGRATE_V25_TO_V26_SQL, OWNED_TABLES, SCHEMA_BATCHES, catalog,
    load_expected_table, structural_digest, unavailable, validate_catalog, validate_table,
};

const TABLE_COUNT: usize = 33;
const EXPLICIT_INDEX_COUNT: usize = 32;
const IMPLICIT_INDEX_COUNT: usize = 86;
const STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x15, 0xf4, 0xca, 0xfa, 0x58, 0x23, 0x32, 0x20, 0x50, 0x80, 0xe6, 0xcf, 0x7a, 0x9a, 0x48, 0x4a,
    0x67, 0x97, 0x87, 0xd4, 0x1e, 0x5c, 0x02, 0xe0, 0x30, 0x09, 0x21, 0xc0, 0xdf, 0xc1, 0xbc, 0x18,
];

static EXPECTED: OnceLock<Result<ExpectedSchema, String>> = OnceLock::new();

pub(super) fn validate(connection: &Connection) -> Result<(), HubStoreError> {
    let expected = match EXPECTED.get_or_init(load) {
        Ok(expected) => expected,
        Err(message) => return Err(unavailable(message)),
    };
    validate_catalog(connection, 25, &expected.catalog)?;
    for table in &expected.tables {
        validate_table(connection, 25, table)?;
    }
    Ok(())
}

fn load() -> Result<ExpectedSchema, String> {
    let connection = Connection::open_in_memory().map_err(stringify)?;
    for batch in &SCHEMA_BATCHES[..24] {
        connection.execute_batch(batch).map_err(stringify)?;
    }
    connection
        .execute_batch(MIGRATE_V25_TO_V26_SQL)
        .map_err(stringify)?;
    let tables = OWNED_TABLES[..TABLE_COUNT]
        .iter()
        .map(|&table| load_expected_table(&connection, table))
        .collect::<Result<Vec<_>, _>>()?;
    let schema = ExpectedSchema {
        version: 25,
        catalog: catalog(&connection).map_err(stringify)?,
        tables,
    };
    validate_generated(&schema)?;
    Ok(schema)
}

fn validate_generated(schema: &ExpectedSchema) -> Result<(), String> {
    let mut table_names = OWNED_TABLES[..TABLE_COUNT]
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    table_names.sort();
    let mut index_names = schema
        .tables
        .iter()
        .flat_map(|table| table.indexes.iter())
        .map(|index| index.name.clone())
        .collect::<Vec<_>>();
    index_names.sort();
    let implicit_indexes = schema
        .tables
        .iter()
        .map(|table| table.signature.implicit_index_count())
        .sum::<usize>();
    let valid = schema.catalog.tables == table_names
        && schema.catalog.explicit_indexes == index_names
        && index_names.len() == EXPLICIT_INDEX_COUNT
        && implicit_indexes == IMPLICIT_INDEX_COUNT
        && schema.catalog.implicit_index_owners.len() == IMPLICIT_INDEX_COUNT
        && schema.catalog.views.is_empty()
        && schema.catalog.triggers.is_empty()
        && schema.catalog.other_objects.is_empty()
        && structural_digest(&schema.tables) == STRUCTURAL_CONTRACT_SHA256;
    valid
        .then_some(())
        .ok_or_else(|| "generated endpoint-only Hub v25 contract is invalid".to_owned())
}

fn stringify(error: impl std::fmt::Display) -> String {
    error.to_string()
}
