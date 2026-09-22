use std::io::{self, Write};

use serde::Serialize;

use super::{Authority, ComputedSummary, Owner};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct ResourceSummaryOutput {
    schema_version: &'static str,
    evaluation_mode: &'static str,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    inventory_declarations_unverified: bool,
    placement_declaration_unverified: bool,
    notice: &'static str,
    device_count: usize,
    runner_instance_count: usize,
    available_cpu_cores: u64,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    available_gpu_count: usize,
    available_gpu_memory_bytes: u64,
    eligible_device_count: usize,
    eligible_instance_count: usize,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
}

pub(super) fn from_summary(value: ComputedSummary) -> ResourceSummaryOutput {
    ResourceSummaryOutput {
        schema_version: value.schema_version,
        evaluation_mode: value.evaluation_mode,
        owner: value.owner,
        conversation_id: value.conversation_id,
        run_id: value.run_id,
        evaluated_at_ms: value.evaluated_at_ms,
        owner_declaration_unverified: value.owner_declaration_unverified,
        inventory_declarations_unverified: value.inventory_declarations_unverified,
        placement_declaration_unverified: value.placement_declaration_unverified,
        notice: value.notice,
        device_count: value.device_count,
        runner_instance_count: value.runner_instance_count,
        available_cpu_cores: value.available_cpu_cores,
        available_memory_bytes: value.available_memory_bytes,
        available_storage_bytes: value.available_storage_bytes,
        available_gpu_count: value.available_gpu_count,
        available_gpu_memory_bytes: value.available_gpu_memory_bytes,
        eligible_device_count: value.eligible_device_count,
        eligible_instance_count: value.eligible_instance_count,
        selected_device_id: value.selected_device_id,
        selected_instance_id: value.selected_instance_id,
        authority: value.authority,
    }
}

pub(crate) fn write_output(
    output: &ResourceSummaryOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline device resource summary [{}] at {}",
        output.schema_version, output.evaluated_at_ms
    )?;
    writeln!(
        writer,
        "owner={} conversation={} run={}",
        output.owner.subject, output.conversation_id, output.run_id
    )?;
    writeln!(
        writer,
        "resources: devices={} runner_instances={} cpu={} memory={} storage={} gpus={} gpu_memory={} eligible_devices={} eligible_instances={}",
        output.device_count,
        output.runner_instance_count,
        output.available_cpu_cores,
        output.available_memory_bytes,
        output.available_storage_bytes,
        output.available_gpu_count,
        output.available_gpu_memory_bytes,
        output.eligible_device_count,
        output.eligible_instance_count
    )?;
    writeln!(
        writer,
        "declarations: owner_unverified={} inventory_unverified={} placement_unverified={}",
        output.owner_declaration_unverified,
        output.inventory_declarations_unverified,
        output.placement_declaration_unverified
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}
