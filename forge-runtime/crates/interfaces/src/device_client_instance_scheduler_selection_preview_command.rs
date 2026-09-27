//! Strict, offline consumer for the instance-scoped scheduler preview
//! projection.  The value joins caller-declared session membership with one
//! unverified resource row and a planning-only scheduler result.  It never
//! calls Forge Core and never promotes the candidate into scheduling,
//! reservation, lease, dispatch, or execution authority.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use serde::{Deserialize, Serialize};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 512 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_SESSION_IDS: usize = 128;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_CANDIDATES: u64 = 128;
const SCHEMA_VERSION: &str = "forge.client-instance-scheduler-selection-preview/v1";
const EVALUATION_MODE: &str = "owner_bound_client_instance_scheduler_selection_preview_only";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preview {
    schema_version: String,
    evaluation_mode: String,
    owner_declaration: Owner,
    instance: Instance,
    conversation_id: String,
    run_id: String,
    attempt_id: String,
    resource: Resource,
    scheduler_preview: SchedulerPreview,
    preview_only: bool,
    read_only: bool,
    authority: Authority,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Instance {
    instance_id: String,
    client_kind: String,
    session_ids: Vec<String>,
    observed_at_ms: u64,
    status: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    device_id: String,
    runner_instance_id: String,
    revision: u64,
    generation: u64,
    heartbeat_sequence: u64,
    observed_at_ms: u64,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
    liveness: String,
    available_cpu_cores: u64,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    gpu_count: u64,
    available_gpu_memory_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SchedulerPreview {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    attempt_id: String,
    evaluated_at_ms: u64,
    candidate_count: u64,
    eligible_candidate_count: u64,
    selection_available: bool,
    selection_reason: String,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    preview_only: bool,
    authority: SchedulerAuthority,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SchedulerAuthority {
    placement_selected: bool,
    reservation_created: bool,
    lease_issued: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    owner_authenticated: bool,
    session_read_authorized: bool,
    prompt_write_authorized: bool,
    device_identity_verified: bool,
    inventory_authoritative: bool,
    placement_selected: bool,
    reservation_created: bool,
    lease_issued: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<Preview, Box<dyn std::error::Error>> {
    let DeviceCommand::ClientInstanceSchedulerSelectionPreview { input } = command else {
        return Err("device client-instance scheduler-selection preview command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("client-instance scheduler-selection input contains duplicate JSON keys: {error}")
    })?;
    let preview: Preview = serde_json::from_slice(&bytes).map_err(|error| {
        format!("client-instance scheduler-selection input is invalid JSON: {error}")
    })?;
    validate(&preview)?;
    Ok(preview)
}

pub(crate) fn write_output(
    preview: &Preview,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, preview)?;
        return writeln!(writer);
    }
    let selected = preview
        .scheduler_preview
        .selected_device_id
        .as_deref()
        .zip(preview.scheduler_preview.selected_instance_id.as_deref())
        .map_or_else(|| "none".to_owned(), |(device, runner)| format!("{device}/{runner}"));
    writeln!(
        writer,
        "offline client-instance scheduler-selection preview [{}] instance={} kind={} conversation={} run={} attempt={} sessions={} resource={} revision={} generation={} heartbeat={} selected={} reason={}",
        preview.schema_version,
        preview.instance.instance_id,
        preview.instance.client_kind,
        preview.conversation_id,
        preview.run_id,
        preview.attempt_id,
        preview.instance.session_ids.len(),
        preview.resource.device_id,
        preview.resource.revision,
        preview.resource.generation,
        preview.resource.heartbeat_sequence,
        selected,
        preview.scheduler_preview.selection_reason,
    )?;
    writeln!(
        writer,
        "read_only=true preview_only=true authority: owner_authenticated=false session_read_authorized=false prompt_write_authorized=false device_identity_verified=false inventory_authoritative=false placement_selected=false reservation_created=false lease_issued=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let preview = match execute(command) {
        Ok(preview) => preview,
        Err(error) => {
            eprintln!("Device client-instance scheduler-selection preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&preview, json, &mut io::stdout().lock()) {
        eprintln!("failed to write client-instance scheduler-selection preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn validate(preview: &Preview) -> Result<(), Box<dyn std::error::Error>> {
    if preview.schema_version != SCHEMA_VERSION
        || preview.evaluation_mode != EVALUATION_MODE
        || !preview.preview_only
        || !preview.read_only
        || !authority_is_false(preview.authority)
        || !valid_owner(&preview.owner_declaration)
        || !valid_identifier(&preview.conversation_id)
        || !valid_identifier(&preview.run_id)
        || !valid_identifier(&preview.attempt_id)
        || !valid_instance(&preview.instance)
        || !valid_resource(&preview.resource)
        || !valid_scheduler_preview(&preview.scheduler_preview, preview)
    {
        return Err("client-instance scheduler-selection observation is invalid".into());
    }
    if !preview
        .instance
        .session_ids
        .iter()
        .any(|session_id| session_id == &preview.conversation_id)
    {
        return Err("scheduler preview conversation is not declared by the selected instance".into());
    }
    Ok(())
}

fn valid_owner(owner: &Owner) -> bool {
    [&owner.issuer, &owner.subject, &owner.tenant_id]
        .into_iter()
        .all(|part| {
            !part.is_empty()
                && part.len() <= MAX_OWNER_PART_BYTES
                && part.trim() == part
                && !part.chars().any(char::is_control)
        })
}

fn valid_instance(instance: &Instance) -> bool {
    valid_identifier(&instance.instance_id)
        && matches!(instance.client_kind.as_str(), "cli" | "tui" | "web" | "app" | "mobile")
        && instance.session_ids.len() <= MAX_SESSION_IDS
        && instance
            .session_ids
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && instance.session_ids.iter().all(|id| valid_identifier(id))
        && instance.observed_at_ms > 0
        && instance.observed_at_ms <= MAX_SAFE_INTEGER
        && matches!(
            instance.status.as_str(),
            "active" | "idle" | "offline" | "unknown"
        )
}

fn valid_resource(resource: &Resource) -> bool {
    valid_identifier(&resource.device_id)
        && valid_identifier(&resource.runner_instance_id)
        && valid_counter(resource.revision)
        && valid_counter(resource.generation)
        && valid_counter(resource.heartbeat_sequence)
        && resource.observed_at_ms > 0
        && resource.observed_at_ms <= MAX_SAFE_INTEGER
        && resource.available_cpu_cores <= 4_096
        && resource.available_memory_bytes <= MAX_SAFE_INTEGER
        && resource.available_storage_bytes <= MAX_SAFE_INTEGER
        && resource.gpu_count <= 32
        && resource.available_gpu_memory_bytes <= MAX_SAFE_INTEGER
        && matches!(resource.approval_state.as_str(), "pending" | "approved" | "revoked")
        && matches!(resource.cordon_state.as_str(), "clear" | "cordoned")
        && matches!(resource.reservation_state.as_str(), "none" | "reserved")
        && matches!(resource.liveness.as_str(), "online" | "offline")
}

fn valid_scheduler_preview(preview: &SchedulerPreview, parent: &Preview) -> bool {
    preview.schema_version == "forge.scheduler-selection-preview/v1"
        && preview.evaluation_mode == "pure_scheduler_selection_preview"
        && preview.owner == parent.owner_declaration
        && preview.conversation_id == parent.conversation_id
        && preview.run_id == parent.run_id
        && preview.attempt_id == parent.attempt_id
        && preview.evaluated_at_ms > 0
        && preview.evaluated_at_ms <= MAX_SAFE_INTEGER
        && preview.candidate_count <= MAX_CANDIDATES
        && preview.eligible_candidate_count <= preview.candidate_count
        && preview.preview_only
        && scheduler_authority_is_false(&preview.authority)
        && match (
            preview.selection_available,
            preview.selected_device_id.as_deref(),
            preview.selected_instance_id.as_deref(),
        ) {
            (true, Some(device), Some(runner)) => {
                preview.eligible_candidate_count > 0
                    && preview.selection_reason == "first_sorted_eligible_candidate"
                    && valid_identifier(device)
                    && valid_identifier(runner)
                    && device == parent.resource.device_id
                    && runner == parent.resource.runner_instance_id
            }
            (false, None, None) => preview.selection_reason == "no_eligible_candidate",
            _ => false,
        }
}

fn valid_counter(value: u64) -> bool {
    value > 0 && value <= MAX_SAFE_INTEGER
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '+' | '/' | '-'))
    })
}

fn authority_is_false(authority: Authority) -> bool {
    !authority.owner_authenticated
        && !authority.session_read_authorized
        && !authority.prompt_write_authorized
        && !authority.device_identity_verified
        && !authority.inventory_authoritative
        && !authority.placement_selected
        && !authority.reservation_created
        && !authority.lease_issued
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

fn scheduler_authority_is_false(authority: &SchedulerAuthority) -> bool {
    !authority.placement_selected
        && !authority.reservation_created
        && !authority.lease_issued
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        let path = Path::new(input);
        if !path.is_file() {
            return Err("client-instance scheduler-selection input must be a regular file".into());
        }
        File::open(path)?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "client-instance scheduler-selection input exceeds {MAX_INPUT_BYTES} bytes"
        )
        .into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::{io::Write, path::PathBuf};
    use tempfile::NamedTempFile;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../docs/contracts/fixtures/forge-client-instance-scheduler-selection-preview-v1.json"
        ))
        .expect("instance scheduler preview fixture")
    }

    fn command(path: &str) -> DeviceCommand {
        DeviceCommand::ClientInstanceSchedulerSelectionPreview {
            input: path.to_owned(),
        }
    }

    fn temp(value: &Value) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        serde_json::to_writer(file.as_file_mut(), value).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn canonical_fixture_binds_membership_resource_and_preview() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../docs/contracts/fixtures/forge-client-instance-scheduler-selection-preview-v1.json",
        );
        let preview = execute(&command(&path.display().to_string())).unwrap();
        assert_eq!(preview.instance.instance_id, "client-web-001");
        assert_eq!(preview.resource.runner_instance_id, "runner-a");
        assert_eq!(
            preview.scheduler_preview.selected_instance_id.as_deref(),
            Some("runner-a")
        );
    }

    #[test]
    fn hidden_conversation_is_rejected_before_rendering() {
        let mut value = fixture();
        value["conversation_id"] = Value::String("conversation-hidden".into());
        let file = temp(&value);
        assert!(execute(&command(&file.path().display().to_string())).is_err());
    }

    #[test]
    fn selected_resource_drift_is_rejected() {
        let mut value = fixture();
        value["scheduler_preview"]["selected_device_id"] = Value::String("device-b".into());
        let file = temp(&value);
        assert!(execute(&command(&file.path().display().to_string())).is_err());
    }

    #[test]
    fn authority_and_duplicate_keys_fail_closed() {
        let mut authority = fixture();
        authority["authority"]["reservation_created"] = Value::Bool(true);
        let file = temp(&authority);
        assert!(execute(&command(&file.path().display().to_string())).is_err());

        let mut duplicate = NamedTempFile::new().unwrap();
        write!(
            duplicate,
            r#"{{"schema_version":"forge.client-instance-scheduler-selection-preview/v1","schema_version":"forge.client-instance-scheduler-selection-preview/v1"}}"#
        )
        .unwrap();
        duplicate.flush().unwrap();
        assert!(execute(&command(&duplicate.path().display().to_string())).is_err());
    }
}
