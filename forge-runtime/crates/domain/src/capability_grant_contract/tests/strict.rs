use super::super::*;
use super::{fixture, reseal_grant, reseal_request};

#[test]
fn strict_decoder_rejects_duplicate_unknown_float_and_noncanonical_json() {
    let canonical = canonical_grant_json(&fixture().grant).expect("canonical grant");
    let duplicate = canonical.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"forgeos.capability-grant/v1\",\"api_version\":",
        1,
    );
    assert!(decode_canonical_grant(duplicate.as_bytes()).is_err());
    let unknown = canonical.replacen("{\"api_version\":", "{\"allowed\":true,\"api_version\":", 1);
    assert!(decode_canonical_grant(unknown.as_bytes()).is_err());
    let float = canonical.replacen("\"max_calls\":1", "\"max_calls\":1.0", 1);
    assert!(decode_canonical_grant(float.as_bytes()).is_err());
    assert!(decode_canonical_grant(format!(" {canonical}").as_bytes()).is_err());
    assert!(decode_canonical_grant(&vec![b' '; MAX_GRANT_BYTES + 1]).is_err());
}

#[test]
fn aliases_dynamic_authority_and_non_int64_numbers_fail() {
    let canonical = canonical_grant_json(&fixture().grant).expect("canonical grant");
    let alias = canonical.replace(
        "\"kind\":\"CapabilityGrant\"",
        "\"kind\":\"AuthorityGrant\"",
    );
    assert!(decode_canonical_grant(alias.as_bytes()).is_err());
    let authorized = canonical.replacen(
        "{\"api_version\":",
        "{\"authorized\":true,\"api_version\":",
        1,
    );
    assert!(decode_canonical_grant(authorized.as_bytes()).is_err());
    let exponent = canonical.replacen("\"trust_epoch\":1", "\"trust_epoch\":1e0", 1);
    assert!(decode_canonical_grant(exponent.as_bytes()).is_err());
    let overflow = canonical.replacen(
        "\"trust_epoch\":1",
        "\"trust_epoch\":9223372036854775808",
        1,
    );
    assert!(decode_canonical_grant(overflow.as_bytes()).is_err());
}

#[test]
fn vocabulary_is_exact_ordered_and_self_digesting() {
    let mut vocabulary = fixture().effect_vocabulary;
    vocabulary.effects.swap(0, 1);
    vocabulary.vocabulary_sha256 =
        effect_vocabulary_sha256(&vocabulary).expect("reseal vocabulary");
    assert!(canonical_effect_vocabulary_json(&vocabulary).is_err());

    let mut vocabulary = fixture().effect_vocabulary;
    vocabulary.vocabulary_sha256.replace_range(..1, "0");
    assert!(canonical_effect_vocabulary_json(&vocabulary).is_err());
}

#[test]
fn grant_proof_identity_time_and_transfer_fail_closed() {
    let mut grant = fixture().grant;
    grant.authority_proof.issuer.principal_type = PrincipalType::Agent;
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());

    let mut grant = fixture().grant;
    grant.authority_proof.proof_base64url = "A".into();
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());

    let mut grant = fixture().grant;
    grant.validity.transferable = true;
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());

    let mut grant = fixture().grant;
    grant.validity.expires_at_unix_ms = grant.validity.not_before_unix_ms;
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());

    let mut grant = fixture().grant;
    grant.grant_sha256.replace_range(..1, "0");
    assert!(canonical_grant_json(&grant).is_err());
}

#[test]
fn scope_paths_order_action_shape_and_separation_are_strict() {
    let mut grant = fixture().grant;
    let ScopeResource::RepoPath { path, .. } = &mut grant.scope.allow[0].resources[0] else {
        panic!("fixture repo path");
    };
    *path = "src/../secrets".into();
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());

    let mut grant = fixture().grant;
    grant.scope.allow.push(grant.scope.allow[0].clone());
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());

    let mut request = fixture().assessment_request;
    let ScopeResource::RepoPath { path_match, .. } = &mut request.requested_action.resources[0]
    else {
        panic!("fixture repo path");
    };
    *path_match = PathMatch::Subtree;
    reseal_request(&mut request);
    assert!(canonical_assessment_request_json(&request).is_err());

    let mut grant = fixture().grant;
    grant.separation_of_duty.requester = Principal {
        authority_domain: grant.authority_proof.issuer.authority_domain.clone(),
        principal_id: grant.authority_proof.issuer.principal_id.clone(),
        principal_type: grant.authority_proof.issuer.principal_type,
    };
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());
}

