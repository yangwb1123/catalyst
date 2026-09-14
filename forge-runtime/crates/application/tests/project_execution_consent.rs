use std::sync::Arc;

use forge_runtime_application::{HubError, HubService};
use forge_runtime_domain::{
    Conversation, ConversationOwner, ConversationScope, HubEntity, HubStore, HubStoreError,
    MAX_PROJECT_EXECUTION_CONSENT_TTL_MS, PendingRunIntentStatus, PendingRunIntentSubmissionResult,
    PendingRunIntentTimelineEventType, ProjectExecutionConsentGrant,
    ProjectExecutionConsentGrantResult, SubmitPendingRunIntent,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

#[test]
fn project_consent_replays_exactly_and_revoke_is_bound_to_the_full_owner_tuple() {
    let fixture = ConsentFixture::new();
    let owner_a = owner("issuer-a", "subject-a", "tenant-a");
    let owner_b = owner("issuer-a", "subject-b", "tenant-a");
    let profile_digest = [0x97; 32];
    let expires_at_ms = unix_time_ms() + 60_000;

    let first = assert_grant_replays_exactly(&fixture, &owner_a, &profile_digest, expires_at_ms);
    assert_changed_grant_replays_conflict(&fixture, &owner_a, &profile_digest, expires_at_ms);
    assert_same_key_is_namespaced_to_owner(&fixture, &owner_b, &first, expires_at_ms);
    assert_owner_scoped_revoke_and_regrant(
        &fixture,
        &owner_a,
        &first,
        &profile_digest,
        expires_at_ms,
    );
}

fn assert_grant_replays_exactly(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    profile_digest: &[u8; 32],
    expires_at_ms: u64,
) -> ProjectExecutionConsentGrant {
    let first = grant_consent(
        fixture,
        owner,
        &fixture.project_id,
        "profile-opaque-a",
        profile_digest,
        expires_at_ms,
        "same-client-key",
    )
    .expect("create owner grant");
    assert!(!first.replayed);
    assert_eq!(first.grant.profile_id, "profile-opaque-a");
    assert_eq!(first.grant.profile_sha256, *profile_digest);

    let replay = grant_consent(
        fixture,
        owner,
        &fixture.project_id,
        "profile-opaque-a",
        profile_digest,
        expires_at_ms,
        "same-client-key",
    )
    .expect("replay exact grant");
    assert!(replay.replayed);
    assert_eq!(replay.grant, first.grant);
    first.grant
}

fn assert_changed_grant_replays_conflict(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    profile_digest: &[u8; 32],
    expires_at_ms: u64,
) {
    for (project_id, profile_id, digest, expiry) in [
        (
            fixture.other_project_id.as_str(),
            "profile-opaque-a",
            *profile_digest,
            expires_at_ms,
        ),
        (
            fixture.project_id.as_str(),
            "profile-opaque-b",
            *profile_digest,
            expires_at_ms,
        ),
        (
            fixture.project_id.as_str(),
            "profile-opaque-a",
            [0x98; 32],
            expires_at_ms,
        ),
        (
            fixture.project_id.as_str(),
            "profile-opaque-a",
            *profile_digest,
            expires_at_ms + 1,
        ),
    ] {
        let mismatch = grant_consent(
            fixture,
            owner,
            project_id,
            profile_id,
            &digest,
            expiry,
            "same-client-key",
        )
        .expect_err("same key with changed immutable field must conflict");
        assert!(matches!(
            mismatch,
            HubError::Store(HubStoreError::Conflict { .. })
        ));
    }
}

fn assert_same_key_is_namespaced_to_owner(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    first: &ProjectExecutionConsentGrant,
    expires_at_ms: u64,
) {
    let owner_grant = grant_consent(
        fixture,
        owner,
        &fixture.project_id,
        "profile-opaque-b",
        &[0x98; 32],
        expires_at_ms,
        "same-client-key",
    )
    .expect("the raw key is namespaced to the exact owner");
    assert_ne!(owner_grant.grant.grant_id, first.grant_id);
}

fn assert_owner_scoped_revoke_and_regrant(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    first: &ProjectExecutionConsentGrant,
    profile_digest: &[u8; 32],
    expires_at_ms: u64,
) {
    assert_foreign_owners_cannot_revoke(fixture, first);
    let revoked = fixture
        .service
        .revoke_project_execution_consent(owner, &first.grant_id, "revoke-key-a")
        .expect("owner revokes its grant");
    assert!(!revoked.replayed);
    let revoke_replay = fixture
        .service
        .revoke_project_execution_consent(owner, &first.grant_id, "revoke-key-a")
        .expect("exact owner/key/grant revoke replay");
    assert!(revoke_replay.replayed);
    assert_eq!(revoke_replay.revocation, revoked.revocation);

    let regrant = grant_consent(
        fixture,
        owner,
        &fixture.project_id,
        "profile-opaque-a",
        profile_digest,
        expires_at_ms,
        "new-grant-key",
    )
    .expect("revocation permits a new grant identity");
    assert_ne!(regrant.grant.grant_id, first.grant_id);
    let mismatched_replay = fixture
        .service
        .revoke_project_execution_consent(owner, &regrant.grant.grant_id, "revoke-key-a")
        .expect_err("revoke key cannot replay against a different grant");
    assert!(matches!(
        mismatched_replay,
        HubError::Store(HubStoreError::Conflict { .. })
    ));
}

fn assert_foreign_owners_cannot_revoke(
    fixture: &ConsentFixture,
    first: &ProjectExecutionConsentGrant,
) {
    for foreign_owner in [
        owner("issuer-b", "subject-a", "tenant-a"),
        owner("issuer-a", "subject-b", "tenant-a"),
        owner("issuer-a", "subject-a", "tenant-b"),
    ] {
        let denied = fixture
            .service
            .revoke_project_execution_consent(&foreign_owner, &first.grant_id, "revoke-key-a")
            .expect_err("all owner tuple components are required");
        assert!(matches!(
            denied,
            HubError::Store(HubStoreError::NotFound { .. })
        ));
    }
}

#[test]
fn project_consent_requires_an_existing_project_and_bounded_future_expiry() {
    let fixture = ConsentFixture::new();
    let owner = owner("issuer", "subject", "tenant");
    let expiry = unix_time_ms() + 60_000;
    assert_missing_project_is_rejected(&fixture, &owner, expiry);
    assert_invalid_expiries_conflict(&fixture, &owner);
}

#[path = "project_execution_consent/pending_intents.rs"]
mod pending_intents;

fn assert_missing_project_is_rejected(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    expiry: u64,
) {
    let missing_project = grant_consent(
        fixture,
        owner,
        "project-that-does-not-exist",
        "opaque-profile",
        &[0x11; 32],
        expiry,
        "missing-project",
    )
    .expect_err("unknown Project rejected by Hub transaction");
    assert!(matches!(
        missing_project,
        HubError::Store(HubStoreError::NotFound {
            entity: HubEntity::Project,
            ..
        })
    ));
}

fn assert_invalid_expiries_conflict(fixture: &ConsentFixture, owner: &ConversationOwner) {
    let expired = grant_consent(
        fixture,
        owner,
        &fixture.project_id,
        "opaque-profile",
        &[0x11; 32],
        unix_time_ms().saturating_sub(1),
        "already-expired",
    )
    .expect_err("expiry must be in the future");
    assert!(matches!(
        expired,
        HubError::Store(HubStoreError::Conflict { .. })
    ));

    let overlong = grant_consent(
        fixture,
        owner,
        &fixture.project_id,
        "opaque-profile",
        &[0x11; 32],
        unix_time_ms() + MAX_PROJECT_EXECUTION_CONSENT_TTL_MS + 60_000,
        "overlong-expiry",
    )
    .expect_err("consent lifetime has a thirty-day maximum");
    assert!(matches!(
        overlong,
        HubError::Store(HubStoreError::Conflict { .. })
    ));
}

fn grant_consent(
    fixture: &ConsentFixture,
    owner: &ConversationOwner,
    project_id: &str,
    profile_id: &str,
    profile_sha256: &[u8; 32],
    expires_at_ms: u64,
    idempotency_key: &str,
) -> Result<ProjectExecutionConsentGrantResult, HubError> {
    fixture.service.grant_project_execution_consent(
        owner,
        project_id,
        profile_id,
        profile_sha256,
        expires_at_ms,
        idempotency_key,
    )
}

struct ConsentFixture {
    _temporary_directory: TempDir,
    service: HubService,
    project_id: String,
    other_project_id: String,
}

impl ConsentFixture {
    fn new() -> Self {
        let temporary_directory = tempfile::tempdir().expect("temporary consent fixture");
        restrict_fixture_root(&temporary_directory);
        let database = temporary_directory.path().join("hub.sqlite3");
        let project_path = temporary_directory.path().join("project");
        std::fs::create_dir(&project_path).expect("create Project directory");
        let project_path = project_path.canonicalize().expect("canonical Project path");
        let store = Arc::new(SqliteHubStore::open(&database).expect("open Hub"));
        let project = store.open_project(&project_path).expect("register Project");
        let other_path = temporary_directory.path().join("other-project");
        std::fs::create_dir(&other_path).expect("create second Project directory");
        let other_path = other_path
            .canonicalize()
            .expect("canonical second Project path");
        let other_project = store
            .open_project(&other_path)
            .expect("register second Project");
        Self {
            _temporary_directory: temporary_directory,
            service: HubService::new(store),
            project_id: project.id,
            other_project_id: other_project.id,
        }
    }
}

fn owner(issuer: &str, subject: &str, tenant_id: &str) -> ConversationOwner {
    ConversationOwner {
        issuer: issuer.into(),
        subject: subject.into(),
        tenant_id: tenant_id.into(),
    }
}

fn unix_time_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after UNIX epoch")
        .as_millis()
        .try_into()
        .expect("milliseconds fit in u64")
}

#[cfg(unix)]
fn restrict_fixture_root(root: &TempDir) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private consent fixture root");
}

#[cfg(not(unix))]
fn restrict_fixture_root(_root: &TempDir) {}
