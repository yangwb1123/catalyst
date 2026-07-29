use std::sync::OnceLock;

use rusqlite::{Connection, Error as SqliteError, ErrorCode, OptionalExtension};
use sha2::{Digest, Sha256};

use super::super::{
    CREATE_V1_SCHEMA_SQL, HubStoreError, MIGRATE_V1_TO_V2_SQL, MIGRATE_V2_TO_V3_SQL,
    MIGRATE_V3_TO_V4_SQL, MIGRATE_V4_TO_V5_SQL,
};

#[path = "full_contract/structure.rs"]
mod structure;

const OWNED_TABLES: &[&str] = &[
    "projects",
    "groups",
    "conversations",
    "group_projects",
    "prompts",
    "runs",
    "run_events",
    "run_assistant_prompts",
    "group_runs",
    "group_executions",
    "group_execution_events",
    "group_model_analyses",
    "group_model_analysis_events",
    "group_model_analysis_results",
];
const SCHEMA_BATCHES: &[&str] = &[
    CREATE_V1_SCHEMA_SQL,
    MIGRATE_V1_TO_V2_SQL,
    MIGRATE_V2_TO_V3_SQL,
    MIGRATE_V3_TO_V4_SQL,
    MIGRATE_V4_TO_V5_SQL,
];
const VERSION_TABLE_COUNTS: [usize; 6] = [0, 5, 8, 9, 11, 14];
const VERSION_EXPLICIT_INDEX_COUNTS: [usize; 6] = [0, 2, 3, 4, 6, 8];
const IMPLICIT_INDEX_COUNT: usize = 25;
const STRUCTURAL_DIGEST_DOMAIN: &[u8] = b"forge-hub-structural-contract-v1\0";
const STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x79, 0x0b, 0x05, 0xcb, 0x9b, 0x27, 0x27, 0x75, 0x58, 0x29, 0xf4, 0x2f, 0xae, 0x47, 0xe3, 0xd0,
    0x19, 0x31, 0x70, 0xac, 0xdb, 0xa4, 0x14, 0x15, 0xa2, 0x00, 0x54, 0x44, 0xe7, 0x97, 0xbb, 0xf9,
];

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
    if version == SCHEMA_BATCHES.len() {
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
    let implicit_indexes = schema
        .tables
        .iter()
        .map(|table| table.signature.implicit_index_count())
        .sum::<usize>();
    if implicit_indexes != IMPLICIT_INDEX_COUNT
        || schema.catalog.implicit_index_owners.len() != IMPLICIT_INDEX_COUNT
    {
        return Err(format!(
            "generated Hub v5 has {implicit_indexes} implicit indexes; expected {IMPLICIT_INDEX_COUNT}"
        ));
    }
    let digest = structural_digest(&schema.tables);
    if digest != STRUCTURAL_CONTRACT_SHA256 {
        return Err(format!(
            "generated Hub v5 structural digest changed: {digest:02x?}"
        ));
    }
    Ok(())
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
        if name.starts_with("sqlite_") {
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

fn invalid(version: i64, object: &str, detail: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: format!("Hub v{version} {object} has invalid {detail}"),
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

fn unavailable(error: impl std::fmt::Display) -> HubStoreError {
    HubStoreError::Unavailable {
        message: error.to_string(),
    }
}

fn stringify(error: impl std::fmt::Display) -> String {
    error.to_string()
}
