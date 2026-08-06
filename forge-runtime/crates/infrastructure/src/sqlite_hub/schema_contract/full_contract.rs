use std::sync::OnceLock;

use rusqlite::{Connection, Error as SqliteError, ErrorCode, OptionalExtension};
use sha2::{Digest, Sha256};

use super::super::{
    CREATE_V1_SCHEMA_SQL, HubStoreError, MIGRATE_V1_TO_V2_SQL, MIGRATE_V2_TO_V3_SQL,
    MIGRATE_V3_TO_V4_SQL, MIGRATE_V4_TO_V5_SQL, MIGRATE_V5_TO_V6_SQL, MIGRATE_V6_TO_V7_SQL,
    MIGRATE_V7_TO_V8_SQL, MIGRATE_V8_TO_V9_SQL, MIGRATE_V9_TO_V10_SQL, MIGRATE_V10_TO_V11_SQL,
    MIGRATE_V11_TO_V12_SQL, MIGRATE_V12_TO_V13_SQL, MIGRATE_V13_TO_V14_SQL, MIGRATE_V14_TO_V15_SQL,
    MIGRATE_V15_TO_V16_SQL, MIGRATE_V16_TO_V17_SQL, MIGRATE_V17_TO_V18_SQL, MIGRATE_V18_TO_V19_SQL,
    MIGRATE_V19_TO_V20_SQL, MIGRATE_V20_TO_V21_SQL,
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
    "group_analysis_panels",
    "group_analysis_panel_analyses",
    "group_panel_syntheses",
    "group_panel_synthesis_events",
    "group_panel_synthesis_results",
    "group_agent_graphs",
    "group_agent_graph_runs",
    "group_agent_graph_run_events",
    "group_agent_graph_node_execution_contracts",
    "group_agent_graph_node_dispatch_requests",
    "group_agent_graph_node_dispatch_claims",
    "group_agent_project_lane_ownerships",
    "group_agent_graph_node_terminal_artifacts",
    "group_agent_graph_node_terminal_receipts",
    "group_agent_graph_execution_schedules",
    "group_agent_graph_scheduled_node_contract_candidates",
    "group_agent_graph_scheduled_node_provider_requests",
    "group_agent_graph_scheduled_node_dispatch_lifecycles",
    "group_agent_graph_scheduled_node_successor_candidates",
];
const SCHEMA_BATCHES: &[&str] = &[
    CREATE_V1_SCHEMA_SQL,
    MIGRATE_V1_TO_V2_SQL,
    MIGRATE_V2_TO_V3_SQL,
    MIGRATE_V3_TO_V4_SQL,
    MIGRATE_V4_TO_V5_SQL,
    MIGRATE_V5_TO_V6_SQL,
    MIGRATE_V6_TO_V7_SQL,
    MIGRATE_V7_TO_V8_SQL,
    MIGRATE_V8_TO_V9_SQL,
    MIGRATE_V9_TO_V10_SQL,
    MIGRATE_V10_TO_V11_SQL,
    MIGRATE_V11_TO_V12_SQL,
    MIGRATE_V12_TO_V13_SQL,
    MIGRATE_V13_TO_V14_SQL,
    MIGRATE_V14_TO_V15_SQL,
    MIGRATE_V15_TO_V16_SQL,
    MIGRATE_V16_TO_V17_SQL,
    MIGRATE_V17_TO_V18_SQL,
    MIGRATE_V18_TO_V19_SQL,
    MIGRATE_V19_TO_V20_SQL,
    MIGRATE_V20_TO_V21_SQL,
];
const VERSION_TABLE_COUNTS: [usize; 22] = [
    0, 5, 8, 9, 11, 14, 16, 19, 20, 22, 23, 24, 28, 29, 30, 31, 32, 33, 33, 33, 33, 33,
];
const VERSION_EXPLICIT_INDEX_COUNTS: [usize; 22] = [
    0, 2, 3, 4, 6, 8, 10, 12, 14, 16, 18, 20, 24, 25, 27, 29, 31, 32, 32, 32, 32, 32,
];
const V6_IMPLICIT_INDEX_COUNT: usize = 29;
const V7_IMPLICIT_INDEX_COUNT: usize = 33;
const V8_IMPLICIT_INDEX_COUNT: usize = 35;
const V9_IMPLICIT_INDEX_COUNT: usize = 38;
const V10_IMPLICIT_INDEX_COUNT: usize = 41;
const V11_IMPLICIT_INDEX_COUNT: usize = 45;
const V12_IMPLICIT_INDEX_COUNT: usize = 61;
const V13_IMPLICIT_INDEX_COUNT: usize = 64;
const V14_IMPLICIT_INDEX_COUNT: usize = 71;
const V15_IMPLICIT_INDEX_COUNT: usize = 79;
const V16_IMPLICIT_INDEX_COUNT: usize = 83;
const V17_IMPLICIT_INDEX_COUNT: usize = 90;
const V18_IMPLICIT_INDEX_COUNT: usize = 90;
const V19_IMPLICIT_INDEX_COUNT: usize = 90;
const V20_IMPLICIT_INDEX_COUNT: usize = 88;
const V21_IMPLICIT_INDEX_COUNT: usize = 88;
const V20_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x72, 0x5f, 0x48, 0xee, 0x23, 0x01, 0x00, 0x56, 0x92, 0x1a, 0x9b, 0x71, 0xeb, 0x31, 0xab, 0x8e,
    0xd0, 0xa0, 0x24, 0x09, 0x9e, 0xff, 0x2a, 0xfd, 0xf0, 0x00, 0x37, 0x1e, 0x53, 0xc7, 0xef, 0xbf,
];
const V21_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x72, 0x5f, 0x48, 0xee, 0x23, 0x01, 0x00, 0x56, 0x92, 0x1a, 0x9b, 0x71, 0xeb, 0x31, 0xab, 0x8e,
    0xd0, 0xa0, 0x24, 0x09, 0x9e, 0xff, 0x2a, 0xfd, 0xf0, 0x00, 0x37, 0x1e, 0x53, 0xc7, 0xef, 0xbf,
];
const V19_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x5a, 0x99, 0x8b, 0xf2, 0x31, 0x6c, 0xcd, 0x31, 0xb4, 0x26, 0x1e, 0x70, 0xf1, 0xe0, 0x1b, 0x68,
    0x5c, 0x26, 0xd6, 0x92, 0x18, 0x51, 0x88, 0x02, 0xff, 0xb1, 0x98, 0x56, 0xb7, 0x2a, 0x8c, 0x65,
];
const V18_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x5a, 0x99, 0x8b, 0xf2, 0x31, 0x6c, 0xcd, 0x31, 0xb4, 0x26, 0x1e, 0x70, 0xf1, 0xe0, 0x1b, 0x68,
    0x5c, 0x26, 0xd6, 0x92, 0x18, 0x51, 0x88, 0x02, 0xff, 0xb1, 0x98, 0x56, 0xb7, 0x2a, 0x8c, 0x65,
];
const V17_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x01, 0x22, 0x6b, 0xac, 0x74, 0xd8, 0x00, 0xd9, 0x3d, 0xa3, 0x3d, 0x75, 0xa6, 0xeb, 0x7a, 0x4e,
    0x53, 0x78, 0xae, 0x8d, 0x2f, 0xe9, 0xd2, 0x6e, 0xcd, 0xef, 0xc6, 0x0b, 0x58, 0xc9, 0x71, 0xe4,
];
const STRUCTURAL_DIGEST_DOMAIN: &[u8] = b"forge-hub-structural-contract-v1\0";
const V6_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x89, 0x39, 0x2a, 0xf9, 0xcd, 0xca, 0x0e, 0xfd, 0x55, 0xd7, 0x28, 0xc7, 0x41, 0xde, 0x1a, 0x01,
    0x05, 0xc9, 0x6b, 0x85, 0x37, 0xd1, 0x41, 0xcd, 0x4d, 0xaf, 0x18, 0x57, 0xd7, 0x31, 0x0c, 0xb9,
];
const V7_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x44, 0xfe, 0xd8, 0x26, 0x8f, 0x1a, 0x30, 0x18, 0x60, 0xf2, 0xad, 0x54, 0x0f, 0x44, 0x81, 0x34,
    0xe3, 0x24, 0x24, 0xc8, 0xb6, 0xee, 0x5b, 0x17, 0xfb, 0x56, 0xa4, 0x2f, 0xcb, 0x5b, 0x34, 0x70,
];
const V8_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x1e, 0xdd, 0xa5, 0x40, 0x70, 0xb6, 0x2b, 0xf9, 0x77, 0x7a, 0x62, 0x16, 0x62, 0x22, 0xf5, 0xf6,
    0x2c, 0x33, 0xd6, 0xa4, 0x84, 0x84, 0xbe, 0x5e, 0x52, 0x5c, 0xc9, 0xf4, 0x2b, 0x33, 0x04, 0xed,
];
const V9_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0xc9, 0xbd, 0x52, 0x32, 0x68, 0xad, 0xe4, 0x99, 0xfe, 0x44, 0x66, 0x73, 0xa3, 0xba, 0xa2, 0x55,
    0x43, 0xf6, 0x26, 0x8c, 0x96, 0x3d, 0x50, 0x21, 0x12, 0xfd, 0x14, 0xa6, 0x07, 0x30, 0x06, 0x07,
];
const V10_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0xce, 0x53, 0x83, 0xf4, 0x4a, 0x3a, 0x98, 0x2a, 0xb1, 0x27, 0x60, 0x8a, 0xcd, 0xa4, 0x73, 0xd1,
    0x53, 0x1f, 0xf1, 0x0f, 0xc4, 0xb6, 0xca, 0x8e, 0x70, 0x36, 0xd8, 0x4f, 0xde, 0xc7, 0x5d, 0x8d,
];
const V11_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0xba, 0x46, 0x8e, 0xd1, 0xb3, 0x93, 0x26, 0x4b, 0x77, 0x88, 0xf2, 0xa8, 0x23, 0x32, 0x66, 0x7b,
    0x30, 0x53, 0xaa, 0x1f, 0x0f, 0xf9, 0x07, 0x4a, 0x0b, 0x14, 0x8c, 0x1a, 0xa8, 0xc8, 0x3f, 0xd7,
];
const V12_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0xee, 0xce, 0x92, 0x4b, 0x11, 0x69, 0x19, 0x50, 0xd7, 0xa7, 0x49, 0xbb, 0x30, 0xb4, 0x7e, 0xf7,
    0xc2, 0x07, 0xbc, 0x62, 0x34, 0xb3, 0x10, 0xff, 0x5a, 0x0b, 0x9a, 0x06, 0xe9, 0xfe, 0x6d, 0xe9,
];
const V13_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x2b, 0x12, 0x22, 0x2a, 0x5a, 0x0f, 0x1e, 0x7d, 0x33, 0x36, 0xac, 0x43, 0x99, 0xe8, 0x0c, 0xfa,
    0x6a, 0x09, 0x7f, 0x50, 0xbd, 0x3d, 0xe3, 0xcc, 0x14, 0x55, 0x41, 0xe4, 0x3d, 0x6f, 0xbb, 0xc1,
];
const V14_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0xce, 0x99, 0x9c, 0xba, 0x9a, 0x00, 0x7d, 0x9e, 0x91, 0xcd, 0x30, 0x3a, 0x8c, 0x63, 0x1b, 0xb0,
    0xa5, 0xfc, 0xeb, 0x58, 0x18, 0xbd, 0xa3, 0x71, 0xdc, 0x35, 0x6b, 0x51, 0x91, 0x5a, 0xbc, 0xe9,
];
const V15_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0xd9, 0xf6, 0xc0, 0xeb, 0x2a, 0x23, 0x74, 0xb2, 0x4e, 0xe4, 0x60, 0x43, 0x5c, 0xc8, 0x18, 0xf3,
    0x4c, 0x78, 0xd0, 0x8f, 0x90, 0x92, 0x51, 0x9d, 0x64, 0x6d, 0x8c, 0x55, 0x18, 0xbf, 0x07, 0x8b,
];
const V16_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x0c, 0xef, 0x17, 0xf1, 0x1e, 0xed, 0x44, 0x81, 0xd8, 0xcc, 0x97, 0xa1, 0xd4, 0x05, 0x68, 0x67,
    0x73, 0x92, 0xdf, 0x40, 0xf2, 0x45, 0x77, 0xb1, 0xdb, 0x61, 0xaf, 0x63, 0xf3, 0x9f, 0x65, 0x31,
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
    let (expected_indexes, expected_digest) = match schema.version {
        6 => (V6_IMPLICIT_INDEX_COUNT, V6_STRUCTURAL_CONTRACT_SHA256),
        7 => (V7_IMPLICIT_INDEX_COUNT, V7_STRUCTURAL_CONTRACT_SHA256),
        8 => (V8_IMPLICIT_INDEX_COUNT, V8_STRUCTURAL_CONTRACT_SHA256),
        9 => (V9_IMPLICIT_INDEX_COUNT, V9_STRUCTURAL_CONTRACT_SHA256),
        10 => (V10_IMPLICIT_INDEX_COUNT, V10_STRUCTURAL_CONTRACT_SHA256),
        11 => (V11_IMPLICIT_INDEX_COUNT, V11_STRUCTURAL_CONTRACT_SHA256),
        12 => (V12_IMPLICIT_INDEX_COUNT, V12_STRUCTURAL_CONTRACT_SHA256),
        13 => (V13_IMPLICIT_INDEX_COUNT, V13_STRUCTURAL_CONTRACT_SHA256),
        14 => (V14_IMPLICIT_INDEX_COUNT, V14_STRUCTURAL_CONTRACT_SHA256),
        15 => (V15_IMPLICIT_INDEX_COUNT, V15_STRUCTURAL_CONTRACT_SHA256),
        16 => (V16_IMPLICIT_INDEX_COUNT, V16_STRUCTURAL_CONTRACT_SHA256),
        17 => (V17_IMPLICIT_INDEX_COUNT, V17_STRUCTURAL_CONTRACT_SHA256),
        18 => (V18_IMPLICIT_INDEX_COUNT, V18_STRUCTURAL_CONTRACT_SHA256),
        19 => (V19_IMPLICIT_INDEX_COUNT, V19_STRUCTURAL_CONTRACT_SHA256),
        20 => (V20_IMPLICIT_INDEX_COUNT, V20_STRUCTURAL_CONTRACT_SHA256),
        21 => (V21_IMPLICIT_INDEX_COUNT, V21_STRUCTURAL_CONTRACT_SHA256),
        version => return Err(format!("Hub v{version} has no release structural contract")),
    };
    let implicit_indexes = schema
        .tables
        .iter()
        .map(|table| table.signature.implicit_index_count())
        .sum::<usize>();
    if implicit_indexes != expected_indexes
        || schema.catalog.implicit_index_owners.len() != expected_indexes
    {
        return Err(format!(
            "generated Hub v{} has {implicit_indexes} implicit indexes; expected {expected_indexes}",
            schema.version
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