#[test]
fn process_exec_action_binds_command_and_usage_timeouts() {
    let fixture = fixture();
    let mut request = fixture.assessment_request;
    let command = command_resource(60_000);
    request.grant.scope = GrantScope {
        allow: vec![ScopeClause {
            resources: vec![command.clone()],
        }],
        deny: Vec::new(),
        effect_id: EffectId::ProcessExec,
    };
    reseal_grant(&mut request.grant);
    request.requested_action.effect_id = EffectId::ProcessExec;
    request.requested_action.resources = vec![command];
    request.requested_action.usage.timeout_ms = 1_000;
    reseal_request(&mut request);

    assert!(canonical_assessment_request_json(&request).is_err());
    let canonical = super::super::canonical::encode(
        &request,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "invalid timeout-bound request",
    )
    .expect("encode malformed request canonically");
    assert!(decode_canonical_assessment_request(canonical.as_bytes()).is_err());
    assert!(evaluate_declared_assessment(&fixture.effect_vocabulary, &request).is_err());
}

#[test]
fn ipv4_mapped_and_zoned_ipv6_resources_and_requests_fail_closed() {
    for host in [
        "::ffff:c000:201",
        "::ffff:192.0.2.1",
        "fe80::1%eth0",
        "::1%lo",
    ] {
        let resource = network_resource(host, HostKind::Ipv6);
        assert!(super::super::scope_validation::validate_resource(&resource).is_err());

        let fixture = fixture();
        let mut request = fixture.assessment_request;
        request.grant.scope = GrantScope {
            allow: vec![ScopeClause {
                resources: vec![resource.clone()],
            }],
            deny: Vec::new(),
            effect_id: EffectId::NetworkRead,
        };
        reseal_grant(&mut request.grant);
        request.requested_action.effect_id = EffectId::NetworkRead;
        request.requested_action.resources = vec![resource];
        reseal_request(&mut request);
        let canonical = super::super::canonical::encode(
            &request,
            MAX_ASSESSMENT_REQUEST_BYTES,
            "invalid IPv6 request",
        )
        .expect("encode malformed request canonically");
        assert!(decode_canonical_assessment_request(canonical.as_bytes()).is_err());
    }
}

#[test]
fn canonical_ipv4_is_not_admitted_as_dns() {
    let host = "192.0.2.1";
    let dns_resource = network_resource(host, HostKind::Dns);
    assert!(super::super::scope_validation::validate_resource(&dns_resource).is_err());

    let fixture = fixture();
    let mut request = fixture.assessment_request;
    request.grant.scope = GrantScope {
        allow: vec![ScopeClause {
            resources: vec![dns_resource.clone()],
        }],
        deny: Vec::new(),
        effect_id: EffectId::NetworkRead,
    };
    reseal_grant(&mut request.grant);
    request.requested_action.effect_id = EffectId::NetworkRead;
    request.requested_action.resources = vec![dns_resource];
    reseal_request(&mut request);
    let canonical = super::super::canonical::encode(
        &request,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "invalid dotted-quad DNS request",
    )
    .expect("encode malformed request canonically");
    assert!(decode_canonical_assessment_request(canonical.as_bytes()).is_err());

    let ipv4_resource = network_resource(host, HostKind::Ipv4);
    super::super::scope_validation::validate_resource(&ipv4_resource)
        .expect("canonical dotted-quad with ipv4 tag");
}

#[test]
fn plan_finalization_requires_final_binding_digests() {
    let mut grant = fixture().grant;
    grant.bindings.plan_sha256 = None;
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());

    grant.issuance_phase = IssuancePhase::BootstrapPlanning;
    grant.bindings.impact_sha256 = None;
    grant.bindings.risk_sha256 = None;
    reseal_grant(&mut grant);
    canonical_grant_json(&grant).expect("bootstrap may declare nullable final bindings");
}

