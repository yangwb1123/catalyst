use super::super::{HubStoreError, MIGRATE_V25_TO_V26_SQL};
use rusqlite::{Connection, Error as SqliteError, ErrorCode, OptionalExtension};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
#[path = "full_contract/divergent_v25.rs"]
mod divergent_v25;
#[path = "full_contract/errors.rs"]
mod errors;
#[path = "full_contract/legacy_digest.rs"]
mod legacy_digest;
#[path = "full_contract/metadata.rs"]
mod metadata;
#[path = "full_contract/structure.rs"]
mod structure;
#[path = "full_contract/v17_v21.rs"]
mod v17_v21;
#[path = "full_contract/v22.rs"]
mod v22;
#[path = "full_contract/v23.rs"]
mod v23;
#[path = "full_contract/v24.rs"]
mod v24;
#[path = "full_contract/v25.rs"]
mod v25;
#[path = "full_contract/v27.rs"]
mod v27;
#[path = "full_contract/v28.rs"]
mod v28;
#[path = "full_contract/v29.rs"]
mod v29;
#[path = "full_contract/v30.rs"]
mod v30;
#[path = "full_contract/v31.rs"]
mod v31;
#[path = "full_contract/v32.rs"]
mod v32;
#[path = "full_contract/v33.rs"]
mod v33;
#[path = "full_contract/v34.rs"]
mod v34;
use errors::{invalid, stringify, unavailable};
use legacy_digest::{
    V6_IMPLICIT_INDEX_COUNT, V6_STRUCTURAL_CONTRACT_SHA256, V7_IMPLICIT_INDEX_COUNT,
    V7_STRUCTURAL_CONTRACT_SHA256, V8_IMPLICIT_INDEX_COUNT, V8_STRUCTURAL_CONTRACT_SHA256,
    V9_IMPLICIT_INDEX_COUNT, V9_STRUCTURAL_CONTRACT_SHA256, V10_IMPLICIT_INDEX_COUNT,
    V10_STRUCTURAL_CONTRACT_SHA256, V11_IMPLICIT_INDEX_COUNT, V11_STRUCTURAL_CONTRACT_SHA256,
    V12_IMPLICIT_INDEX_COUNT, V12_STRUCTURAL_CONTRACT_SHA256, V13_IMPLICIT_INDEX_COUNT,
    V13_STRUCTURAL_CONTRACT_SHA256, V14_IMPLICIT_INDEX_COUNT, V14_STRUCTURAL_CONTRACT_SHA256,
    V15_IMPLICIT_INDEX_COUNT, V15_STRUCTURAL_CONTRACT_SHA256, V16_IMPLICIT_INDEX_COUNT,
    V16_STRUCTURAL_CONTRACT_SHA256,
};
use metadata::{OWNED_TABLES, SCHEMA_BATCHES, VERSION_EXPLICIT_INDEX_COUNTS, VERSION_TABLE_COUNTS};
use v17_v21::{
    V17_IMPLICIT_INDEX_COUNT, V17_STRUCTURAL_CONTRACT_SHA256, V18_IMPLICIT_INDEX_COUNT,
    V18_STRUCTURAL_CONTRACT_SHA256, V19_IMPLICIT_INDEX_COUNT, V19_STRUCTURAL_CONTRACT_SHA256,
    V20_IMPLICIT_INDEX_COUNT, V20_STRUCTURAL_CONTRACT_SHA256, V21_IMPLICIT_INDEX_COUNT,
    V21_STRUCTURAL_CONTRACT_SHA256,
};
const STRUCTURAL_DIGEST_DOMAIN: &[u8] = b"forge-hub-structural-contract-v1\0";
static EXPECTED_SCHEMAS: OnceLock<Result<Vec<ExpectedSchema>, String>> = OnceLock::new();

struct ExpectedSchema {
    version: usize,
    catalog: CatalogSignature,
    tables: Vec<ExpectedTable>,
}
struct ExpectedTable {
    name: &'static str,
    sql: String,
    signature: structure::TableSignature,
    indexes: Vec<ExpectedIndex>,
}
struct ExpectedIndex {
    name: String,
    sql: String,
}

#[derive(Default, PartialEq, Eq)]
struct CatalogSignature {
    tables: Vec<String>,
    explicit_indexes: Vec<String>,
    implicit_index_owners: Vec<String>,
    views: Vec<String>,
    triggers: Vec<String>,
    other_objects: Vec<(String, String)>,
}

