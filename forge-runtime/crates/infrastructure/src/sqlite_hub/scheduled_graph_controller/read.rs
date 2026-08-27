use rusqlite::{Connection, OptionalExtension, params};

use crate::runtime_domain::{
    MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS, ScheduledGraphControllerEvent,
    ScheduledGraphControllerEventPayload, ScheduledGraphControllerHeader,
    ScheduledGraphControllerJournal,
};

use super::super::{group_run_codec, schema};
use super::{corrupt, not_found};
use crate::runtime_domain::HubStoreError;

const HEADER_SQL: &str = "SELECT controller_id,graph_run_id,schedule_id,version,
 controller_protocol_version,schedule_version,progress_protocol_version,schedule_sha256,
 core_bin_sha256,execution_profile_sha256,node_count,max_effectful_steps,
 max_total_cost_usd_micros,controller_sha256,header_blob,created_at_ms
 FROM group_agent_scheduled_graph_controllers WHERE graph_run_id=?1";

const EVENTS_SQL: &str = "SELECT sequence,previous_event_sha256,event_sha256,event_kind,
 effectful_step_reservation,reserved_cost_usd_micros,event_blob,created_at_ms
 FROM group_agent_scheduled_graph_controller_events
 WHERE controller_id=?1 ORDER BY sequence ASC LIMIT ?2";

