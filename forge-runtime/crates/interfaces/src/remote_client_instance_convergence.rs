//! Owner-bound convergence for the paired client-instance observations.
//!
//! The two source values are validated by their existing strict readers before
//! this module joins them. The result is a display-only observation and never
//! grants session, inventory, scheduling, or execution authority.

use serde_json::{Value, json};

use crate::remote_command::RemoteError;

pub(crate) const SCHEMA_VERSION: &str = "forge.client-instance-session-resource-convergence/v1";
pub(crate) const EVALUATION_MODE: &str =
    "owner_bound_client_instance_session_resource_convergence_only";

pub(crate) fn validate(session_view: &Value, resource_view: &Value) -> Result<(), RemoteError> {
    // Keep the join boundary self-contained. The normal RemoteClient path
    // already validates each GET before calling this function, but callers in
    // the TUI/dispatch layer can also reach the join with injected values.
    // Re-run the strict source validators so a manually assembled envelope
    // cannot bypass the display-only contract by matching owner/instance
    // fields alone.
    crate::device_client_session_view_command::validate_remote_response(session_view).map_err(
        |_| RemoteError("Forge API returned an invalid client-instance session view".into()),
    )?;
    crate::device_client_instance_resource_view_command::validate_remote_response(resource_view)
        .map_err(|_| {
            RemoteError("Forge API returned an invalid client-instance resource view".into())
        })?;
    if session_view.get("owner_declaration") != resource_view.get("owner_declaration")
        || session_view.get("owner_declaration_unverified")
            != resource_view.get("owner_declaration_unverified")
        || session_view.get("instances") != resource_view.get("instances")
    {
        return Err(RemoteError(
            "Forge API client-instance session/resource observations did not converge".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::validate;

    fn session_view() -> Value {
        serde_json::from_str(include_str!(
            "../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
        ))
        .expect("client-instance session view fixture")
    }

    fn resource_view() -> Value {
        serde_json::from_str(include_str!(
            "../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
        ))
        .expect("client-instance resource view fixture")
    }

    #[test]
    fn join_rejects_nested_authority_even_when_owner_and_instances_match() {
        let session = session_view();
        let mut resource = resource_view();
        resource["authority"]["dispatch_performed"] = Value::Bool(true);

        let error = validate(&session, &resource).expect_err("authority must fail closed");
        assert_eq!(
            error.to_string(),
            "Forge API returned an invalid client-instance resource view"
        );
    }
}

pub(crate) fn envelope(session_view: Value, resource_view: Value) -> Value {
    let mut value = json!({
        "schema_version": SCHEMA_VERSION,
        "evaluation_mode": EVALUATION_MODE,
        "session_view": null,
        "resource_view": null,
        "converged": true,
        "read_only": true,
        "authority": {
            "owner_authenticated": false,
            "session_read_authorized": false,
            "prompt_write_authorized": false,
            "device_identity_verified": false,
            "reservation_created": false,
            "execution_authorized": false,
            "dispatch_performed": false,
            "audit_published": false
        }
    });
    value["session_view"] = session_view;
    value["resource_view"] = resource_view;
    value
}
