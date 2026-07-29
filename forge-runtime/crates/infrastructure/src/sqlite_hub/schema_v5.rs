use std::sync::OnceLock;

use rusqlite::{Connection, OptionalExtension};

use super::MIGRATE_V4_TO_V5_SQL;

struct ColumnSpec {
    name: &'static str,
    data_type: &'static str,
    primary_key: i64,
}

impl ColumnSpec {
    const fn new(name: &'static str, data_type: &'static str, primary_key: i64) -> Self {
        Self {
            name,
            data_type,
            primary_key,
        }
    }
}

struct IndexColumnSpec {
    name: &'static str,
    descending: i64,
}

struct IndexSpec {
    name: &'static str,
    columns: &'static [IndexColumnSpec],
}

type Column = (String, String, i64, Option<String>, i64);
type ForeignKey = (i64, i64, String, String, String, String, String, String);
type IndexEntry = (i64, String, i64, String, i64);
type IndexColumn = (Option<String>, i64, Option<String>);

const ANALYSES: &[ColumnSpec] = &[
    ColumnSpec::new("id", "TEXT", 1),
    ColumnSpec::new("group_run_id", "TEXT", 0),
    ColumnSpec::new("analysis_version", "INTEGER", 0),
    ColumnSpec::new("status", "TEXT", 0),
    ColumnSpec::new("source_snapshot_sha256", "BLOB", 0),
    ColumnSpec::new("provider", "TEXT", 0),
    ColumnSpec::new("endpoint", "TEXT", 0),
    ColumnSpec::new("model", "TEXT", 0),
    ColumnSpec::new("system_prompt_version", "INTEGER", 0),
    ColumnSpec::new("system_prompt_sha256", "BLOB", 0),
    ColumnSpec::new("max_output_tokens", "INTEGER", 0),
    ColumnSpec::new("max_model_output_bytes", "INTEGER", 0),
    ColumnSpec::new("max_model_events", "INTEGER", 0),
    ColumnSpec::new("config_json", "TEXT", 0),
    ColumnSpec::new("config_sha256", "BLOB", 0),
    ColumnSpec::new("request_body", "BLOB", 0),
    ColumnSpec::new("request_bytes", "INTEGER", 0),
    ColumnSpec::new("request_sha256", "BLOB", 0),
    ColumnSpec::new("cursor_json", "TEXT", 0),
    ColumnSpec::new("journal_bytes", "INTEGER", 0),
    ColumnSpec::new("idempotency_key", "TEXT", 0),
    ColumnSpec::new("protocol_version", "INTEGER", 0),
    ColumnSpec::new("created_at_ms", "INTEGER", 0),
];
const EVENTS: &[ColumnSpec] = &[
    ColumnSpec::new("analysis_id", "TEXT", 1),
    ColumnSpec::new("seq", "INTEGER", 2),
    ColumnSpec::new("event_json", "TEXT", 0),
    ColumnSpec::new("event_sha256", "BLOB", 0),
];
const RESULTS: &[ColumnSpec] = &[
    ColumnSpec::new("analysis_id", "TEXT", 1),
    ColumnSpec::new("result_version", "INTEGER", 0),
    ColumnSpec::new("result_blob", "BLOB", 0),
    ColumnSpec::new("result_bytes", "INTEGER", 0),
    ColumnSpec::new("result_sha256", "BLOB", 0),
    ColumnSpec::new("created_at_ms", "INTEGER", 0),
];

const GROUP_RUN_INDEX: &[IndexColumnSpec] = &[
    IndexColumnSpec {
        name: "group_run_id",
        descending: 0,
    },
    IndexColumnSpec {
        name: "created_at_ms",
        descending: 1,
    },
    IndexColumnSpec {
        name: "id",
        descending: 1,
    },
];
const CREATED_INDEX: &[IndexColumnSpec] = &[
    IndexColumnSpec {
        name: "created_at_ms",
        descending: 1,
    },
    IndexColumnSpec {
        name: "id",
        descending: 1,
    },
];
const INDEXES: &[IndexSpec] = &[
    IndexSpec {
        name: "group_model_analyses_group_run",
        columns: GROUP_RUN_INDEX,
    },
    IndexSpec {
        name: "group_model_analyses_created",
        columns: CREATED_INDEX,
    },
];
const DEFINITIONS: &[(&str, &str)] = &[
    ("table", "group_model_analyses"),
    ("table", "group_model_analysis_events"),
    ("table", "group_model_analysis_results"),
    ("index", "group_model_analyses_group_run"),
    ("index", "group_model_analyses_created"),
];
static EXPECTED_DEFINITIONS: OnceLock<Result<Vec<String>, String>> = OnceLock::new();