pub(super) fn validate_version(connection: &Connection, version: i64) -> Result<(), HubStoreError> {
    let index = usize::try_from(version)
        .ok()
        .filter(|index| *index < VERSION_TABLE_COUNTS.len())
        .ok_or_else(|| invalid(version, "schema", "version"))?;
    let expected = &expected_schemas()?[index];
    validate_catalog(connection, version, &expected.catalog)?;
    for table in &expected.tables {
        validate_table(connection, version, table)?;
    }
    Ok(())
}

pub(super) fn validate_endpoint_only_v25(connection: &Connection) -> Result<(), HubStoreError> {
    divergent_v25::validate(connection)
}
fn validate_catalog(
    connection: &Connection,
    version: i64,
    expected: &CatalogSignature,
) -> Result<(), HubStoreError> {
    let actual = catalog(connection).map_err(sqlite_error)?;
    if &actual != expected {
        return Err(invalid(version, "main catalog", "object inventory"));
    }
    Ok(())
}
fn validate_table(
    connection: &Connection,
    version: i64,
    expected: &ExpectedTable,
) -> Result<(), HubStoreError> {
    validate_definition(connection, version, "table", expected.name, &expected.sql)?;
    let actual = structure::inspect(connection, expected.name).map_err(sqlite_error)?;
    if actual.signature != expected.signature {
        return Err(invalid(version, expected.name, "structural signature"));
    }
    validate_indexes(connection, version, expected, &actual.explicit_indexes)
}
fn validate_indexes(
    connection: &Connection,
    version: i64,
    table: &ExpectedTable,
    actual_names: &[String],
) -> Result<(), HubStoreError> {
    let expected_names = table
        .indexes
        .iter()
        .map(|index| index.name.as_str())
        .collect::<Vec<_>>();
    if actual_names.iter().map(String::as_str).collect::<Vec<_>>() != expected_names {
        return Err(invalid(version, table.name, "explicit index inventory"));
    }
    for index in &table.indexes {
        validate_definition(connection, version, "index", &index.name, &index.sql)?;
    }
    Ok(())
}
fn validate_definition(
    connection: &Connection,
    version: i64,
    kind: &str,
    name: &str,
    expected: &str,
) -> Result<(), HubStoreError> {
    let actual = definition(connection, kind, name).map_err(sqlite_error)?;
    if actual.as_deref() != Some(expected) {
        return Err(invalid(version, name, "sqlite_schema definition"));
    }
    Ok(())
}
fn expected_schemas() -> Result<&'static [ExpectedSchema], HubStoreError> {
    match EXPECTED_SCHEMAS.get_or_init(load_expected_schemas) {
        Ok(schemas) => Ok(schemas),
        Err(message) => Err(unavailable(message)),
    }
}
fn load_expected_schemas() -> Result<Vec<ExpectedSchema>, String> {
    let connection = Connection::open_in_memory().map_err(stringify)?;
    let mut schemas = Vec::with_capacity(SCHEMA_BATCHES.len() + 1);
    for version in 0..=SCHEMA_BATCHES.len() {
        if version > 0 {
            connection
                .execute_batch(SCHEMA_BATCHES[version - 1])
                .map_err(stringify)?;
        }
        schemas.push(load_expected_schema(&connection, version)?);
    }
    Ok(schemas)
}
fn load_expected_schema(connection: &Connection, version: usize) -> Result<ExpectedSchema, String> {
    let table_count = VERSION_TABLE_COUNTS[version];
    let tables = OWNED_TABLES[..table_count]
        .iter()
        .map(|&table| load_expected_table(connection, table))
        .collect::<Result<Vec<_>, _>>()?;
    let schema = ExpectedSchema {
        version,
        catalog: catalog(connection).map_err(stringify)?,
        tables,
    };
    validate_generated_contract(&schema)?;
    if version >= 6 {
        validate_release_structure(&schema)?;
    }
    Ok(schema)
}
fn validate_generated_contract(schema: &ExpectedSchema) -> Result<(), String> {
    let mut table_names = OWNED_TABLES[..VERSION_TABLE_COUNTS[schema.version]]
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
    let valid = schema.catalog.tables == table_names
        && schema.catalog.explicit_indexes == index_names
        && index_names.len() == VERSION_EXPLICIT_INDEX_COUNTS[schema.version]
        && schema.catalog.views.is_empty()
        && schema.catalog.triggers.is_empty()
        && schema.catalog.other_objects.is_empty();
    valid
        .then_some(())
        .ok_or_else(|| format!("generated Hub v{} catalog is invalid", schema.version))
}
fn validate_release_structure(schema: &ExpectedSchema) -> Result<(), String> {
    let (expected_indexes, expected_digest) = release_structural_contract(schema.version)?;
    let implicit_indexes = schema
        .tables
        .iter()
        .map(|table| table.signature.implicit_index_count())
        .sum::<usize>();
    let expected_catalog_index_owners = match schema.version {
        31 => v31::catalog_implicit_index_count(schema.version, expected_indexes),
        32 => v32::catalog_implicit_index_count(schema.version, expected_indexes),
        33 => v33::catalog_implicit_index_count(schema.version, expected_indexes),
        34 => v34::catalog_implicit_index_count(schema.version, expected_indexes),
        _ => v30::catalog_implicit_index_count(schema.version, expected_indexes),
    };
    if implicit_indexes != expected_indexes
        || schema.catalog.implicit_index_owners.len() != expected_catalog_index_owners
    {
        return Err(format!(
            "generated Hub v{} has {implicit_indexes} structural implicit indexes and {} catalog owners; expected {expected_indexes} and {}",
            schema.version,
            schema.catalog.implicit_index_owners.len(),
            expected_catalog_index_owners
        ));
    }
    let digest = structural_digest(&schema.tables);
    if digest != expected_digest {
        return Err(format!(
            "generated Hub v{} structural digest changed: {digest:02x?}",
            schema.version
        ));
    }
    Ok(())
}
#[allow(clippy::too_many_lines)] // The released per-version digests are kept in one audit table.
fn release_structural_contract(version: usize) -> Result<(usize, [u8; 32]), String> {
    if let Some(contract) = release_contract_through_v21(version) {
        return Ok(contract);
    }
    Ok(match version {
        22 => (
            v22::V22_IMPLICIT_INDEX_COUNT,
            v22::V22_STRUCTURAL_CONTRACT_SHA256,
        ),
        23 => (
            v23::V23_IMPLICIT_INDEX_COUNT,
            v23::V23_STRUCTURAL_CONTRACT_SHA256,
        ),
        24 => (
            v24::V24_IMPLICIT_INDEX_COUNT,
            v24::V24_STRUCTURAL_CONTRACT_SHA256,
        ),
        25 | 26 => (
            v25::V25_IMPLICIT_INDEX_COUNT,
            v25::V25_STRUCTURAL_CONTRACT_SHA256,
        ),
        27 => (
            v27::V27_IMPLICIT_INDEX_COUNT,
            v27::V27_STRUCTURAL_CONTRACT_SHA256,
        ),
        28 => (
            v28::V28_IMPLICIT_INDEX_COUNT,
            v28::V28_STRUCTURAL_CONTRACT_SHA256,
        ),
        29 => v29::V29_STRUCTURAL_CONTRACT,
        30 => v30::V30_STRUCTURAL_CONTRACT,
        31 => v31::V31_STRUCTURAL_CONTRACT,
        32 => v32::V32_STRUCTURAL_CONTRACT,
        33 => v33::V33_STRUCTURAL_CONTRACT,
        34 => v34::V34_STRUCTURAL_CONTRACT,
        version => {
            return Err(format!("Hub v{version} has no release structural contract"));
        }
    })
}

