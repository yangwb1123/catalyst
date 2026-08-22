use super::super::*;
use super::{fixture, reseal_grant, reseal_request};

const INVALID_VERSION_REFS: &[&str] = &[
    "lateſt",
    "version 1",
    "版本1",
    ".v1",
    "LATEST",
    "current",
    "active",
    "v*",
];

#[test]
fn version_ref_uses_a_closed_ascii_lexical_domain() {
    canonical_grant_json(&fixture().grant).expect("golden Grant remains valid");
    for version in ["v1", "2026-08-11", "sha256:abcdef/1@prod+blue"] {
        super::super::scope_validation::validate_resource(&secret_resource(version))
            .expect("immutable ASCII version_ref");
    }
    for version in INVALID_VERSION_REFS {
        assert!(
            super::super::scope_validation::validate_resource(&secret_resource(version)).is_err(),
            "invalid version_ref {version:?}"
        );
    }
}

#[test]
fn invalid_version_refs_fail_strict_request_decode() {
    for version in INVALID_VERSION_REFS {
        let request = request_with_version(version);
        let canonical = super::super::canonical::encode(
            &request,
            MAX_ASSESSMENT_REQUEST_BYTES,
            "invalid secret version request",
        )
        .expect("encode malformed request canonically");
        assert!(
            decode_canonical_assessment_request(canonical.as_bytes()).is_err(),
            "invalid version_ref {version:?}"
        );
    }
    canonical_assessment_request_json(&request_with_version("version/2026-08-11@1"))
        .expect("valid immutable version request");
}

fn request_with_version(version: &str) -> DeclaredAssessmentRequest {
    let mut request = fixture().assessment_request;
    let resource = secret_resource(version);
    request.grant.scope = GrantScope {
        allow: vec![ScopeClause {
            resources: vec![resource.clone()],
        }],
        deny: Vec::new(),
        effect_id: EffectId::SecretsRead,
    };
    reseal_grant(&mut request.grant);
    request.requested_action.effect_id = EffectId::SecretsRead;
    request.requested_action.resources = vec![resource];
    reseal_request(&mut request);
    request
}

fn secret_resource(version_ref: &str) -> ScopeResource {
    ScopeResource::SecretRef {
        broker_id: "fixture-broker".into(),
        secret_ref: "secrets/project/api-key".into(),
        version_ref: version_ref.into(),
    }
}
