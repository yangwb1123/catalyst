use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    run_status: String,
    run_state_admissible: bool,
    attempt_id: String,
    attempt_state: String,
    attempt_state_admissible: bool,
    command_id: String,
    intent_target_id: String,
    lease_epoch: u64,
    lease_active: bool,
    evaluated_at_ms: u64,
    candidate_count: u32,
    declarative_ready_count: u32,
    declarative_preflight_ready: bool,
    rejection_reasons: Vec<String>,
    selected_target_id: Option<String>,
    preview_only: bool,
    authority: Authority,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    identity_verified: bool,
    run_authoritative: bool,
    attempt_persisted: bool,
    lease_issued: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[test]
fn run_attempt_lease_dispatch_preflight_fixture_is_metadata_only() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("strict preflight fixture");
    assert_eq!(
        fixture.schema_version,
        "forge.run-attempt-lease-dispatch-preflight/v1"
    );
    assert_eq!(
        fixture.evaluation_mode,
        "pure_run_attempt_lease_dispatch_preflight"
    );
    assert!(!fixture.owner.issuer.is_empty());
    assert!(!fixture.owner.subject.is_empty());
    assert!(!fixture.owner.tenant_id.is_empty());
    assert!(!fixture.conversation_id.is_empty());
    assert!(!fixture.run_id.is_empty());
    assert_eq!(fixture.run_status, "nonterminal");
    assert!(fixture.run_state_admissible);
    assert!(!fixture.attempt_id.is_empty());
    assert_eq!(fixture.attempt_state, "accepted");
    assert!(fixture.attempt_state_admissible);
    assert!(!fixture.command_id.is_empty());
    assert!(!fixture.intent_target_id.is_empty());
    assert!(fixture.lease_epoch > 0);
    assert!(fixture.lease_active);
    assert!(fixture.evaluated_at_ms > 0);
    assert_eq!(fixture.candidate_count, 2);
    assert_eq!(fixture.declarative_ready_count, 1);
    assert!(fixture.declarative_preflight_ready);
    assert!(fixture.rejection_reasons.is_empty());
    assert!(fixture.selected_target_id.is_none());
    assert!(fixture.preview_only);
    assert_authority_is_false(fixture.authority);
}

fn assert_authority_is_false(authority: Authority) {
    assert!(!authority.identity_verified);
    assert!(!authority.run_authoritative);
    assert!(!authority.attempt_persisted);
    assert!(!authority.lease_issued);
    assert!(!authority.reservation_created);
    assert!(!authority.execution_authorized);
    assert!(!authority.dispatch_performed);
    assert!(!authority.audit_published);
}
