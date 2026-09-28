//! Local projection of owner conversations onto one caller-declared client
//! instance.  The declaration is an observation and never changes the
//! authenticated request, Conversation ownership, or any device authority.

use std::{collections::BTreeSet, error::Error, fs::File, io::Read, path::Path};

use serde::Deserialize;
use serde_json::Value;

const MAX_INSTANCE_ID_BYTES: usize = 128;
const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const CONVERGENCE_SCHEMA_VERSION: &str = "forge.client-instance-session-resource-convergence/v1";
const CONVERGENCE_EVALUATION_MODE: &str =
    "owner_bound_client_instance_session_resource_convergence_only";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConvergenceEnvelope {
    schema_version: String,
    evaluation_mode: String,
    session_view: Value,
    resource_view: Value,
    converged: bool,
    read_only: bool,
    authority: ConvergenceAuthority,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent contract flags preserve the frozen observation and authority wire shape"
)]
struct ConvergenceAuthority {
    owner_authenticated: bool,
    session_read_authorized: bool,
    prompt_write_authorized: bool,
    device_identity_verified: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientInstanceSessionScope {
    pub(crate) instance_id: String,
    pub(crate) client_kind: String,
    pub(crate) session_ids: BTreeSet<String>,
}

/// Loads and strictly validates an offline client-instance/session view file.
/// The file is a caller-supplied display declaration; it is never persisted
/// and cannot grant a remote client or device permission.
pub(crate) fn read_local_view(input: &str) -> Result<Value, Box<dyn Error>> {
    let value = read_local_json(input)?;
    validated_session_projection(&value)
}

/// Loads the resource side of an offline client-instance projection. A
/// session-only file cannot prove that a Runner target belongs to the selected
/// instance, so admission candidates require a resource or converged file.
pub(crate) fn read_local_resource_view(input: &str) -> Result<Value, Box<dyn Error>> {
    let value = read_local_json(input)?;
    match value.get("schema_version").and_then(Value::as_str) {
        Some("forge.client-instance-resource-view/v1") => {
            crate::device_client_instance_resource_view_command::validate_remote_response(&value)?;
            Ok(value)
        }
        Some(CONVERGENCE_SCHEMA_VERSION) => {
            validate_convergence_envelope(&value)?;
            value
                .get("resource_view")
                .cloned()
                .ok_or_else(|| "client-instance convergence view has no resource_view".into())
        }
        _ => Err("Runner admission --instance-view requires a resource or converged view".into()),
    }
}

/// Extracts one instance from a strictly validated session/resource view.
/// Unknown instance IDs fail closed so a typo never silently broadens the
/// session projection.
pub(crate) fn scope_from_view(
    view: &Value,
    instance_id: &str,
) -> Result<ClientInstanceSessionScope, Box<dyn Error>> {
    validate_instance_id(instance_id)?;
    let view = validated_session_projection(view)?;
    let instances = view
        .get("instances")
        .and_then(Value::as_array)
        .ok_or_else(|| "client-instance view has no instances array".to_owned())?;
    let instance = instances
        .iter()
        .find(|candidate| candidate.get("instance_id").and_then(Value::as_str) == Some(instance_id))
        .ok_or_else(|| format!("client-instance view does not declare instance {instance_id:?}"))?;
    let client_kind = instance
        .get("client_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "client-instance view instance has no client_kind".to_owned())?;
    let raw_session_ids = instance
        .get("session_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| "client-instance view instance has no session_ids".to_owned())?;
    let mut session_ids = BTreeSet::new();
    for value in raw_session_ids {
        let session_id = value
            .as_str()
            .ok_or_else(|| "client-instance view session_ids must contain strings".to_owned())?;
        if !valid_identifier(session_id) || !session_ids.insert(session_id.to_owned()) {
            return Err("client-instance view contains an invalid or duplicate session id".into());
        }
    }
    Ok(ClientInstanceSessionScope {
        instance_id: instance_id.to_owned(),
        client_kind: client_kind.to_owned(),
        session_ids,
    })
}

fn validated_session_projection(value: &Value) -> Result<Value, Box<dyn Error>> {
    match value.get("schema_version").and_then(Value::as_str) {
        Some("forge.client-instance-session-view/v1") => {
            crate::device_client_session_view_command::validate_remote_response(value)?;
            Ok(value.clone())
        }
        Some("forge.client-instance-resource-view/v1") => {
            crate::device_client_instance_resource_view_command::validate_remote_response(value)?;
            Ok(value.clone())
        }
        Some(CONVERGENCE_SCHEMA_VERSION) => validate_convergence_envelope(value),
        _ => Err("client-instance view schema_version is invalid".into()),
    }
}

fn validate_convergence_envelope(value: &Value) -> Result<Value, Box<dyn Error>> {
    let envelope: ConvergenceEnvelope = serde_json::from_value(value.clone())
        .map_err(|error| format!("client-instance convergence envelope is invalid: {error}"))?;
    if envelope.schema_version != CONVERGENCE_SCHEMA_VERSION
        || envelope.evaluation_mode != CONVERGENCE_EVALUATION_MODE
        || !envelope.converged
        || !envelope.read_only
        || !authority_is_false(&envelope.authority)
    {
        return Err("client-instance convergence envelope is invalid".into());
    }
    crate::device_client_session_view_command::validate_remote_response(&envelope.session_view)
        .map_err(|error| format!("client-instance convergence session view is invalid: {error}"))?;
    crate::device_client_instance_resource_view_command::validate_remote_response(
        &envelope.resource_view,
    )
    .map_err(|error| format!("client-instance convergence resource view is invalid: {error}"))?;
    if envelope.session_view.get("owner_declaration")
        != envelope.resource_view.get("owner_declaration")
        || envelope.session_view.get("owner_declaration_unverified")
            != envelope.resource_view.get("owner_declaration_unverified")
        || envelope.session_view.get("instances") != envelope.resource_view.get("instances")
    {
        return Err("client-instance convergence observations did not converge".into());
    }
    Ok(envelope.session_view)
}

fn authority_is_false(authority: &ConvergenceAuthority) -> bool {
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
        std::io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!("client-instance view input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}

fn read_local_json(input: &str) -> Result<Value, Box<dyn Error>> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes)
        .map_err(|error| format!("client-instance view contains duplicate JSON keys: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("client-instance view is invalid JSON: {error}").into())
}

/// Returns whether an owner Conversation is in the selected local instance
/// projection. Missing or malformed declaration data is treated as invisible.
pub(crate) fn matches_conversation(
    conversation: &Value,
    view: Option<&Value>,
    instance_id: Option<&str>,
) -> bool {
    let Some(instance_id) = instance_id else {
        return true;
    };
    // An active local instance filter without a validated projection must
    // remain empty. Treating a missing view as "no filter" would broaden the
    // TUI back to every owner Conversation during a refresh gap or after a
    // caller clears the observed candidate, which is the opposite of the
    // fail-closed display boundary.
    let Some(view) = view else {
        return false;
    };
    let Some(conversation_id) = conversation.get("id").and_then(Value::as_str) else {
        return false;
    };
    scope_from_view(view, instance_id)
        .is_ok_and(|scope| scope.session_ids.contains(conversation_id))
}

pub(crate) fn declared_instance_ids(view: &Value) -> impl Iterator<Item = &str> {
    view.get("instances")
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|instances| instances.iter())
        .filter_map(|instance| instance.get("instance_id").and_then(Value::as_str))
}

pub(crate) fn validate_instance_id(instance_id: &str) -> Result<(), Box<dyn Error>> {
    if !valid_identifier(instance_id) || instance_id.len() > MAX_INSTANCE_ID_BYTES {
        return Err("client-instance id is invalid".into());
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_INSTANCE_ID_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-' | '+'))
    })
}

#[cfg(test)]
mod tests {
    use std::{io::Write, path::PathBuf};

    use serde_json::json;
    use tempfile::NamedTempFile;

    use super::{read_local_view, scope_from_view, validate_instance_id};

    fn session_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json")
    }

    fn resource_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json")
    }

    fn convergence_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../docs/contracts/fixtures/forge-client-instance-session-resource-convergence-v1.json",
        )
    }

    fn temporary_view(value: &serde_json::Value) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        serde_json::to_writer(file.as_file_mut(), value).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn local_session_view_projects_only_declared_instance_sessions() {
        let view = read_local_view(&session_fixture().display().to_string()).unwrap();
        let scope = scope_from_view(&view, "client-web-001").unwrap();
        assert_eq!(scope.client_kind, "web");
        assert_eq!(
            scope.session_ids.iter().collect::<Vec<_>>(),
            [&"conversation-001"]
        );
        assert!(super::matches_conversation(
            &json!({"id": "conversation-001"}),
            Some(&view),
            Some("client-web-001")
        ));
        assert!(!super::matches_conversation(
            &json!({"id": "conversation-002"}),
            Some(&view),
            Some("client-web-001")
        ));
    }

    #[test]
    fn local_resource_view_is_accepted_by_the_same_display_filter() {
        let view = read_local_view(&resource_fixture().display().to_string()).unwrap();
        let scope = scope_from_view(&view, "client-cli-001").unwrap();
        assert_eq!(scope.client_kind, "cli");
        assert!(scope.session_ids.contains("conversation-002"));
    }

    #[test]
    fn paired_convergence_view_projects_only_declared_instance_sessions() {
        let view = read_local_view(&convergence_fixture().display().to_string()).unwrap();
        assert_eq!(
            view["schema_version"],
            "forge.client-instance-session-view/v1"
        );
        let scope = scope_from_view(&view, "client-web-001").unwrap();
        assert_eq!(scope.client_kind, "web");
        assert_eq!(
            scope.session_ids.iter().collect::<Vec<_>>(),
            [&"conversation-001"]
        );
        assert!(super::matches_conversation(
            &json!({"id": "conversation-001"}),
            Some(&view),
            Some("client-web-001")
        ));
        assert!(!super::matches_conversation(
            &json!({"id": "conversation-002"}),
            Some(&view),
            Some("client-web-001")
        ));
    }

    #[test]
    fn convergence_scope_revalidates_the_outer_envelope() {
        let envelope: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/contracts/fixtures/forge-client-instance-session-resource-convergence-v1.json"
        ))
        .unwrap();
        let scope = scope_from_view(&envelope, "client-cli-001").unwrap();
        assert_eq!(scope.client_kind, "cli");
        assert!(scope.session_ids.contains("conversation-002"));

        let mut unknown_field = envelope.clone();
        unknown_field["unexpected"] = json!(true);
        let file = temporary_view(&unknown_field);
        assert!(read_local_view(&file.path().display().to_string()).is_err());

        let mut drifted = envelope;
        drifted["resource_view"]["instances"][0]["status"] = json!("idle");
        let file = temporary_view(&drifted);
        assert!(read_local_view(&file.path().display().to_string()).is_err());
    }

    #[test]
    fn convergence_file_rejects_duplicate_outer_keys_before_projection() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"{{"schema_version":"forge.client-instance-session-resource-convergence/v1","schema_version":"forge.client-instance-session-resource-convergence/v1"}}"#
        )
        .unwrap();
        file.flush().unwrap();
        assert!(read_local_view(&file.path().display().to_string()).is_err());
    }

    #[test]
    fn missing_view_never_broadens_an_active_instance_filter() {
        assert!(!super::matches_conversation(
            &json!({"id": "conversation-001"}),
            None,
            Some("client-web-001")
        ));
        assert!(super::matches_conversation(
            &json!({"id": "conversation-001"}),
            None,
            None
        ));
    }

    #[test]
    fn instance_and_session_ids_reject_path_separators() {
        assert!(validate_instance_id("client/web").is_err());
        let view = json!({
            "instances": [{
                "instance_id": "client-web-001",
                "client_kind": "web",
                "session_ids": ["conversation/001"]
            }]
        });
        assert!(scope_from_view(&view, "client-web-001").is_err());
    }
}
