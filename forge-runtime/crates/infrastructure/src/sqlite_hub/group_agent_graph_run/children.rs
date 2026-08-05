use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    super::{
        group_agent_scheduled_node_contract, group_agent_scheduled_node_lifecycle,
        group_agent_scheduled_node_provider_request, read_error,
    },
    schedule,
};

const V11_OWNED_CHILD_QUERY: &str = "SELECT EXISTS(
       SELECT graph_run_id FROM group_agent_graph_run_events WHERE graph_run_id=?1
       UNION ALL
       SELECT graph_run_id FROM group_agent_graph_node_execution_contracts
         WHERE graph_run_id=?1
       UNION ALL
       SELECT graph_run_id FROM group_agent_graph_node_dispatch_requests
         WHERE graph_run_id=?1
     )";

const V12_OWNED_CHILD_QUERY: &str = "SELECT EXISTS(
       SELECT graph_run_id FROM group_agent_graph_run_events WHERE graph_run_id=?1
       UNION ALL
       SELECT graph_run_id FROM group_agent_graph_node_execution_contracts
         WHERE graph_run_id=?1
       UNION ALL
       SELECT graph_run_id FROM group_agent_graph_node_dispatch_requests
         WHERE graph_run_id=?1
       UNION ALL
       SELECT graph_run_id FROM group_agent_graph_node_dispatch_claims
         WHERE graph_run_id=?1
       UNION ALL
       SELECT graph_run_id FROM group_agent_project_lane_ownerships
         WHERE graph_run_id=?1
       UNION ALL
       SELECT graph_run_id FROM group_agent_graph_node_terminal_artifacts
         WHERE graph_run_id=?1
       UNION ALL
       SELECT graph_run_id FROM group_agent_graph_node_terminal_receipts
         WHERE graph_run_id=?1
     )";

pub(super) fn has_owned_child(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<bool, HubStoreError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(read_error)?;
    let query = if version == 11 {
        V11_OWNED_CHILD_QUERY
    } else {
        V12_OWNED_CHILD_QUERY
    };
    let owned = connection
        .query_row(query, [graph_run_id], |row| row.get(0))
        .map_err(read_error)?;
    if owned || version < 13 {
        return Ok(owned);
    }
    if schedule::read::has_graph_run_child(connection, graph_run_id)? {
        return Ok(true);
    }
    if version < 14 {
        return Ok(false);
    }
    if group_agent_scheduled_node_contract::read::has_graph_run_child(connection, graph_run_id)? {
        return Ok(true);
    }
    if version < 15 {
        return Ok(false);
    }
    if group_agent_scheduled_node_provider_request::read::has_graph_run_child(
        connection,
        graph_run_id,
    )? {
        return Ok(true);
    }
    if version < 16 {
        return Ok(false);
    }
    group_agent_scheduled_node_lifecycle::has_graph_run_child(connection, graph_run_id)
}