pub(super) fn validate(connection: &Connection) -> Result<(), super::HubStoreError> {
    for (table, columns, unique_columns, foreign_key) in [
        (
            "group_model_analyses",
            ANALYSES,
            &["idempotency_key"][..],
            foreign_key("group_runs", "group_run_id"),
        ),
        (
            "group_model_analysis_events",
            EVENTS,
            &[][..],
            foreign_key("group_model_analyses", "analysis_id"),
        ),
        (
            "group_model_analysis_results",
            RESULTS,
            &[][..],
            foreign_key("group_model_analyses", "analysis_id"),
        ),
    ] {
        validate_columns(connection, table, columns)?;
        validate_unique_constraints(connection, table, unique_columns)?;
        validate_foreign_key(connection, table, &foreign_key)?;
    }
    for index in INDEXES {
        validate_index(connection, "group_model_analyses", index)?;
    }
    validate_explicit_index_inventory(connection)?;
    validate_no_triggers(connection)?;
    validate_definitions(connection)
}

fn foreign_key(parent: &'static str, source: &'static str) -> ForeignKey {
    (
        0,
        0,
        parent.into(),
        source.into(),
        "id".into(),
        "NO ACTION".into(),
        "RESTRICT".into(),
        "NONE".into(),
    )
}

fn validate_columns(
    connection: &Connection,
    table: &str,
    expected: &[ColumnSpec],
) -> Result<(), super::HubStoreError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA main.table_info('{table}')"))
        .map_err(unavailable)?;
    let actual: Vec<Column> = statement
        .query_map([], |row| {
            Ok((
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .map_err(unavailable)?
        .collect::<Result<_, _>>()
        .map_err(unavailable)?;
    let valid = actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.0 == expected.name
                && actual.1 == expected.data_type
                && actual.2 == 1
                && actual.3.is_none()
                && actual.4 == expected.primary_key
        });
    valid.then_some(()).ok_or_else(|| invalid(table, "columns"))
}

fn validate_unique_constraints(
    connection: &Connection,
    table: &str,
    expected: &[&str],
) -> Result<(), super::HubStoreError> {
    let names = indexes(connection, table)?
        .into_iter()
        .filter(|entry| entry.2 == 1 && entry.3 == "u" && entry.4 == 0)
        .map(|entry| entry.1)
        .collect::<Vec<_>>();
    let mut actual = names
        .iter()
        .map(|name| index_column_names(connection, name))
        .collect::<Result<Vec<_>, _>>()?;
    actual.sort();
    let mut expected = expected
        .iter()
        .map(|column| vec![(*column).to_owned()])
        .collect::<Vec<_>>();
    expected.sort();
    (actual == expected)
        .then_some(())
        .ok_or_else(|| invalid(table, "unique constraints"))
}

fn index_column_names(
    connection: &Connection,
    index: &str,
) -> Result<Vec<String>, super::HubStoreError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA main.index_info('{index}')"))
        .map_err(unavailable)?;
    statement
        .query_map([], |row| row.get(2))
        .map_err(unavailable)?
        .collect::<Result<_, _>>()
        .map_err(unavailable)
}

fn validate_foreign_key(
    connection: &Connection,
    table: &str,
    expected: &ForeignKey,
) -> Result<(), super::HubStoreError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA main.foreign_key_list('{table}')"))
        .map_err(unavailable)?;
    let actual: Vec<ForeignKey> = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })
        .map_err(unavailable)?
        .collect::<Result<_, _>>()
        .map_err(unavailable)?;
    (actual.len() == 1 && actual.first() == Some(expected))
        .then_some(())
        .ok_or_else(|| invalid(table, "foreign key"))
}

