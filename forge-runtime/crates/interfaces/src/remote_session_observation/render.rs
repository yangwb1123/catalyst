use std::{collections::BTreeMap, io};

use serde_json::Value;

pub(in super::super) fn render_human(value: &Value, writer: &mut impl io::Write) -> io::Result<()> {
    let summary = &value["resource_summary"];
    let inventory = value["inventory"]["devices"]
        .as_array()
        .map(|devices| {
            devices
                .iter()
                .filter_map(|candidate| {
                    let device = candidate["device"].get("device_id")?.as_str()?;
                    Some((device, candidate))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    writeln!(
        writer,
        "offline session device observation [{}] at {}",
        value["schema_version"].as_str().unwrap_or("unknown"),
        value["evaluated_at_ms"].as_i64().unwrap_or_default()
    )?;
    writeln!(
        writer,
        "owner={} conversation={} run={}",
        value["owner"]["subject"].as_str().unwrap_or("unknown"),
        value["conversation_id"].as_str().unwrap_or("unknown"),
        value["run_id"].as_str().unwrap_or("unknown")
    )?;
    render_resources(summary, writer)?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )?;
    render_decisions(value, &inventory, writer)
}

fn render_resources(summary: &Value, writer: &mut impl io::Write) -> io::Result<()> {
    writeln!(
        writer,
        "resources: devices={} runner_instances={} cpu={} memory={} storage={} gpus={} gpu_memory={} eligible_devices={} eligible_instances={}",
        summary["device_count"].as_u64().unwrap_or_default(),
        summary["runner_instance_count"]
            .as_u64()
            .unwrap_or_default(),
        summary["available_cpu_cores"].as_u64().unwrap_or_default(),
        summary["available_memory_bytes"]
            .as_u64()
            .unwrap_or_default(),
        summary["available_storage_bytes"]
            .as_u64()
            .unwrap_or_default(),
        summary["available_gpu_count"].as_u64().unwrap_or_default(),
        summary["available_gpu_memory_bytes"]
            .as_u64()
            .unwrap_or_default(),
        summary["eligible_device_count"]
            .as_u64()
            .unwrap_or_default(),
        summary["eligible_instance_count"]
            .as_u64()
            .unwrap_or_default()
    )
}

fn render_decisions(
    value: &Value,
    inventory: &BTreeMap<&str, &Value>,
    writer: &mut impl io::Write,
) -> io::Result<()> {
    for decision in value["placement_observation"]["decisions"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let device = decision["device_id"].as_str().unwrap_or("unknown");
        let instance = decision["instance_id"].as_str().unwrap_or("unknown");
        let resources = inventory
            .get(device)
            .map_or_else(String::new, |candidate| resource_description(candidate));
        if decision["matches_requirements"].as_bool().unwrap_or(false) {
            writeln!(writer, "{device}/{instance}: matches{resources}")?;
        } else {
            let reasons = decision["exclusion_reasons"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",");
            writeln!(
                writer,
                "{device}/{instance}: excluded ({reasons}){resources}"
            )?;
        }
    }
    Ok(())
}

fn resource_description(candidate: &Value) -> String {
    let declaration = &candidate["device"];
    format!(
        " resources=cpu:{} memory:{} storage:{} gpu:{} gpu_memory:{}",
        declaration["available_cpu_cores"]
            .as_u64()
            .unwrap_or_default(),
        declaration["available_memory_bytes"]
            .as_u64()
            .unwrap_or_default(),
        declaration["available_storage_bytes"]
            .as_u64()
            .unwrap_or_default(),
        declaration["gpu"]["present"].as_bool().unwrap_or(false),
        declaration["gpu"]["memory_bytes"]
            .as_u64()
            .unwrap_or_default(),
    )
}
