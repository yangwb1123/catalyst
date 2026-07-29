use rusqlite::Connection;

type ColumnSignature = (i64, String, String, i64, Option<String>, i64, i64);
type ForeignKeySignature = (i64, i64, String, String, String, String, String, String);
type IndexKeySignature = (i64, i64, Option<String>, i64, Option<String>);

#[derive(PartialEq, Eq)]
pub(super) struct TableSignature {
    columns: Vec<ColumnSignature>,
    foreign_keys: Vec<ForeignKeySignature>,
    indexes: Vec<IndexSignature>,
}

impl TableSignature {
    pub(super) fn implicit_index_count(&self) -> usize {
        self.indexes
            .iter()
            .filter(|index| index.origin != "c")
            .count()
    }

    pub(super) fn encode(&self, output: &mut Vec<u8>) {
        push_len(output, self.columns.len());
        for column in &self.columns {
            encode_column(output, column);
        }
        push_len(output, self.foreign_keys.len());
        for foreign_key in &self.foreign_keys {
            encode_foreign_key(output, foreign_key);
        }
        push_len(output, self.indexes.len());
        for index in &self.indexes {
            encode_index(output, index);
        }
    }
}

pub(super) struct TableInspection {
    pub(super) signature: TableSignature,
    pub(super) explicit_indexes: Vec<String>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct IndexSignature {
    unique: i64,
    origin: String,
    partial: i64,
    keys: Vec<IndexKeySignature>,
}

struct IndexRow {
    name: String,
    unique: i64,
    origin: String,
    partial: i64,
}

pub(super) fn inspect(connection: &Connection, table: &str) -> rusqlite::Result<TableInspection> {
    let columns = columns(connection, table)?;
    let foreign_keys = foreign_keys(connection, table)?;
    let rows = index_rows(connection, table)?;
    let explicit_indexes = explicit_indexes(&rows);
    let indexes = index_signatures(connection, rows)?;
    Ok(TableInspection {
        signature: TableSignature {
            columns,
            foreign_keys,
            indexes,
        },
        explicit_indexes,
    })
}

fn columns(connection: &Connection, table: &str) -> rusqlite::Result<Vec<ColumnSignature>> {
    let escaped = escaped(table);
    let mut statement = connection.prepare(&format!("PRAGMA main.table_xinfo('{escaped}')"))?;
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })?
        .collect()
}

fn foreign_keys(
    connection: &Connection,
    table: &str,
) -> rusqlite::Result<Vec<ForeignKeySignature>> {
    let escaped = escaped(table);
    let mut statement =
        connection.prepare(&format!("PRAGMA main.foreign_key_list('{escaped}')"))?;
    let mut signatures = statement
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
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    signatures.sort();
    Ok(signatures)
}

fn index_rows(connection: &Connection, table: &str) -> rusqlite::Result<Vec<IndexRow>> {
    let escaped = escaped(table);
    let mut statement = connection.prepare(&format!("PRAGMA main.index_list('{escaped}')"))?;
    statement
        .query_map([], |row| {
            Ok(IndexRow {
                name: row.get(1)?,
                unique: row.get(2)?,
                origin: row.get(3)?,
                partial: row.get(4)?,
            })
        })?
        .collect()
}

fn explicit_indexes(rows: &[IndexRow]) -> Vec<String> {
    let mut names = rows
        .iter()
        .filter(|row| row.origin == "c")
        .map(|row| row.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn index_signatures(
    connection: &Connection,
    rows: Vec<IndexRow>,
) -> rusqlite::Result<Vec<IndexSignature>> {
    let mut signatures = Vec::with_capacity(rows.len());
    for row in rows {
        signatures.push(IndexSignature {
            unique: row.unique,
            origin: row.origin,
            partial: row.partial,
            keys: index_keys(connection, &row.name)?,
        });
    }
    signatures.sort();
    Ok(signatures)
}

fn index_keys(connection: &Connection, index: &str) -> rusqlite::Result<Vec<IndexKeySignature>> {
    let escaped = escaped(index);
    let mut statement = connection.prepare(&format!("PRAGMA main.index_xinfo('{escaped}')"))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;
    rows.filter_map(|row| match row {
        Ok((seq, cid, name, desc, collation, 1)) => Some(Ok((seq, cid, name, desc, collation))),
        Ok(_) => None,
        Err(error) => Some(Err(error)),
    })
    .collect()
}

fn escaped(name: &str) -> String {
    name.replace('\'', "''")
}

fn encode_column(output: &mut Vec<u8>, column: &ColumnSignature) {
    push_i64(output, column.0);
    push_string(output, &column.1);
    push_string(output, &column.2);
    push_i64(output, column.3);
    push_optional_string(output, column.4.as_deref());
    push_i64(output, column.5);
    push_i64(output, column.6);
}

fn encode_foreign_key(output: &mut Vec<u8>, foreign_key: &ForeignKeySignature) {
    push_i64(output, foreign_key.0);
    push_i64(output, foreign_key.1);
    push_string(output, &foreign_key.2);
    push_string(output, &foreign_key.3);
    push_string(output, &foreign_key.4);
    push_string(output, &foreign_key.5);
    push_string(output, &foreign_key.6);
    push_string(output, &foreign_key.7);
}

fn encode_index(output: &mut Vec<u8>, index: &IndexSignature) {
    push_i64(output, index.unique);
    push_string(output, &index.origin);
    push_i64(output, index.partial);
    push_len(output, index.keys.len());
    for key in &index.keys {
        push_i64(output, key.0);
        push_i64(output, key.1);
        push_optional_string(output, key.2.as_deref());
        push_i64(output, key.3);
        push_optional_string(output, key.4.as_deref());
    }
}

pub(super) fn push_string(output: &mut Vec<u8>, value: &str) {
    push_len(output, value.len());
    output.extend_from_slice(value.as_bytes());
}

pub(super) fn push_len(output: &mut Vec<u8>, value: usize) {
    let value = u64::try_from(value).expect("schema contract length fits u64");
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_i64(output: &mut Vec<u8>, value: i64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_optional_string(output: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            output.push(1);
            push_string(output, value);
        }
        None => output.push(0),
    }
}
