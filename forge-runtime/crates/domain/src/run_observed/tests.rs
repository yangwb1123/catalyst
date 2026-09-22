use super::*;
use serde::Deserialize;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-run-observed-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    api_version: String,
    owner_ref: String,
    conversation_id: String,
    run_id: String,
    prompt_id: String,
    created_at_ms: u64,
    latest_sequence: u64,
    status: String,
    metadata_observed: bool,
    content_included: bool,
    authority: FixtureAuthority,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureAuthority {
    identity_verified: bool,
    owner_authorized: bool,
    run_authoritative: bool,
    persistence_attested: bool,
    content_provenance_verified: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[test]
fn run_observed_fixture_matches_the_pure_domain_value() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("strict fixture");
    let observed = observe_run(fixture_input()).expect("valid metadata");

    assert_eq!(observed.api_version, fixture.api_version);
    assert_eq!(observed.owner_ref, fixture.owner_ref);
    assert_eq!(observed.conversation_id, fixture.conversation_id);
    assert_eq!(observed.run_id, fixture.run_id);
    assert_eq!(observed.prompt_id, fixture.prompt_id);
    assert_eq!(observed.created_at_ms, fixture.created_at_ms);
    assert_eq!(observed.latest_sequence, fixture.latest_sequence);
    assert_eq!(observed.status, fixture.status);
    assert_eq!(observed.metadata_observed, fixture.metadata_observed);
    assert_eq!(observed.content_included, fixture.content_included);
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.owner_authorized);
    assert!(!fixture.authority.run_authoritative);
    assert!(!fixture.authority.persistence_attested);
    assert!(!fixture.authority.content_provenance_verified);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(observed.authority, RunObservedAuthority::default());
}

#[test]
fn run_observed_fixture_has_no_content_or_raw_owner_fields() {
    for forbidden in [
        "content",
        "prompt_content",
        "message",
        "output",
        "tool",
        "error",
        "title",
        "path",
        "provider",
        "token",
        "issuer",
        "subject",
        "tenant_id",
        "authorization",
        "execution",
        "dispatch",
        "outbox",
    ] {
        assert!(
            !FIXTURE.contains(&format!("\"{forbidden}\"")),
            "fixture leaked {forbidden}"
        );
    }
}

#[test]
fn run_observed_rejects_unbounded_or_missing_metadata() {
    let mut unsafe_sequence = fixture_input();
    unsafe_sequence.run.latest_sequence = 0;
    assert_eq!(
        observe_run(unsafe_sequence),
        Err(RunObservedError::InvalidMetadata)
    );

    let mut unsafe_timestamp = fixture_input();
    unsafe_timestamp.run.created_at_ms = MAX_RUN_OBSERVED_SAFE_INTEGER + 1;
    assert_eq!(
        observe_run(unsafe_timestamp),
        Err(RunObservedError::InvalidMetadata)
    );

    let mut foreign_shape = fixture_input();
    foreign_shape.conversation_id = "conversation/001".into();
    assert_eq!(
        observe_run(foreign_shape),
        Err(RunObservedError::InvalidMetadata)
    );
}

fn fixture_input() -> RunObservedInput {
    RunObservedInput {
        owner: ConversationOwner {
            issuer: "https://id.example".into(),
            subject: "user-1".into(),
            tenant_id: "tenant-1".into(),
        },
        conversation_id: "conversation-001".into(),
        run: OwnedRunSummary {
            run_id: "run-001".into(),
            prompt_id: "prompt-001".into(),
            created_at_ms: 200,
            latest_sequence: 5,
            status: OwnedRunStatus::Nonterminal,
        },
    }
}
