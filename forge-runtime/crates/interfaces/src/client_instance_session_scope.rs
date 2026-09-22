//! Local projection of owner conversations onto one caller-declared client
//! instance.  The declaration is an observation and never changes the
//! authenticated request, Conversation ownership, or any device authority.

use std::{collections::BTreeSet, error::Error};

use serde_json::Value;

use crate::args::DeviceCommand;

const MAX_INSTANCE_ID_BYTES: usize = 128;

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
    match crate::device_client_session_view_command::execute(
        &DeviceCommand::ClientSessionViewPreview {
            input: input.to_owned(),
        },
    ) {
        Ok(output) => serde_json::to_value(output).map_err(Into::into),
        Err(session_error) => {
            let output = crate::device_client_instance_resource_view_command::execute(
                &DeviceCommand::ClientInstanceResourceViewPreview {
                    input: input.to_owned(),
                },
            )
            .map_err(|resource_error| {
                format!(
                    "client-instance view must be a strict session-view or resource-view: session-view={session_error}; resource-view={resource_error}"
                )
            })?;
            serde_json::to_value(output).map_err(Into::into)
        }
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
    match view.get("schema_version").and_then(Value::as_str) {
        Some("forge.client-instance-session-view/v1") => {
            crate::device_client_session_view_command::validate_remote_response(view)?;
        }
        Some("forge.client-instance-resource-view/v1") => {
            crate::device_client_instance_resource_view_command::validate_remote_response(view)?;
        }
        _ => return Err("client-instance view schema_version is invalid".into()),
    }
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
    use std::path::PathBuf;

    use serde_json::json;

    use super::{read_local_view, scope_from_view, validate_instance_id};

    fn session_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json")
    }

    fn resource_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json")
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
