use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[path = "inventory/convergence.rs"]
mod convergence;
#[path = "inventory/execution_evidence.rs"]
mod execution_evidence;
#[path = "inventory/inventory_reads.rs"]
mod inventory_reads;
#[path = "inventory/offline_execution.rs"]
mod offline_execution;
#[path = "inventory/offline_instance_views.rs"]
mod offline_instance_views;
#[path = "inventory/offline_inventory.rs"]
mod offline_inventory;
#[path = "inventory/offline_placement.rs"]
mod offline_placement;
#[path = "inventory/persisted_observation.rs"]
mod persisted_observation;
#[path = "inventory/refresh.rs"]
mod refresh;
#[path = "inventory/runner_receipts.rs"]
mod runner_receipts;
#[path = "inventory/session_observation.rs"]
mod session_observation;
