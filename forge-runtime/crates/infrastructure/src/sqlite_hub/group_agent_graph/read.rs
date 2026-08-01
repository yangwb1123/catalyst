use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphInspection, GroupAgentGraphManifest,
    GroupAgentGraphRecord, GroupAgentGraphSource, GroupAgentGraphStatus, HubEntity, HubStoreError,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_LIST_LIMIT,
};

use super::{
    super::{group_run_codec, group_run_read, read_error},
    codec, rows,
};

pub(super) fn inspect(
    connection: &mut Connection,
    graph_id: &str,
) -> Result<GroupAgentGraphInspection, HubStoreError> {
    inspect_transaction_with_hook(connection, graph_id, || Ok(()))
}

fn inspect_transaction_with_hook<F>(
    connection: &mut Connection,
    graph_id: &str,
    after_graph: F,
) -> Result<GroupAgentGraphInspection, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot_with_hook(&transaction, graph_id, after_graph)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(in crate::sqlite_hub) fn inspect_in_snapshot(
    connection: &Connection,
    graph_id: &str,
) -> Result<GroupAgentGraphInspection, HubStoreError> {
    inspect_in_snapshot_with_hook(connection, graph_id, || Ok(()))
}

fn inspect_in_snapshot_with_hook<F>(
    connection: &Connection,
    graph_id: &str,
    after_graph: F,
) -> Result<GroupAgentGraphInspection, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let stored = rows::find_by_id(connection, graph_id)?
        .ok_or_else(|| not_found(HubEntity::GroupAgentGraph, graph_id))?;
    after_graph()?;
    validate_stored(connection, stored)
}

#[cfg(test)]
pub(super) fn inspect_after_graph<F>(
    connection: &mut Connection,
    graph_id: &str,
    after_graph: F,
) -> Result<GroupAgentGraphInspection, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    inspect_transaction_with_hook(connection, graph_id, after_graph)
}

pub(super) fn list(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupAgentGraphRecord>, HubStoreError> {
    validate_list_request(connection, group_run_id, limit)?;
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    rows::query_metadata(connection, group_run_id, limit)?
        .into_iter()
        .map(metadata_record)
        .collect()
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredGraph,
) -> Result<GroupAgentGraphInspection, HubStoreError> {
    validate_stored_key(&stored.idempotency_key)?;
    let record = metadata_record(stored.metadata)?;
    if stored.manifest_blob.len() != record.manifest_bytes {
        return Err(corrupt("stored graph manifest byte count disagrees"));
    }
    let digest = codec::encode_blob(&record.manifest_sha256, "manifest")
        .map_err(|error| corrupt(&error.to_string()))?;
    let (manifest, manifest_json) = codec::decode(&stored.manifest_blob, &digest)?;
    validate_manifest_inputs(connection, &manifest)?;
    let inspection = GroupAgentGraphInspection {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph: record,
        manifest,
        manifest_json,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn validate_manifest_candidate(
    connection: &Connection,
    manifest: &GroupAgentGraphManifest,
) -> Result<(), HubStoreError> {
    validate_manifest_inputs_as(connection, manifest, InputClassification::Candidate)
}

fn validate_manifest_inputs(
    connection: &Connection,
    manifest: &GroupAgentGraphManifest,
) -> Result<(), HubStoreError> {
    validate_manifest_inputs_as(connection, manifest, InputClassification::Stored)
}

fn validate_manifest_inputs_as(
    connection: &Connection,
    manifest: &GroupAgentGraphManifest,
    classification: InputClassification,
) -> Result<(), HubStoreError> {
    let stored = group_run_read::find_by_id(connection, &manifest.source.group_run_id)?
        .ok_or_else(|| classification.mismatch("graph references a missing frozen Group Run"))?;
    let snapshot = group_run_read::decode_stored(stored)?;
    if source_from_snapshot(&snapshot) != manifest.source {
        return Err(classification.mismatch("graph source does not match its frozen Group Run"));
    }
    if !members_match(&snapshot, manifest) {
        return Err(
            classification.mismatch("graph node membership does not match its frozen Group Run")
        );
    }
    Ok(())
}

fn source_from_snapshot(
    snapshot: &crate::runtime_domain::GroupRunSnapshot,
) -> GroupAgentGraphSource {
    GroupAgentGraphSource {
        group_run_version: snapshot.run.v,
        group_run_id: snapshot.run.run_id.clone(),
        group_id: snapshot.run.group_id.clone(),
        context_version: snapshot.run.context_version,
        context_slice_sha256: snapshot.run.context_slice_sha256.clone(),
        snapshot_sha256: snapshot.run.snapshot_sha256.clone(),
        snapshot_bytes: snapshot.run.snapshot_bytes,
    }
}

fn members_match(
    snapshot: &crate::runtime_domain::GroupRunSnapshot,
    manifest: &GroupAgentGraphManifest,
) -> bool {
    manifest.nodes.iter().all(|node| {
        snapshot
            .context
            .payload
            .members
            .iter()
            .any(|member| member.project_id == node.project_id && member.role == node.member_role)
    })
}

#[derive(Clone, Copy)]
enum InputClassification {
    Candidate,
    Stored,
}

impl InputClassification {
    fn mismatch(self, message: &str) -> HubStoreError {
        match self {
            Self::Candidate => conflict(message),
            Self::Stored => corrupt(message),
        }
    }
}

fn metadata_record(raw: rows::RawGraphMetadata) -> Result<GroupAgentGraphRecord, HubStoreError> {
    let record = GroupAgentGraphRecord {
        v: convert(raw.graph_version, "graph version")?,
        graph_id: raw.id,
        group_run_id: raw.group_run_id,
        status: parse_status(&raw.status)?,
        source_snapshot_sha256: codec::decode_hex(&raw.source_snapshot_sha256, "source")?,
        manifest_sha256: codec::decode_hex(&raw.manifest_sha256, "manifest")?,
        manifest_bytes: convert(raw.manifest_bytes, "manifest byte count")?,
        node_count: convert(raw.node_count, "node count")?,
        edge_count: convert(raw.edge_count, "edge count")?,
        wave_count: convert(raw.wave_count, "wave count")?,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    record
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(record)
}

fn validate_stored_key(key: &str) -> Result<(), HubStoreError> {
    if group_run_codec::valid_text(key, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES) {
        Ok(())
    } else {
        Err(corrupt("stored graph idempotency key is invalid"))
    }
}

fn validate_list_request(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_AGENT_GRAPH_LIST_LIMIT).contains(&limit) {
        return Err(conflict("graph list limit is outside its bounds"));
    }
    let Some(id) = group_run_id else {
        return Ok(());
    };
    if !group_run_codec::valid_text(id, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES) {
        return Err(conflict("Group Run filter is outside its bounds"));
    }
    connection
        .query_row("SELECT 1 FROM group_runs WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| not_found(HubEntity::GroupRun, id))
}

fn parse_status(value: &str) -> Result<GroupAgentGraphStatus, HubStoreError> {
    match value {
        "prepared" => Ok(GroupAgentGraphStatus::Prepared),
        _ => Err(corrupt("stored Group Agent Graph status is unsupported")),
    }
}

fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value).map_err(|error| corrupt(&format!("invalid graph {subject}: {error}")))
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentGraph,
        message: message.into(),
    }
}
