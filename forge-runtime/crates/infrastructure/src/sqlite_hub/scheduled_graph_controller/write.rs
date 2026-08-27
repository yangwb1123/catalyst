use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    AppendScheduledGraphControllerDisposition, AppendScheduledGraphControllerResult, HubStoreError,
    ScheduledGraphControllerEvent, ScheduledGraphControllerHeader, ScheduledGraphControllerJournal,
};

use super::super::schema;
use super::{conflict, corrupt, read};

const INSERT_HEADER_SQL: &str = "INSERT INTO group_agent_scheduled_graph_controllers(
 controller_id,graph_run_id,schedule_id,version,controller_protocol_version,schedule_version,
 progress_protocol_version,schedule_sha256,core_bin_sha256,execution_profile_sha256,node_count,
 max_effectful_steps,max_total_cost_usd_micros,controller_sha256,header_blob,created_at_ms)
 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)";

const INSERT_EVENT_SQL: &str = "INSERT INTO group_agent_scheduled_graph_controller_events(
 controller_id,sequence,previous_event_sha256,event_sha256,event_kind,
 effectful_step_reservation,reserved_cost_usd_micros,event_blob,created_at_ms)
 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)";

pub(super) fn start(
    connection: &mut Connection,
    header: &ScheduledGraphControllerHeader,
    event: &ScheduledGraphControllerEvent,
) -> Result<AppendScheduledGraphControllerResult, HubStoreError> {
    let candidate = ScheduledGraphControllerJournal {
        header: header.clone(),
        events: vec![event.clone()],
    };
    candidate
        .validate()
        .map_err(|_| conflict("scheduled Graph controller start input is invalid"))?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(schema::sqlite_error)?;
    if existing_controller_id(&transaction, &header.graph_run_id)?.is_some() {
        let stored = read::inspect(&transaction, &header.graph_run_id)?;
        let disposition = if stored.header == *header && stored.events.first() == Some(event) {
            AppendScheduledGraphControllerDisposition::Replayed
        } else {
            return Err(conflict(
                "Graph Run already has a different scheduled controller",
            ));
        };
        transaction.commit().map_err(schema::sqlite_error)?;
        return Ok(result(disposition, stored));
    }
    insert_header(&transaction, header)?;
    insert_event(&transaction, event)?;
    let stored = read::inspect(&transaction, &header.graph_run_id)?;
    transaction.commit().map_err(schema::sqlite_error)?;
    Ok(result(
        AppendScheduledGraphControllerDisposition::Stored,
        stored,
    ))
}

pub(super) fn append(
    connection: &mut Connection,
    event: &ScheduledGraphControllerEvent,
) -> Result<AppendScheduledGraphControllerResult, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(schema::sqlite_error)?;
    let mut stored = read::inspect(&transaction, &event.graph_run_id)?;
    if let Some(existing) = stored
        .events
        .iter()
        .find(|existing| existing.sequence == event.sequence)
    {
        if existing != event {
            return Err(conflict(
                "controller event sequence was concurrently consumed",
            ));
        }
        transaction.commit().map_err(schema::sqlite_error)?;
        return Ok(result(
            AppendScheduledGraphControllerDisposition::Replayed,
            stored,
        ));
    }
    stored.events.push(event.clone());
    stored
        .validate()
        .map_err(|_| conflict("scheduled Graph controller append input is invalid"))?;
    insert_event(&transaction, event)?;
    let committed = read::inspect(&transaction, &event.graph_run_id)?;
    transaction.commit().map_err(schema::sqlite_error)?;
    Ok(result(
        AppendScheduledGraphControllerDisposition::Stored,
        committed,
    ))
}

fn existing_controller_id(
    transaction: &Transaction<'_>,
    graph_run_id: &str,
) -> Result<Option<String>, HubStoreError> {
    transaction
        .query_row(
            "SELECT controller_id FROM group_agent_scheduled_graph_controllers WHERE graph_run_id=?1",
            [graph_run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(schema::sqlite_error)
}

fn insert_header(
    transaction: &Transaction<'_>,
    header: &ScheduledGraphControllerHeader,
) -> Result<(), HubStoreError> {
    let json = header
        .canonical_json()
        .map_err(|_| conflict("controller header cannot be encoded"))?;
    let changed = transaction
        .execute(
            INSERT_HEADER_SQL,
            params![
                header.controller_id,
                header.graph_run_id,
                header.schedule_id,
                i64::from(header.v),
                i64::from(header.controller_protocol_version),
                i64::from(header.schedule_version),
                i64::from(header.progress_protocol_version),
                read::digest(&header.schedule_sha256)?,
                read::digest(&header.core_bin_sha256)?,
                read::digest(&header.execution_profile.profile_sha256)?,
                read::to_i64(header.node_count)?,
                i64::from(header.max_effectful_steps),
                read::to_i64(header.max_total_cost_usd_micros)?,
                read::digest(&header.controller_sha256)?,
                json.as_bytes(),
                read::to_i64(header.created_at_ms)?,
            ],
        )
        .map_err(schema::sqlite_error)?;
    if changed != 1 {
        return Err(corrupt(
            "controller header insert changed an invalid row count",
        ));
    }
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    event: &ScheduledGraphControllerEvent,
) -> Result<(), HubStoreError> {
    let json = event
        .canonical_json()
        .map_err(|_| conflict("controller event cannot be encoded"))?;
    let (reservation, cost) = read::dispatch_fields(&event.payload);
    let changed = transaction
        .execute(
            INSERT_EVENT_SQL,
            params![
                event.controller_id,
                read::to_i64(event.sequence)?,
                event
                    .previous_event_sha256
                    .as_deref()
                    .map(read::digest)
                    .transpose()?,
                read::digest(&event.event_sha256)?,
                read::event_kind(&event.payload),
                reservation.map(i64::from),
                cost.map(read::to_i64).transpose()?,
                json.as_bytes(),
                read::to_i64(event.created_at_ms)?,
            ],
        )
        .map_err(schema::sqlite_error)?;
    if changed != 1 {
        return Err(corrupt(
            "controller event insert changed an invalid row count",
        ));
    }
    Ok(())
}

fn result(
    disposition: AppendScheduledGraphControllerDisposition,
    journal: ScheduledGraphControllerJournal,
) -> AppendScheduledGraphControllerResult {
    AppendScheduledGraphControllerResult {
        disposition,
        journal,
    }
}
