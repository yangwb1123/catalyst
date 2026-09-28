//! Bounded Rust consumer for the owner-bound client-instance/session-view
//! contract. This is a local observation decoder only; it is not an
//! authenticated session read and has no Prompt or device authority.

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_INSTANCES: usize = 128;
const MAX_SESSION_IDS: usize = 128;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_TIMESTAMP_MS: i64 = 9_007_199_254_740_991;
const SCHEMA_VERSION: &str = "forge.client-instance-session-view/v1";
const EVALUATION_MODE: &str = "owner_bound_session_view_only";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientInstanceSessionViewOutput {
    schema_version: String,
    evaluation_mode: String,
    #[serde(rename = "owner_declaration")]
    owner: Owner,
    owner_declaration_unverified: bool,
    instances: Vec<Instance>,
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
#[allow(
    clippy::struct_field_names,
    reason = "Preserve frozen wire field names and existing Serde type-name diagnostics"
)]
struct Instance {
    instance_id: String,
    client_kind: String,
    session_ids: Vec<String>,
    observed_at_ms: i64,
    status: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent contract flags preserve the frozen observation and authority wire shape"
)]
struct Authority {
    owner_authenticated: bool,
    session_read_authorized: bool,
    prompt_write_authorized: bool,
    device_identity_verified: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<ClientInstanceSessionViewOutput, Box<dyn Error>> {
    let DeviceCommand::ClientSessionViewPreview { input: input_path } = command else {
        return Err("device client-instance/session-view preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("client-instance/session-view input contains duplicate JSON keys: {error}")
    })?;
    let output: ClientInstanceSessionViewOutput = serde_json::from_slice(&bytes)
        .map_err(|error| format!("client-instance/session-view input is invalid JSON: {error}"))?;
    validate_output(&output)?;
    Ok(output)
}

/// Validates the same strict observation shape for the authenticated
/// candidate reader. The response remains a metadata-only value and is never
/// promoted to a client registration or session authority.
pub(crate) fn validate_remote_response(value: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    let output: ClientInstanceSessionViewOutput = serde_json::from_value(value.clone())
        .map_err(|error| format!("client-instance/session-view response is invalid: {error}"))?;
    validate_output(&output)
}

pub(crate) fn write_output(
    output: &ClientInstanceSessionViewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    write_human_output(output, "offline", writer)
}

/// Renders a validated response returned by the explicitly enabled remote
/// candidate. The label keeps the transport distinction visible in TUI
/// output while preserving the same metadata-only projection.
pub(crate) fn write_remote_output(
    value: &serde_json::Value,
    writer: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let output: ClientInstanceSessionViewOutput = serde_json::from_value(value.clone())
        .map_err(|error| format!("client-instance/session-view response is invalid: {error}"))?;
    validate_output(&output)?;
    write_human_output(&output, "remote", writer)?;
    Ok(())
}

fn write_human_output(
    output: &ClientInstanceSessionViewOutput,
    source: &str,
    writer: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        writer,
        "{source} client-instance/session-view [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "owner={}/{}/{} instances={} read_only=true",
        output.owner.issuer,
        output.owner.subject,
        output.owner.tenant_id,
        output.instances.len()
    )?;
    for instance in &output.instances {
        writeln!(
            writer,
            "instance {}: client_kind={} status={} observed_at_ms={} sessions={}",
            instance.instance_id,
            instance.client_kind,
            instance.status,
            instance.observed_at_ms,
            instance.session_ids.join(",")
        )?;
    }
    writeln!(
        writer,
        "authority: owner_authenticated=false session_read_authorized=false prompt_write_authorized=false device_identity_verified=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn validate_output(output: &ClientInstanceSessionViewOutput) -> Result<(), Box<dyn Error>> {
    if output.schema_version != SCHEMA_VERSION
        || output.evaluation_mode != EVALUATION_MODE
        || !valid_owner(&output.owner)
        || !output.owner_declaration_unverified
        || !output.read_only
        || !authority_is_false(output.authority)
        || output.instances.len() > MAX_INSTANCES
    {
        return Err("client-instance/session-view observation is invalid".into());
    }
    for (index, instance) in output.instances.iter().enumerate() {
        if !valid_instance(instance)
            || (index > 0 && output.instances[index - 1].instance_id >= instance.instance_id)
        {
            return Err(format!(
                "client-instance/session-view instance {:?} is invalid",
                instance.instance_id
            )
            .into());
        }
    }
    Ok(())
}

fn valid_instance(instance: &Instance) -> bool {
    valid_identifier(&instance.instance_id)
        && valid_client_kind(&instance.client_kind)
        && instance.session_ids.len() <= MAX_SESSION_IDS
        && instance.observed_at_ms > 0
        && instance.observed_at_ms <= MAX_TIMESTAMP_MS
        && valid_status(&instance.status)
        && instance
            .session_ids
            .iter()
            .enumerate()
            .all(|(index, session_id)| {
                valid_identifier(session_id)
                    && (index == 0 || instance.session_ids[index - 1] < *session_id)
            })
}

fn valid_owner(owner: &Owner) -> bool {
    valid_owner_part(&owner.issuer)
        && valid_owner_part(&owner.subject)
        && valid_owner_part(&owner.tenant_id)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OWNER_PART_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-' | '+' | '/'))
    })
}

fn valid_client_kind(value: &str) -> bool {
    matches!(value, "cli" | "tui" | "web" | "app" | "mobile")
}

fn valid_status(value: &str) -> bool {
    matches!(value, "active" | "idle" | "offline" | "unknown")
}

fn authority_is_false(authority: Authority) -> bool {
    !authority.owner_authenticated
        && !authority.session_read_authorized
        && !authority.prompt_write_authorized
        && !authority.device_identity_verified
        && !authority.reservation_created
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(
            format!("client-instance/session-view input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_client_session_view_command_tests.rs"]
mod tests;