#[test]
fn allow_clauses_are_or_branches_and_cannot_be_combined() {
    let fixture = fixture();
    let first = repo_resource("src/a.rs");
    let second = repo_resource("src/b.rs");
    let scope = GrantScope {
        allow: vec![
            ScopeClause {
                resources: vec![first.clone()],
            },
            ScopeClause {
                resources: vec![second.clone()],
            },
        ],
        deny: Vec::new(),
        effect_id: EffectId::RepoRead,
    };
    let mut action = fixture.assessment_request.requested_action;
    action.resources = vec![first, second];
    assert_eq!(
        super::super::scope_validation::scope_relation(&scope, &action),
        ScopeRelation::OutsideDeclaredScope
    );
}

#[test]
fn production_execution_requires_external_operator_declarations() {
    let mut grant = fixture().grant;
    grant.scope.effect_id = EffectId::ReleaseExecute;
    grant.scope.allow = vec![ScopeClause {
        resources: vec![
            ScopeResource::Artifact {
                artifact_kind: "release_bundle".into(),
                artifact_ref: "artifact://release".into(),
                artifact_sha256: "a".repeat(64),
            },
            ScopeResource::Environment {
                environment_class: EnvironmentClass::Production,
                environment_id: "production".into(),
                environment_sha256: "b".repeat(64),
            },
        ],
    }];
    grant.scope.deny.clear();
    reseal_grant(&mut grant);
    assert!(canonical_grant_json(&grant).is_err());

    grant.authority_proof.issuer = Issuer {
        authority_class: AuthorityClass::ExternalOperator,
        authority_domain: "external.fixture".into(),
        principal_id: "release-operator".into(),
        principal_type: PrincipalType::Operator,
    };
    grant.approval_refs = vec![ApprovalRef {
        approval_id: "approval-release".into(),
        approval_sha256: "c".repeat(64),
        authority_domain: "external.fixture".into(),
    }];
    reseal_grant(&mut grant);
    canonical_grant_json(&grant).expect("structural external declaration");
}

#[test]
fn declared_proof_is_excluded_only_from_grant_semantic_digest() {
    let fixture = fixture();
    let mut grant = fixture.grant;
    let digest = grant.grant_sha256.clone();
    grant.authority_proof.proof_base64url = "QUFBQUFBQUFBQUFB".into();
    assert_eq!(grant_sha256(&grant).expect("grant digest"), digest);
    canonical_grant_json(&grant).expect("alternative structurally valid proof");

    let mut request = fixture.assessment_request;
    request.grant = grant;
    assert_ne!(
        assessment_request_sha256(&request).expect("request digest"),
        request.request_sha256
    );
}

#[test]
fn evaluator_applies_deny_precedence_and_scope_boundaries() {
    let fixture = fixture();
    let mut denied = fixture.assessment_request.clone();
    set_requested_path(&mut denied, "src/secrets/key.rs");
    reseal_request(&mut denied);
    let assessment = evaluate_declared_assessment(&fixture.effect_vocabulary, &denied)
        .expect("assess denied action");
    assert_eq!(
        assessment.relations.scope,
        ScopeRelation::DeniedByDeclaration
    );
    assert_eq!(assessment.reason_codes, vec![ReasonCode::DenyMatched]);

    let mut outside = fixture.assessment_request;
    set_requested_path(&mut outside, "tests/model.rs");
    reseal_request(&mut outside);
    let assessment = evaluate_declared_assessment(&fixture.effect_vocabulary, &outside)
        .expect("assess uncovered action");
    assert_eq!(
        assessment.relations.scope,
        ScopeRelation::OutsideDeclaredScope
    );
    assert_eq!(assessment.reason_codes, vec![ReasonCode::ScopeNotCovered]);
}