pub(super) fn inspect(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<ScheduledGraphControllerJournal, HubStoreError> {
    let header = read_header(connection, graph_run_id)?;
    let events = read_events(connection, &header.controller_id)?;
    let journal = ScheduledGraphControllerJournal { header, events };
    journal
        .validate()
        .map_err(|_| corrupt("stored scheduled Graph controller journal is invalid"))?;
    Ok(journal)
}

fn read_header(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<ScheduledGraphControllerHeader, HubStoreError> {
    let row = connection
        .query_row(HEADER_SQL, [graph_run_id], |row| {
            Ok(HeaderRow {
                controller_id: row.get(0)?,
                graph_run_id: row.get(1)?,
                schedule_id: row.get(2)?,
                version: row.get(3)?,
                controller_protocol_version: row.get(4)?,
                schedule_version: row.get(5)?,
                progress_protocol_version: row.get(6)?,
                schedule_sha256: row.get(7)?,
                core_bin_sha256: row.get(8)?,
                execution_profile_sha256: row.get(9)?,
                node_count: row.get(10)?,
                max_effectful_steps: row.get(11)?,
                max_total_cost_usd_micros: row.get(12)?,
                controller_sha256: row.get(13)?,
                header_blob: row.get(14)?,
                created_at_ms: row.get(15)?,
            })
        })
        .optional()
        .map_err(schema::sqlite_error)?
        .ok_or_else(|| not_found(graph_run_id))?;
    decode_header(&row, graph_run_id)
}

fn decode_header(
    row: &HeaderRow,
    expected_graph_run_id: &str,
) -> Result<ScheduledGraphControllerHeader, HubStoreError> {
    let json = std::str::from_utf8(&row.header_blob)
        .map_err(|_| corrupt("controller header blob is not UTF-8"))?;
    let header = ScheduledGraphControllerHeader::decode_exact(json)
        .map_err(|_| corrupt("controller header blob is invalid"))?;
    let exact = row.controller_id == header.controller_id
        && row.graph_run_id == expected_graph_run_id
        && row.graph_run_id == header.graph_run_id
        && row.schedule_id == header.schedule_id
        && row.version == i64::from(header.v)
        && row.controller_protocol_version == i64::from(header.controller_protocol_version)
        && row.schedule_version == i64::from(header.schedule_version)
        && row.progress_protocol_version == i64::from(header.progress_protocol_version)
        && row.schedule_sha256 == digest(&header.schedule_sha256)?
        && row.core_bin_sha256 == digest(&header.core_bin_sha256)?
        && row.execution_profile_sha256 == digest(&header.execution_profile.profile_sha256)?
        && row.node_count == to_i64(header.node_count)?
        && row.max_effectful_steps == i64::from(header.max_effectful_steps)
        && row.max_total_cost_usd_micros == to_i64(header.max_total_cost_usd_micros)?
        && row.controller_sha256 == digest(&header.controller_sha256)?
        && row.created_at_ms == to_i64(header.created_at_ms)?;
    exact
        .then_some(header)
        .ok_or_else(|| corrupt("controller header columns disagree with canonical bytes"))
}

fn read_events(
    connection: &Connection,
    controller_id: &str,
) -> Result<Vec<ScheduledGraphControllerEvent>, HubStoreError> {
    let limit = i64::try_from(MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS + 1)
        .map_err(|_| corrupt("controller event bound is invalid"))?;
    let mut statement = connection
        .prepare(EVENTS_SQL)
        .map_err(schema::sqlite_error)?;
    let rows = statement
        .query_map(params![controller_id, limit], |row| {
            Ok(EventRow {
                sequence: row.get(0)?,
                previous_event_sha256: row.get(1)?,
                event_sha256: row.get(2)?,
                event_kind: row.get(3)?,
                effectful_step_reservation: row.get(4)?,
                reserved_cost_usd_micros: row.get(5)?,
                event_blob: row.get(6)?,
                created_at_ms: row.get(7)?,
            })
        })
        .map_err(schema::sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(schema::sqlite_error)?;
    if rows.len() > MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS {
        return Err(corrupt("controller event count exceeds its bound"));
    }
    rows.iter().map(decode_event).collect()
}

fn decode_event(row: &EventRow) -> Result<ScheduledGraphControllerEvent, HubStoreError> {
    let json = std::str::from_utf8(&row.event_blob)
        .map_err(|_| corrupt("controller event blob is not UTF-8"))?;
    let event = ScheduledGraphControllerEvent::decode_exact(json)
        .map_err(|_| corrupt("controller event blob is invalid"))?;
    let (reservation, cost) = dispatch_fields(&event.payload);
    let exact = row.sequence == to_i64(event.sequence)?
        && optional_digest(event.previous_event_sha256.as_ref())? == row.previous_event_sha256
        && row.event_sha256 == digest(&event.event_sha256)?
        && row.event_kind == event_kind(&event.payload)
        && row.effectful_step_reservation == reservation.map(i64::from)
        && row.reserved_cost_usd_micros == cost.map(to_i64).transpose()?
        && row.created_at_ms == to_i64(event.created_at_ms)?;
    exact
        .then_some(event)
        .ok_or_else(|| corrupt("controller event columns disagree with canonical bytes"))
}

pub(super) fn event_kind(value: &ScheduledGraphControllerEventPayload) -> &'static str {
    use ScheduledGraphControllerEventPayload as Payload;
    match value {
        Payload::Started { .. } => "started",
        Payload::MaterializePlanned { .. } => "materialize_planned",
        Payload::MaterializeObserved { .. } => "materialize_observed",
        Payload::PreparePlanned { .. } => "prepare_planned",
        Payload::PrepareObserved { .. } => "prepare_observed",
        Payload::AwaitingFreshConsent { .. } => "awaiting_fresh_consent",
        Payload::DispatchPlanned { .. } => "dispatch_planned",
        Payload::NodeCompleted { .. } => "node_completed",
        Payload::RetryablePreclaimFailure { .. } => "retryable_preclaim_failure",
        Payload::Stopped { .. } => "stopped",
        Payload::Completed { .. } => "completed",
    }
}

pub(super) fn dispatch_fields(
    value: &ScheduledGraphControllerEventPayload,
) -> (Option<u16>, Option<u64>) {
    match value {
        ScheduledGraphControllerEventPayload::DispatchPlanned {
            effectful_step_reservation,
            reserved_cost_usd_micros,
            ..
        } => (
            Some(*effectful_step_reservation),
            Some(*reserved_cost_usd_micros),
        ),
        _ => (None, None),
    }
}

fn optional_digest(value: Option<&String>) -> Result<Option<Vec<u8>>, HubStoreError> {
    value.map(String::as_str).map(digest).transpose()
}

pub(super) fn digest(value: &str) -> Result<Vec<u8>, HubStoreError> {
    group_run_codec::decode_hex_digest(value)
        .map(|digest| digest.to_vec())
        .ok_or_else(|| corrupt("controller digest is invalid"))
}

pub(super) fn to_i64<T>(value: T) -> Result<i64, HubStoreError>
where
    T: TryInto<i64>,
{
    value
        .try_into()
        .map_err(|_| corrupt("controller integer exceeds SQLite range"))
}

struct HeaderRow {
    controller_id: String,
    graph_run_id: String,
    schedule_id: String,
    version: i64,
    controller_protocol_version: i64,
    schedule_version: i64,
    progress_protocol_version: i64,
    schedule_sha256: Vec<u8>,
    core_bin_sha256: Vec<u8>,
    execution_profile_sha256: Vec<u8>,
    node_count: i64,
    max_effectful_steps: i64,
    max_total_cost_usd_micros: i64,
    controller_sha256: Vec<u8>,
    header_blob: Vec<u8>,
    created_at_ms: i64,
}

struct EventRow {
    sequence: i64,
    previous_event_sha256: Option<Vec<u8>>,
    event_sha256: Vec<u8>,
    event_kind: String,
    effectful_step_reservation: Option<i64>,
    reserved_cost_usd_micros: Option<i64>,
    event_blob: Vec<u8>,
    created_at_ms: i64,
}
