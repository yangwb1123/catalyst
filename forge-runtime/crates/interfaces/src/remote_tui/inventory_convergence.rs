use serde_json::Value;

/// Compares the owner, identity, lifecycle counters, and shared resource
/// fields between lossless v2 inventory and the composed
/// client-instance/resource view. Both responses have already passed their
/// individual strict decoders; this extra join prevents one TUI refresh from
/// displaying two different resource images.
pub(super) fn observations_converged(v2: &Value, resource: &Value) -> bool {
    if !declarations_converged(v2, resource) {
        return false;
    }
    let Some(v2_devices) = v2.get("devices").and_then(Value::as_array) else {
        return false;
    };
    let Some(resource_devices) = resource.get("devices").and_then(Value::as_array) else {
        return false;
    };
    if v2_devices.len() != resource_devices.len() {
        return false;
    }
    v2_devices.iter().all(|candidate| {
        let Some(device) = candidate.get("device") else {
            return false;
        };
        let Some(device_id) = device.get("device_id").and_then(Value::as_str) else {
            return false;
        };
        let Some(resource_device) = resource_devices
            .iter()
            .find(|row| row.get("device_id").and_then(Value::as_str) == Some(device_id))
        else {
            return false;
        };
        candidate.get("instance_id") == resource_device.get("runner_instance_id")
            && candidate.get("revision") == resource_device.get("revision")
            && candidate.get("generation") == resource_device.get("generation")
            && candidate.get("heartbeat_sequence") == resource_device.get("heartbeat_sequence")
            && device.get("owner") == resource_device.get("owner")
            && device.get("approval_state") == resource_device.get("approval_state")
            && device.get("cordon_state") == resource_device.get("cordon_state")
            && device.get("reservation_state") == resource_device.get("reservation_state")
            && device.get("liveness") == resource_device.get("liveness")
            && device.get("snapshot_observed_at_ms") == resource_device.get("observed_at_ms")
            && device.get("os") == resource_device.get("os")
            && device.get("architecture") == resource_device.get("architecture")
            && device.get("available_cpu_cores") == resource_device.get("available_cpu_cores")
            && device.get("available_memory_bytes") == resource_device.get("available_memory_bytes")
            && device.get("available_storage_bytes")
                == resource_device.get("available_storage_bytes")
            && gpu_summary_converged(device, resource_device)
    })
}

fn gpu_summary_converged(device: &Value, resource_device: &Value) -> bool {
    let Some(gpus) = device.get("gpus").and_then(Value::as_array) else {
        return false;
    };
    let Some(gpu_count) = resource_device.get("gpu_count").and_then(Value::as_u64) else {
        return false;
    };
    if gpu_count != gpus.len() as u64 {
        return false;
    }
    let Some(available_gpu_memory) = gpus.iter().try_fold(0_u64, |total, gpu| {
        total.checked_add(gpu.get("available_memory_bytes")?.as_u64()?)
    }) else {
        return false;
    };
    resource_device
        .get("available_gpu_memory_bytes")
        .and_then(Value::as_u64)
        == Some(available_gpu_memory)
}

fn declarations_converged(v2: &Value, resource: &Value) -> bool {
    if v2.get("owner_declaration") != resource.get("owner_declaration")
        || v2
            .get("owner_declaration_unverified")
            .and_then(Value::as_bool)
            != Some(true)
        || resource
            .get("owner_declaration_unverified")
            .and_then(Value::as_bool)
            != Some(true)
        || v2
            .get("inventory_declarations_unverified")
            .and_then(Value::as_bool)
            != Some(true)
        || resource
            .get("device_attributes_unverified")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::observations_converged;

    fn pair(
        revision: u64,
        generation: u64,
        heartbeat: u64,
    ) -> (serde_json::Value, serde_json::Value) {
        (
            inventory_observation(revision, generation, heartbeat),
            resource_observation(revision, generation, heartbeat),
        )
    }

    fn inventory_observation(revision: u64, generation: u64, heartbeat: u64) -> serde_json::Value {
        json!({
            "owner_declaration": {
                "issuer": "https://id.example",
                "subject": "user-1",
                "tenant_id": "tenant-1"
            },
            "owner_declaration_unverified": true,
            "inventory_declarations_unverified": true,
            "devices": [{
                "instance_id": "runner-a",
                "revision": revision,
                "generation": generation,
                "heartbeat_sequence": heartbeat,
                "device": {
                    "device_id": "device-a",
                    "owner": {
                        "issuer": "https://id.example",
                        "subject": "user-1",
                        "tenant_id": "tenant-1"
                    },
                    "approval_state": "approved",
                    "cordon_state": "clear",
                    "reservation_state": "none",
                    "liveness": "online",
                    "snapshot_observed_at_ms": 150000,
                    "lease_expires_at_ms": 210000,
                    "os": "linux",
                    "architecture": "amd64",
                    "available_cpu_cores": 8,
                    "available_memory_bytes": 16384,
                    "available_storage_bytes": 8192,
                    "gpus": []
                }
            }]
        })
    }

    fn resource_observation(revision: u64, generation: u64, heartbeat: u64) -> serde_json::Value {
        json!({
            "owner_declaration": {
                "issuer": "https://id.example",
                "subject": "user-1",
                "tenant_id": "tenant-1"
            },
            "owner_declaration_unverified": true,
            "device_attributes_unverified": true,
            "devices": [{
                "device_id": "device-a",
                "runner_instance_id": "runner-a",
                "revision": revision,
                "generation": generation,
                "heartbeat_sequence": heartbeat,
                "observed_at_ms": 150000,
                "owner": {
                    "issuer": "https://id.example",
                    "subject": "user-1",
                    "tenant_id": "tenant-1"
                },
                "approval_state": "approved",
                "cordon_state": "clear",
                "reservation_state": "none",
                "liveness": "online",
                "os": "linux",
                "architecture": "amd64",
                "available_cpu_cores": 8,
                "available_memory_bytes": 16384,
                "available_storage_bytes": 8192,
                "gpu_count": 0,
                "available_gpu_memory_bytes": 0
            }]
        })
    }

    #[test]
    fn matching_counters_converge() {
        let (v2, resource) = pair(2, 1, 2);
        assert!(observations_converged(&v2, &resource));
    }

    #[test]
    fn mismatched_counters_fail_closed() {
        let (v2, mut resource) = pair(2, 1, 2);
        resource["devices"][0]["heartbeat_sequence"] = json!(3);
        assert!(!observations_converged(&v2, &resource));
    }

    #[test]
    fn mismatched_resource_fields_fail_closed() {
        let (v2, mut resource) = pair(2, 1, 2);
        resource["devices"][0]["available_memory_bytes"] = json!(11);
        assert!(!observations_converged(&v2, &resource));
    }

    #[test]
    fn mismatched_observation_time_fails_closed() {
        let (v2, mut resource) = pair(2, 1, 2);
        resource["devices"][0]["observed_at_ms"] = json!(150001);
        assert!(!observations_converged(&v2, &resource));
    }

    #[test]
    fn mismatched_owner_declaration_fail_closed() {
        let (v2, mut resource) = pair(2, 1, 2);
        resource["owner_declaration"]["subject"] = json!("foreign-user");
        assert!(!observations_converged(&v2, &resource));
    }
}
