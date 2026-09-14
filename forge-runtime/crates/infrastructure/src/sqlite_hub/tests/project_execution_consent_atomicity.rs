use crate::runtime_domain::{ConversationOwner, HubEntity, HubStore, HubStoreError};
use tempfile::TempDir;

use crate::sqlite_hub::{
    SqliteHubStore,
    project_execution_consent::{GrantInput, grant_with_clock_and_hook},
};

#[test]
fn expired_grants_release_the_exact_owner_project_pair_for_a_new_identity() {
    let (_root, store, owner, project_id) = fixture();
    let mut connection = store.connect().expect("open Hub connection");
    let first = grant_at(
        &mut connection,
        grant_input(
            &owner,
            &project_id,
            "profile-opaque-a",
            &[0x3a; 32],
            2_000,
            "grant-before-expiry",
        ),
        1_000,
    )
    .expect("create first grant");

    assert_active_grant_blocks_replacement(&mut connection, &owner, &project_id);
    assert_expiry_allows_replacement(&mut connection, &owner, &project_id, &first.grant.grant_id);
}

fn assert_active_grant_blocks_replacement(
    connection: &mut rusqlite::Connection,
    owner: &ConversationOwner,
    project_id: &str,
) {
    let error = grant_at(
        connection,
        grant_input(
            owner,
            project_id,
            "profile-opaque-b",
            &[0x4b; 32],
            3_000,
            "grant-at-1999",
        ),
        1_999,
    )
    .expect_err("existing unexpired grant blocks another profile");
    assert!(matches!(
        error,
        HubStoreError::Conflict {
            entity: HubEntity::ProjectExecutionConsent,
            ..
        }
    ));
}

fn assert_expiry_allows_replacement(
    connection: &mut rusqlite::Connection,
    owner: &ConversationOwner,
    project_id: &str,
    first_grant_id: &str,
) {
    let replacement = grant_at(
        connection,
        grant_input(
            owner,
            project_id,
            "profile-opaque-b",
            &[0x4b; 32],
            3_000,
            "grant-at-expiry",
        ),
        2_000,
    )
    .expect("an expired grant permits a new identity");
    assert_ne!(first_grant_id, replacement.grant.grant_id);
    assert_eq!(replacement.grant.profile_id, "profile-opaque-b");
    assert_eq!(replacement.grant.profile_sha256, [0x4b; 32]);
    assert_eq!(
        replacement.grant.expires_at_ms - replacement.grant.granted_at_ms,
        1_000
    );
}

#[test]
fn insert_fault_rolls_back_grant_and_its_initial_event_together() {
    let (_root, store, owner, project_id) = fixture();
    let mut connection = store.connect().expect("open Hub connection");
    let injected = grant_with_clock_and_hook(
        &mut connection,
        grant_input(
            &owner,
            &project_id,
            "profile-opaque",
            &[0x5c; 32],
            2_000,
            "grant-rollback-key",
        ),
        || Ok(1_000),
        || {
            Err(HubStoreError::Conflict {
                entity: HubEntity::ProjectExecutionConsent,
                message: "test fault after grant row".into(),
            })
        },
    )
    .expect_err("injected failure aborts the transaction");
    assert!(matches!(
        injected,
        HubStoreError::Conflict {
            entity: HubEntity::ProjectExecutionConsent,
            ..
        }
    ));
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM project_execution_consent_grants",
                [],
                |row| { row.get::<_, i64>(0) }
            )
            .expect("count rolled-back grants"),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM project_execution_consent_events",
                [],
                |row| { row.get::<_, i64>(0) }
            )
            .expect("count rolled-back events"),
        0
    );
}

fn grant_at(
    connection: &mut rusqlite::Connection,
    input: GrantInput<'_>,
    now_ms: u64,
) -> Result<forge_runtime_domain::ProjectExecutionConsentGrantResult, HubStoreError> {
    grant_with_clock_and_hook(connection, input, || Ok(now_ms), || Ok(()))
}

fn grant_input<'a>(
    owner: &'a ConversationOwner,
    project_id: &'a str,
    profile_id: &'a str,
    profile_sha256: &'a [u8; 32],
    expires_at_ms: u64,
    idempotency_key: &'a str,
) -> GrantInput<'a> {
    GrantInput {
        owner,
        project_id,
        profile_id,
        profile_sha256,
        expires_at_ms,
        idempotency_key,
    }
}

fn fixture() -> (TempDir, SqliteHubStore, ConversationOwner, String) {
    let root = tempfile::tempdir().expect("temporary consent fixture");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let project_directory = root.path().join("project");
    std::fs::create_dir(&project_directory).expect("create Project directory");
    let project_path = project_directory
        .canonicalize()
        .expect("canonicalize Project directory");
    let store = SqliteHubStore::open(&database).expect("open Hub");
    let project = store.open_project(&project_path).expect("register Project");
    let owner = ConversationOwner {
        issuer: "https://issuer.example.test".into(),
        subject: "subject-a".into(),
        tenant_id: "tenant-blue".into(),
    };
    (root, store, owner, project.id)
}

#[cfg(unix)]
fn restrict_fixture_root(root: &TempDir) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private consent fixture root");
}

#[cfg(not(unix))]
fn restrict_fixture_root(_root: &TempDir) {}