fn release_contract_through_v21(version: usize) -> Option<(usize, [u8; 32])> {
    match version {
        6 => Some((V6_IMPLICIT_INDEX_COUNT, V6_STRUCTURAL_CONTRACT_SHA256)),
        7 => Some((V7_IMPLICIT_INDEX_COUNT, V7_STRUCTURAL_CONTRACT_SHA256)),
        8 => Some((V8_IMPLICIT_INDEX_COUNT, V8_STRUCTURAL_CONTRACT_SHA256)),
        9 => Some((V9_IMPLICIT_INDEX_COUNT, V9_STRUCTURAL_CONTRACT_SHA256)),
        10 => Some((V10_IMPLICIT_INDEX_COUNT, V10_STRUCTURAL_CONTRACT_SHA256)),
        11 => Some((V11_IMPLICIT_INDEX_COUNT, V11_STRUCTURAL_CONTRACT_SHA256)),
        12 => Some((V12_IMPLICIT_INDEX_COUNT, V12_STRUCTURAL_CONTRACT_SHA256)),
        13 => Some((V13_IMPLICIT_INDEX_COUNT, V13_STRUCTURAL_CONTRACT_SHA256)),
        14 => Some((V14_IMPLICIT_INDEX_COUNT, V14_STRUCTURAL_CONTRACT_SHA256)),
        15 => Some((V15_IMPLICIT_INDEX_COUNT, V15_STRUCTURAL_CONTRACT_SHA256)),
        16 => Some((V16_IMPLICIT_INDEX_COUNT, V16_STRUCTURAL_CONTRACT_SHA256)),
        17 => Some((V17_IMPLICIT_INDEX_COUNT, V17_STRUCTURAL_CONTRACT_SHA256)),
        18 => Some((V18_IMPLICIT_INDEX_COUNT, V18_STRUCTURAL_CONTRACT_SHA256)),
        19 => Some((V19_IMPLICIT_INDEX_COUNT, V19_STRUCTURAL_CONTRACT_SHA256)),
        20 => Some((V20_IMPLICIT_INDEX_COUNT, V20_STRUCTURAL_CONTRACT_SHA256)),
        21 => Some((V21_IMPLICIT_INDEX_COUNT, V21_STRUCTURAL_CONTRACT_SHA256)),
        _ => None,
    }
}
fn structural_digest(tables: &[ExpectedTable]) -> [u8; 32] {
    let mut ordered = tables.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by(|left, right| left.name.cmp(right.name));
    let mut encoding = STRUCTURAL_DIGEST_DOMAIN.to_vec();
    structure::push_len(&mut encoding, ordered.len());
    for table in ordered {
        structure::push_string(&mut encoding, table.name);
        table.signature.encode(&mut encoding);
    }
    Sha256::digest(&encoding).into()
}

