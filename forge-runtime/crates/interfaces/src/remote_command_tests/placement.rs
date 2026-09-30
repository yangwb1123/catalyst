use super::*;

fn placement_request() -> Value {
    json!({
        "schema_version": "forge.device-placement-dry-run/v1",
        "evaluated_at_ms": 1_800_000_000_000_i64,
        "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "max_snapshot_age_ms": 60_000,
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
            "snapshot_observed_at_ms": 1799999999000_i64, "lease_expires_at_ms": 1800000060000_i64,
            "os": "linux", "architecture": "amd64", "available_cpu_cores": 2,
            "available_memory_bytes": 4096, "available_storage_bytes": 4096,
            "runtimes": ["oci"], "gpu": {"present": false, "memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "trust_zone": "standard",
            "sandbox_levels": ["container"], "concurrency_limit": 2, "active_concurrency": 0
        }]
    })
}

fn placement_response(matches: bool, reasons: Value) -> Value {
    json!({
        "schema_version": "forge.device-placement-dry-run-result/v1",
        "evaluation_mode": "offline_static_only",
        "evaluated_at_ms": 1_800_000_000_000_i64,
        "owner_declaration": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "owner_declaration_unverified": true,
        "device_attributes_unverified": true,
        "notice": "All owner, approval, liveness, resource, residency, trust, and sandbox attributes are unverified caller declarations. This offline comparison selects no target and grants no execution authority.",
        "device_results": [{
            "device_id": "device-1",
            "attributes_unverified": true,
            "matches_requirements": matches,
            "exclusion_reasons": reasons
        }],
        "execution_authorized": false,
        "reservation_created": false,
        "dispatch_performed": false
    })
}

fn registry_placement_request() -> Value {
    json!({
        "requirements": {
            "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
            "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
            "sandbox_floor": "container", "concurrency_slots": 1
        }
    })
}

fn registry_placement_response() -> Value {
    json!({
        "schema_version": "forge.device-inventory-placement-evaluation/v2",
        "evaluation_mode": "offline_static_only",
        "source_schema_version": "forge.device-inventory-observation/v2",
        "evaluation_owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
        "evaluated_at_ms": 1800000000000_i64,
        "notice": "Every owner, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only comparison selects no target and grants no execution authority.",
        "decisions": [{
            "revision": 1, "generation": 1, "heartbeat_sequence": 1,
            "device_id": "device-a", "instance_id": "runner-a", "reservation_state": "none",
            "gpu_count": 0, "available_gpu_memory_bytes": 0,
            "matches_requirements": true, "exclusion_reasons": [],
            "owner_declaration_unverified": true, "device_attributes_unverified": true
        }],
        "eligible_candidate_count": 1,
        "selected_device_id": null,
        "selected_instance_id": null,
        "authority": {
            "identity_verified": false, "heartbeat_persisted": false,
            "inventory_authoritative": false, "placement_selected": false,
            "reservation_created": false, "execution_authorized": false,
            "dispatch_performed": false
        }
    })
}

fn scheduler_selection_lease_request() -> Value {
    json!({
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
    })
}

fn scheduler_selection_lease_response() -> Value {
    json!({
        "schema_version": "forge.execution-lease-registry/v1",
        "evaluation_mode": "durable_scheduler_lease_claim",
        "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
        "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
        "device_id": "device-a", "instance_id": "runner-a",
        "inventory_revision": 7, "generation": 3, "heartbeat_sequence": 12,
        "grant": {
            "v": 1, "attempt_id": "attempt-1", "target_id": "runner-a", "epoch": 1,
            "fencing_token": "fence-token-a", "issued_at_ms": 1800000000000_i64,
            "expires_at_ms": 1800000030000_i64
        },
        "replayed": false,
        "authority": {
            "placement_selected": true, "reservation_created": true, "lease_issued": true,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    })
}

fn scheduler_selection_lease_renewal_request() -> Value {
    json!({
        "conversation_id": "conversation-1",
        "run_id": "run-1",
        "attempt_id": "attempt-1",
        "target_id": "runner-a",
        "epoch": 1,
        "fencing_token": "fence-token-a",
        "ttl_ms": 30000
    })
}

fn scheduler_selection_lease_release_request() -> Value {
    json!({
        "conversation_id": "conversation-1",
        "run_id": "run-1",
        "attempt_id": "attempt-1",
        "target_id": "runner-a",
        "epoch": 2,
        "fencing_token": "fence-token-b",
    })
}

fn scheduler_selection_lease_release_response() -> Value {
    json!({
        "schema_version": "forge.execution-lease-registry/v1",
        "evaluation_mode": "durable_scheduler_lease_release",
        "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
        "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
        "device_id": "device-a", "instance_id": "runner-a", "epoch": 2,
        "released_at_ms": 1800000000100_i64, "replayed": false,
        "authority": {
            "placement_selected": false, "reservation_created": false, "lease_issued": false,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    })
}

#[path = "placement/declared_placement_tests.rs"]
mod declared_placement_tests;
#[path = "placement/registry_tests.rs"]
mod registry_tests;
#[path = "placement/scheduler_leases_tests.rs"]
mod scheduler_leases_tests;
#[path = "placement/scheduler_selection_tests.rs"]
mod scheduler_selection_tests;
