use rusqlite::{Connection, OptionalExtension};

const ANALYSES: &[&str] = &[
    "id",
    "group_run_id",
    "analysis_version",
    "status",
    "source_snapshot_sha256",
    "provider",
    "endpoint",
    "model",
    "system_prompt_version",
    "system_prompt_sha256",
    "max_output_tokens",
    "max_model_output_bytes",
    "max_model_events",
    "config_json",
    "config_sha256",
    "request_body",
    "request_bytes",
    "request_sha256",
    "cursor_json",
    "journal_bytes",
    "idempotency_key",
    "protocol_version",
    "created_at_ms",
];
const EVENTS: &[&str] = &["analysis_id", "seq", "event_json", "event_sha256"];
const RESULTS: &[&str] = &[
    "analysis_id",
    "result_version",
    "result_blob",
    "result_bytes",
    "result_sha256",
    "created_at_ms",
];

pub(super) fn validate(connection: &Connection) -> Result<(), super::HubStoreError> {
    for (table, columns) in [
        ("group_model_analyses", ANALYSES),
        ("group_model_analysis_events", EVENTS),
        ("group_model_analysis_results", RESULTS),
    ] {
        if table_columns(connection, table)? != columns {
            return Err(corrupt(&format!(
                "Hub v5 table {table} has an invalid shape"
            )));
        }
    }
    for index in [
        "group_model_analyses_group_run",
        "group_model_analyses_created",
    ] {
        if !object_exists(connection, "index", index)? {
            return Err(corrupt(&format!("Hub v5 index {index} is missing")));
        }
    }
    validate_foreign_keys(connection)
}

fn table_columns(
    connection: &Connection,
    table: &str,
) -> Result<Vec<String>, super::HubStoreError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT name FROM pragma_table_info('{table}') ORDER BY cid"
        ))
        .map_err(unavailable)?;
    statement
        .query_map([], |row| row.get(0))
        .map_err(unavailable)?
        .collect::<Result<_, _>>()
        .map_err(unavailable)
}

fn object_exists(
    connection: &Connection,
    kind: &str,
    name: &str,
) -> Result<bool, super::HubStoreError> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2",
            [kind, name],
            |_| Ok(true),
        )
        .optional()
        .map(|value| value.unwrap_or(false))
        .map_err(unavailable)
}

fn validate_foreign_keys(connection: &Connection) -> Result<(), super::HubStoreError> {
    for (table, parent) in [
        ("group_model_analyses", "group_runs"),
        ("group_model_analysis_events", "group_model_analyses"),
        ("group_model_analysis_results", "group_model_analyses"),
    ] {
        let actual = connection
            .query_row(
                &format!("SELECT \"table\" FROM pragma_foreign_key_list('{table}')"),
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(unavailable)?;
        if actual.as_deref() != Some(parent) {
            return Err(corrupt(&format!(
                "Hub v5 table {table} has an invalid foreign key"
            )));
        }
    }
    Ok(())
}

fn corrupt(message: &str) -> super::HubStoreError {
    super::HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn unavailable(error: impl std::fmt::Display) -> super::HubStoreError {
    super::HubStoreError::Unavailable {
        message: error.to_string(),
    }
}