fn validate_index(
    connection: &Connection,
    table: &str,
    expected: &IndexSpec,
) -> Result<(), super::HubStoreError> {
    let flags = indexes(connection, table)?
        .into_iter()
        .find(|entry| entry.1 == expected.name)
        .map(|entry| (entry.2, entry.3, entry.4));
    if flags != Some((0, "c".into(), 0)) {
        return Err(invalid(expected.name, "index flags"));
    }
    let actual = index_columns(connection, expected.name)?;
    let valid = actual.len() == expected.columns.len()
        && actual
            .iter()
            .zip(expected.columns)
            .all(|(actual, expected)| {
                actual.0.as_deref() == Some(expected.name)
                    && actual.1 == expected.descending
                    && actual.2.as_deref() == Some("BINARY")
            });
    valid
        .then_some(())
        .ok_or_else(|| invalid(expected.name, "index columns"))
}

fn index_columns(
    connection: &Connection,
    index: &str,
) -> Result<Vec<IndexColumn>, super::HubStoreError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA main.index_xinfo('{index}')"))
        .map_err(unavailable)?;
    let columns = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    Ok(columns
        .into_iter()
        .filter(|column| column.4 == 1)
        .map(|column| (column.1, column.2, column.3))
        .collect())
}

fn validate_explicit_index_inventory(connection: &Connection) -> Result<(), super::HubStoreError> {
    for (table, expected) in [
        (
            "group_model_analyses",
            &[
                "group_model_analyses_created",
                "group_model_analyses_group_run",
            ][..],
        ),
        ("group_model_analysis_events", &[][..]),
        ("group_model_analysis_results", &[][..]),
    ] {
        let mut actual = indexes(connection, table)?
            .into_iter()
            .filter(|entry| entry.3 == "c")
            .map(|entry| entry.1)
            .collect::<Vec<_>>();
        actual.sort();
        if actual != expected {
            return Err(invalid(table, "explicit index inventory"));
        }
    }
    Ok(())
}

fn indexes(connection: &Connection, table: &str) -> Result<Vec<IndexEntry>, super::HubStoreError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA main.index_list('{table}')"))
        .map_err(unavailable)?;
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .map_err(unavailable)?
        .collect::<Result<_, _>>()
        .map_err(unavailable)
}

fn validate_no_triggers(connection: &Connection) -> Result<(), super::HubStoreError> {
    for table in [
        "group_model_analyses",
        "group_model_analysis_events",
        "group_model_analysis_results",
    ] {
        let trigger = connection
            .query_row(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'trigger' AND tbl_name = ?1 COLLATE NOCASE LIMIT 1",
                [table],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(unavailable)?;
        if trigger.is_some() {
            return Err(invalid(table, "trigger inventory"));
        }
    }
    Ok(())
}

fn validate_definitions(connection: &Connection) -> Result<(), super::HubStoreError> {
    for (&(kind, name), expected) in DEFINITIONS.iter().zip(expected_definitions()?) {
        let actual = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = ?1 AND name = ?2",
                [kind, name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(unavailable)?;
        let valid = actual.as_deref() == Some(expected);
        if !valid {
            return Err(invalid(name, "definition"));
        }
    }
    Ok(())
}

fn expected_definitions() -> Result<&'static [String], super::HubStoreError> {
    match EXPECTED_DEFINITIONS.get_or_init(load_expected_definitions) {
        Ok(definitions) => Ok(definitions),
        Err(message) => Err(unavailable(message)),
    }
}

fn load_expected_definitions() -> Result<Vec<String>, String> {
    let connection = Connection::open_in_memory().map_err(|error| error.to_string())?;
    connection
        .execute_batch(MIGRATE_V4_TO_V5_SQL)
        .map_err(|error| error.to_string())?;
    DEFINITIONS
        .iter()
        .map(|&(kind, name)| {
            connection
                .query_row(
                    "SELECT sql FROM sqlite_schema WHERE type = ?1 AND name = ?2",
                    [kind, name],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn invalid(object: &str, detail: &str) -> super::HubStoreError {
    super::HubStoreError::Corrupt {
        message: format!("Hub v5 {object} has invalid {detail}"),
    }
}

fn unavailable(error: impl std::fmt::Display) -> super::HubStoreError {
    super::HubStoreError::Unavailable {
        message: error.to_string(),
    }
}
