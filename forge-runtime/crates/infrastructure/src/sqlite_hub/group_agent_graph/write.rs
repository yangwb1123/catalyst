use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphInspection, GroupAgentGraphRecord,
    GroupAgentGraphStatus, HubEntity, HubStoreError, PrepareGroupAgentGraph,
    PrepareGroupAgentGraphDisposition, PrepareGroupAgentGraphResult,
};

use super::{
    super::{read_error, write_error},
    codec, read, rows,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn prepare(
    connection: &mut Connection,
    request: &PrepareGroupAgentGraph,
) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let result = prepare_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn prepare_locked(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentGraph,
) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        let inspection = read::validate_stored(transaction, stored)?;
        ensure_replay(&inspection, request)?;
        return Ok(result(
            PrepareGroupAgentGraphDisposition::Replayed,
            inspection,
        ));
    }
    if let Some(stored) = rows::find_by_id(transaction, &request.graph_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "graph ID already belongs to another idempotency key",
        ));
    }
    create(transaction, request)
}

fn create(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentGraph,
) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
    read::validate_manifest_candidate(transaction, &request.manifest)?;
    let encoded = codec::encode_candidate(request)?;
    let record = record(request, &encoded);
    insert(transaction, request, &record, &encoded)?;
    let inspection = reread_created(transaction, request, &record)?;
    Ok(result(
        PrepareGroupAgentGraphDisposition::Created,
        inspection,
    ))
}

fn reread_created(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentGraph,
    expected: &GroupAgentGraphRecord,
) -> Result<GroupAgentGraphInspection, HubStoreError> {
    let inspection = read::inspect_in_snapshot(transaction, &expected.graph_id)?;
    let matches = inspection.graph == *expected
        && inspection.manifest == request.manifest
        && inspection.manifest_json == request.manifest_json;
    matches
        .then_some(inspection)
        .ok_or_else(|| corrupt("created graph disagrees with its committed input"))
}

fn record(
    request: &PrepareGroupAgentGraph,
    encoded: &codec::EncodedManifest,
) -> GroupAgentGraphRecord {
    GroupAgentGraphRecord {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: request.graph_id.clone(),
        group_run_id: request.manifest.source.group_run_id.clone(),
        status: GroupAgentGraphStatus::Prepared,
        source_snapshot_sha256: request.manifest.source.snapshot_sha256.clone(),
        manifest_sha256: super::super::group_run_codec::encode_hex_digest(&encoded.digest),
        manifest_bytes: encoded.bytes.len(),
        node_count: request.manifest.nodes.len(),
        edge_count: request.manifest.edges.len(),
        wave_count: request.manifest.waves.len(),
        created_at_ms: request.created_at_ms,
    }
}

fn insert(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentGraph,
    record: &GroupAgentGraphRecord,
    encoded: &codec::EncodedManifest,
) -> Result<(), HubStoreError> {
    let source = codec::encode_blob(&record.source_snapshot_sha256, "source")?;
    transaction
        .execute(
            "INSERT INTO group_agent_graphs(
               id,group_run_id,graph_version,status,source_snapshot_sha256,
               manifest_blob,manifest_bytes,manifest_sha256,node_count,edge_count,
               wave_count,idempotency_key,created_at_ms
             ) VALUES(?1,?2,?3,'prepared',?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                record.graph_id,
                record.group_run_id,
                i64::from(record.v),
                source.as_slice(),
                encoded.bytes.as_slice(),
                to_i64(record.manifest_bytes, "manifest byte count")?,
                encoded.digest.as_slice(),
                to_i64(record.node_count, "node count")?,
                to_i64(record.edge_count, "edge count")?,
                to_i64(record.wave_count, "wave count")?,
                request.idempotency_key,
                to_i64(record.created_at_ms, "creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentGraph, error))?;
    Ok(())
}

fn ensure_replay(
    inspection: &GroupAgentGraphInspection,
    request: &PrepareGroupAgentGraph,
) -> Result<(), HubStoreError> {
    let exact = inspection.manifest == request.manifest
        && inspection.manifest_json == request.manifest_json
        && inspection.graph.manifest_sha256 == request.manifest_sha256;
    exact
        .then_some(())
        .ok_or_else(|| conflict("idempotency key was reused with different graph input"))
}

fn result(
    disposition: PrepareGroupAgentGraphDisposition,
    inspection: GroupAgentGraphInspection,
) -> PrepareGroupAgentGraphResult {
    PrepareGroupAgentGraphResult {
        v: GROUP_AGENT_GRAPH_VERSION,
        disposition,
        inspection,
    }
}

fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value).map_err(|error| conflict(&format!("invalid {subject}: {error}")))
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