#[test]
fn evaluator_reports_all_relations_without_positive_authority() {
    let fixture = fixture();
    let mut request = fixture.assessment_request;
    request.expected.bindings.context_sha256 = "a".repeat(64);
    request.expected.capability.capability_id = "different".into();
    request.expected.subject.principal_id = "different".into();
    request.expected.task_binding.task_id = "different".into();
    request.requested_action.effect_id = EffectId::RepoWrite;
    request.requested_action.usage.output_bytes = request.grant.budget.max_output_bytes + 1;
    request.evaluated_at_unix_ms = request.grant.validity.expires_at_unix_ms;
    reseal_request(&mut request);
    let assessment = evaluate_declared_assessment(&fixture.effect_vocabulary, &request)
        .expect("assess mismatches");
    assert_eq!(assessment.relations.effect, EffectRelation::EffectMismatch);
    assert_eq!(
        assessment.relations.scope,
        ScopeRelation::OutsideDeclaredScope
    );
    assert_eq!(
        assessment.authorization_decision,
        NoAuthorizationDecision::None
    );
    assert!(!assessment.permission_attestation && !assessment.effect_attestation);
    assert_eq!(
        assessment.reason_codes,
        vec![
            ReasonCode::BindingMismatch,
            ReasonCode::BudgetExceeded,
            ReasonCode::CapabilityMismatch,
            ReasonCode::EffectMismatch,
            ReasonCode::SubjectMismatch,
            ReasonCode::TaskMismatch,
            ReasonCode::TemporalWindowMismatch,
        ]
    );
}

#[test]
fn assessment_validator_reassembles_exactly_and_rejects_attestation() {
    let fixture = fixture();
    validate_assessment(
        &fixture.effect_vocabulary,
        &fixture.assessment_request,
        &fixture.expected_assessment,
    )
    .expect("validate golden assessment");

    let mut stale = fixture.expected_assessment.clone();
    stale.relations.binding = BindingRelation::BindingMismatch;
    stale.reason_codes = vec![ReasonCode::BindingMismatch];
    stale.assessment_sha256 = super::super::codec::assessment_sha256(&stale).expect("reseal");
    assert!(
        validate_assessment(
            &fixture.effect_vocabulary,
            &fixture.assessment_request,
            &stale
        )
        .is_err()
    );

    let mut escalation = fixture.expected_assessment;
    escalation.permission_attestation = true;
    escalation.assessment_sha256 =
        super::super::codec::assessment_sha256(&escalation).expect("reseal");
    assert!(canonical_assessment_json(&escalation).is_err());
}

#[test]
fn assessment_shape_binds_reason_codes_to_relations() {
    let mut assessment = fixture().expected_assessment;
    assessment.reason_codes = vec![ReasonCode::BindingMismatch];
    assessment.assessment_sha256 =
        super::super::codec::assessment_sha256(&assessment).expect("reseal assessment");
    assert!(canonical_assessment_json(&assessment).is_err());

    let mut impossible = fixture().expected_assessment;
    impossible.relations.effect = EffectRelation::EffectMismatch;
    impossible.relations.scope = ScopeRelation::DeniedByDeclaration;
    impossible.reason_codes = vec![ReasonCode::DenyMatched, ReasonCode::EffectMismatch];
    impossible.assessment_sha256 =
        super::super::codec::assessment_sha256(&impossible).expect("reseal impossible assessment");
    let canonical = super::super::canonical::encode(
        &impossible,
        MAX_ASSESSMENT_BYTES,
        "impossible declared assessment",
    )
    .expect("encode impossible assessment canonically");
    assert!(decode_canonical_assessment(canonical.as_bytes()).is_err());
}

fn set_requested_path(request: &mut DeclaredAssessmentRequest, value: &str) {
    let ScopeResource::RepoPath { path, .. } = &mut request.requested_action.resources[0] else {
        panic!("fixture repo path");
    };
    *path = value.into();
}

fn repo_resource(path: &str) -> ScopeResource {
    ScopeResource::RepoPath {
        path_match: PathMatch::Exact,
        path: path.into(),
    }
}

fn command_resource(timeout_ms: i64) -> ScopeResource {
    ScopeResource::Command {
        argv: vec!["/usr/bin/printf".into(), "ok".into()],
        cwd: ".".into(),
        environment_sha256: "d".repeat(64),
        stdin_bytes: 0,
        stdin_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
        timeout_ms,
        tool_snapshot_sha256: "e".repeat(64),
    }
}

fn network_resource(host: &str, host_kind: HostKind) -> ScopeResource {
    ScopeResource::NetworkOrigin {
        host: host.into(),
        host_kind,
        port: 443,
        scheme: NetworkScheme::Https,
    }
}