fn load_expected_table(
    connection: &Connection,
    name: &'static str,
) -> Result<ExpectedTable, String> {
    let sql = required_definition(connection, "table", name)?;
    let inspection = structure::inspect(connection, name).map_err(stringify)?;
    let indexes = inspection
        .explicit_indexes
        .into_iter()
        .map(|name| load_expected_index(connection, name))
        .collect::<Result<_, _>>()?;
    Ok(ExpectedTable {
        name,
        sql,
        signature: inspection.signature,
        indexes,
    })
}

fn load_expected_index(connection: &Connection, name: String) -> Result<ExpectedIndex, String> {
    let sql = required_definition(connection, "index", &name)?;
    Ok(ExpectedIndex { name, sql })
}

fn required_definition(connection: &Connection, kind: &str, name: &str) -> Result<String, String> {
    definition(connection, kind, name)
        .map_err(stringify)?
        .ok_or_else(|| format!("expected {kind} {name} is absent"))
}

fn definition(connection: &Connection, kind: &str, name: &str) -> rusqlite::Result<Option<String>> {
    connection
        .query_row(
            "SELECT sql FROM main.sqlite_schema WHERE type = ?1 AND name = ?2",
            [kind, name],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(Option::flatten)
}

fn catalog(connection: &Connection) -> rusqlite::Result<CatalogSignature> {
    let mut statement = connection
        .prepare("SELECT type,name,tbl_name,sql FROM main.sqlite_schema ORDER BY type,name")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut catalog = CatalogSignature::default();
    for (kind, name, owner, sql) in rows {
        if kind == "index" && sql.is_none() {
            catalog.implicit_index_owners.push(owner);
            continue;
        }
        match kind.as_str() {
            "table" => catalog.tables.push(name),
            "index" if sql.is_some() => catalog.explicit_indexes.push(name),
            "index" => {}
            "view" => catalog.views.push(name),
            "trigger" => catalog.triggers.push(name),
            _ => catalog.other_objects.push((kind, name)),
        }
    }
    catalog.sort();
    Ok(catalog)
}

impl CatalogSignature {
    fn sort(&mut self) {
        self.tables.sort();
        self.explicit_indexes.sort();
        self.implicit_index_owners.sort();
        self.views.sort();
        self.triggers.sort();
        self.other_objects.sort();
    }
}

pub(super) fn sqlite_error(error: SqliteError) -> HubStoreError {
    let corrupt = matches!(
        &error,
        SqliteError::SqliteFailure(problem, _)
            if matches!(
                problem.code,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
            )
    );
    if corrupt {
        return HubStoreError::Corrupt {
            message: error.to_string(),
        };
    }
    unavailable(error)
}
