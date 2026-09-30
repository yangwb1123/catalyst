use std::{
    io::Cursor,
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::super::{commands::dispatch_command, state::TuiState};
use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};
const PLACEMENT_INPUT: &str = r#"{
  "schema_version": "forge.device-placement-dry-run/v1",
  "evaluated_at_ms": 1800000000000,
  "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
  "max_snapshot_age_ms": 60000,
  "requirements": {
    "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
    "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
    "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
    "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
    "sandbox_floor": "container", "concurrency_slots": 1
  },
  "devices": [{
    "device_id": "device-1",
    "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
    "approval_state": "approved", "cordon_state": "clear", "liveness": "online",
    "snapshot_observed_at_ms": 1799999999000, "lease_expires_at_ms": 1800000060000,
    "os": "linux", "architecture": "amd64", "available_cpu_cores": 2,
    "available_memory_bytes": 4096, "available_storage_bytes": 4096,
    "runtimes": ["oci"], "gpu": {"present": false, "memory_bytes": 0, "runtime": ""},
    "data_residency_zones": ["us-west"], "trust_zone": "standard",
    "sandbox_levels": ["container"], "concurrency_limit": 2, "active_concurrency": 0
  }]
}"#;

const SCHEDULER_SELECTION_INPUT: &str = r#"{
  "conversation_id": "conversation-1",
  "run_id": "run-1",
  "attempt_id": "attempt-1",
  "requirements": {
    "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
    "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
    "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
    "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
    "sandbox_floor": "container", "concurrency_slots": 1
  }
}"#;

const SCHEDULER_SELECTION_LEASE_INPUT: &str = r#"{
  "conversation_id": "conversation-1",
  "run_id": "run-1",
  "attempt_id": "attempt-1",
  "requirements": {
    "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
    "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
    "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
    "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
    "sandbox_floor": "container", "concurrency_slots": 1
  },
  "ttl_ms": 30000
}"#;

const SCHEDULER_SELECTION_LEASE_RENEWAL_INPUT: &str = r#"{
  "conversation_id": "conversation-1",
  "run_id": "run-1",
  "attempt_id": "attempt-1",
  "target_id": "runner-a",
  "epoch": 1,
  "fencing_token": "fence-token-a",
  "ttl_ms": 30000
}"#;

const SCHEDULER_SELECTION_LEASE_RELEASE_INPUT: &str = r#"{
  "conversation_id": "conversation-1",
  "run_id": "run-1",
  "attempt_id": "attempt-1",
  "target_id": "runner-a",
  "epoch": 2,
  "fencing_token": "fence-token-b"
}"#;

fn canonical_inventory_resource_pair() -> (Value, Value) {
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .expect("inventory/resource convergence fixture");
    (pair["inventory"].clone(), pair["resource_view"].clone())
}

fn visible_scheduler_lease_state(inventory: Value, resource: Value) -> TuiState {
    let mut session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .expect("client-instance session-view fixture");
    // The session and resource readers must begin with one converged local
    // display image; the request refresh below is then the boundary under
    // test.
    session["instances"] = resource["instances"].clone();
    TuiState {
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(session),
        client_instance_resource_view_observed: Some(resource),
        device_inventory_v2_observed: Some(inventory),
        ..TuiState::default()
    }
}

fn scheduler_lease_response(conversation_id: &str, epoch: u64, fencing_token: &str) -> Value {
    json!({
        "schema_version": "forge.execution-lease-registry/v1",
        "evaluation_mode": "durable_scheduler_lease_claim",
        "owner": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
        "conversation_id": conversation_id, "run_id": "run-1", "attempt_id": "attempt-1",
        "device_id": "device-a", "instance_id": "runner-a",
        "inventory_revision": 3, "generation": 2, "heartbeat_sequence": 4,
        "grant": {
            "v": 1, "attempt_id": "attempt-1", "target_id": "runner-a", "epoch": epoch,
            "fencing_token": fencing_token, "issued_at_ms": 1800000000000_i64,
            "expires_at_ms": 1800000030000_i64
        },
        "replayed": false,
        "authority": {
            "placement_selected": true, "reservation_created": true, "lease_issued": true,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    })
}

#[path = "placement/authorization.rs"]
mod authorization;
#[path = "placement/instance_visibility.rs"]
mod instance_visibility;
#[path = "placement/lease_mutations.rs"]
mod lease_mutations;
#[path = "placement/lease_refresh.rs"]
mod lease_refresh;
#[path = "placement/preview.rs"]
mod preview;
