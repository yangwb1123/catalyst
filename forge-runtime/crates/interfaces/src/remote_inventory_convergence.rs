//! Owner-scoped inventory/resource convergence for an explicit CLI read.
//!
//! The two source observations are already decoded by their individual
//! strict consumers. This module joins their owner, device identity,
//! lifecycle counters, and shared resource fields; it never turns the
//! declarations into inventory, placement, lease, or execution authority.

use serde_json::{Value, json};

use crate::remote_command::RemoteError;

pub(crate) const SCHEMA_VERSION: &str = "forge.device-inventory-resource-convergence/v1";
pub(crate) const EVALUATION_MODE: &str = "owner_bound_inventory_resource_convergence_only";

pub(crate) fn validate(inventory: &Value, resource_view: &Value) -> Result<(), RemoteError> {
    // Revalidate both source envelopes at the join boundary. This is
    // deliberately redundant with the authenticated GET readers because the
    // convergence helper is also reachable from injected TUI/dispatch seams;
    // matching owner and resource counters alone must never promote malformed
    // metadata into a converged observation.
    crate::device_inventory_observation_v2_command::validate_remote_response(inventory)
        .map_err(|_| RemoteError("Forge API returned an invalid v2 device inventory".into()))?;
    crate::device_client_instance_resource_view_command::validate_remote_response(resource_view)
        .map_err(|_| {
            RemoteError("Forge API returned an invalid client-instance resource view".into())
        })?;
    if inventory.get("owner_declaration") != resource_view.get("owner_declaration")
        || inventory
            .get("owner_declaration_unverified")
            .and_then(Value::as_bool)
            != Some(true)
        || resource_view
            .get("owner_declaration_unverified")
            .and_then(Value::as_bool)
            != Some(true)
        || inventory
            .get("inventory_declarations_unverified")
            .and_then(Value::as_bool)
            != Some(true)
        || resource_view
            .get("device_attributes_unverified")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(RemoteError(
            "Forge API inventory/resource owner or declaration drifted".into(),
        ));
    }
    let Some(inventory_devices) = inventory.get("devices").and_then(Value::as_array) else {
        return Err(RemoteError(
            "Forge API returned an invalid inventory/resource convergence pair".into(),
        ));
    };
    let Some(resource_devices) = resource_view.get("devices").and_then(Value::as_array) else {
        return Err(RemoteError(
            "Forge API returned an invalid inventory/resource convergence pair".into(),
        ));
    };
    if inventory_devices.len() != resource_devices.len()
        || inventory_devices.iter().any(|candidate| {
            let Some(device) = candidate.get("device") else {
                return true;
            };
            let Some(device_id) = device.get("device_id").and_then(Value::as_str) else {
                return true;
            };
            let Some(resource_device) = resource_devices
                .iter()
                .find(|row| row.get("device_id").and_then(Value::as_str) == Some(device_id))
            else {
                return true;
            };
            candidate.get("instance_id") != resource_device.get("runner_instance_id")
                || candidate.get("revision") != resource_device.get("revision")
                || candidate.get("generation") != resource_device.get("generation")
                || candidate.get("heartbeat_sequence") != resource_device.get("heartbeat_sequence")
                || device.get("owner") != resource_device.get("owner")
                || device.get("approval_state") != resource_device.get("approval_state")
                || device.get("cordon_state") != resource_device.get("cordon_state")
                || device.get("reservation_state") != resource_device.get("reservation_state")
                || device.get("liveness") != resource_device.get("liveness")
                || device.get("snapshot_observed_at_ms") != resource_device.get("observed_at_ms")
                || device.get("os") != resource_device.get("os")
                || device.get("architecture") != resource_device.get("architecture")
                || device.get("available_cpu_cores") != resource_device.get("available_cpu_cores")
                || device.get("available_memory_bytes")
                    != resource_device.get("available_memory_bytes")
                || device.get("available_storage_bytes")
                    != resource_device.get("available_storage_bytes")
                || !gpu_summary_converged(device, resource_device)
        })
    {
        return Err(RemoteError(
            "Forge API inventory/resource observations did not converge".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::validate;

    fn inventory() -> Value {
        serde_json::from_str::<Value>(include_str!(
            "../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
        ))
        .expect("inventory/resource convergence fixture")
        .get("inventory")
        .cloned()
        .expect("inventory fixture member")
    }

    fn resource_view() -> Value {
        serde_json::from_str::<Value>(include_str!(
            "../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
        ))
        .expect("inventory/resource convergence fixture")
        .get("resource_view")
        .cloned()
        .expect("resource view fixture member")
    }

    #[test]
    fn join_rejects_nested_authority_even_when_resource_fields_match() {
        let mut observation = inventory();
        observation["execution_authorized"] = Value::Bool(true);

        let error =
            validate(&observation, &resource_view()).expect_err("authority must fail closed");
        assert_eq!(
            error.to_string(),
            "Forge API returned an invalid v2 device inventory"
        );
    }
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

pub(crate) fn envelope(inventory: Value, resource_view: Value) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "evaluation_mode": EVALUATION_MODE,
        "inventory": inventory,
        "resource_view": resource_view,
        "converged": true,
        "read_only": true,
        "authority": {
            "inventory_authoritative": false,
            "device_identity_verified": false,
            "reservation_created": false,
            "lease_issued": false,
            "execution_authorized": false,
            "dispatch_performed": false,
            "audit_published": false
        }
    })
}
