use serde_json::Value;

/// Checks that the two explicit client-instance projections describe the same
/// owner-scoped session image. The resource view carries the same instance
/// rows as the session view; comparing those rows (including observed time)
/// prevents a refresh from mixing a newer resource image with an older Prompt
/// visibility declaration.
pub(super) fn observations_converged(session: &Value, resource: &Value) -> bool {
    session.get("owner_declaration") == resource.get("owner_declaration")
        && session.get("owner_declaration_unverified")
            == resource.get("owner_declaration_unverified")
        && session.get("instances") == resource.get("instances")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::observations_converged;

    fn pair(observed_at_ms: u64) -> (serde_json::Value, serde_json::Value) {
        let owner = json!({
            "issuer": "https://id.example",
            "subject": "user-1",
            "tenant_id": "tenant-1"
        });
        let instances = json!([{
            "instance_id": "client-web-001",
            "client_kind": "web",
            "session_ids": ["conversation-001"],
            "observed_at_ms": observed_at_ms,
            "status": "active"
        }]);
        (
            json!({
                "owner_declaration": owner,
                "owner_declaration_unverified": true,
                "instances": instances
            }),
            json!({
                "owner_declaration": owner,
                "owner_declaration_unverified": true,
                "instances": instances
            }),
        )
    }

    #[test]
    fn matching_instance_images_converge() {
        let (session, resource) = pair(200500);
        assert!(observations_converged(&session, &resource));
    }

    #[test]
    fn different_observation_times_fail_closed() {
        let (session, mut resource) = pair(200500);
        resource["instances"][0]["observed_at_ms"] = json!(200501);
        assert!(!observations_converged(&session, &resource));
    }
}
