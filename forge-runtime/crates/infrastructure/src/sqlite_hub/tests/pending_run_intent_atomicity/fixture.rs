use crate::runtime_domain::{
    ConversationOwner, ConversationScope, HubStore, HubStoreError,
    MAX_PROJECT_EXECUTION_CONSENT_TTL_MS, SubmitPendingRunIntent,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

use super::{PROFILE_DIGEST, PROFILE_ID, SubmitInput, count};
use crate::sqlite_hub::rows;

pub(super) struct Fixture {
    pub(super) _root: TempDir,
    pub(super) store: SqliteHubStore,
    pub(super) owner: ConversationOwner,
    pub(super) project_id: String,
    pub(super) conversation_id: String,
    pub(super) grant_id: String,
    pub(super) grant_expiry: u64,
}

impl Fixture {
    pub(super) fn new(with_grant: bool) -> Self {
        let (root, store, owner, project_id, conversation_id) = fixture_core();
        let granted_at = rows::now_ms().expect("read clock");
        let grant_expiry = granted_at + MAX_PROJECT_EXECUTION_CONSENT_TTL_MS;
        let grant_id = fixture_grant(&store, &owner, &project_id, grant_expiry, with_grant);
        Self {
            _root: root,
            store,
            owner,
            project_id,
            conversation_id,
            grant_id,
            grant_expiry,
        }
    }

    pub(super) fn input<'a>(
        &'a self,
        key: &'a str,
        expected_version: u64,
        content: &'a str,
    ) -> SubmitInput<'a> {
        SubmitInput {
            owner: &self.owner,
            conversation_id: &self.conversation_id,
            content,
            idempotency_key: key,
            expected_version,
            profile_id: PROFILE_ID,
            profile_sha256: &PROFILE_DIGEST,
        }
    }

    pub(super) fn connection(&self) -> rusqlite::Connection {
        self.store.connect().expect("open Hub connection")
    }

    pub(super) fn submit(
        &self,
        owner: &ConversationOwner,
        submission: SubmitPendingRunIntent<'_>,
    ) -> Result<forge_runtime_domain::PendingRunIntentSubmissionResult, HubStoreError> {
        self.store
            .submit_owned_prompt_run_intent(owner, &submission)
    }

    pub(super) fn count(&self, table: &str) -> i64 {
        count(&self.connection(), table)
    }

    pub(super) fn create_owned_conversation(&self, key: &str) -> String {
        self.create_owned_conversation_with_scope(
            key,
            &ConversationScope::Project(self.project_id.clone()),
        )
    }

    pub(super) fn create_owned_conversation_with_scope(
        &self,
        key: &str,
        scope: &ConversationScope,
    ) -> String {
        self.store
            .create_owned_conversation(&self.owner, scope, key, key)
            .expect("create another owned Conversation")
            .id
    }
}

fn fixture_core() -> (TempDir, SqliteHubStore, ConversationOwner, String, String) {
    let root = tempfile::tempdir().expect("create Hub fixture root");
    let database = root.path().join("state").join("hub.sqlite3");
    let store = SqliteHubStore::open(&database).expect("create Hub");
    let project_directory = root.path().join("project");
    std::fs::create_dir(&project_directory).expect("create project directory");
    let project_path = project_directory
        .canonicalize()
        .expect("canonicalize project directory");
    let project = store.open_project(&project_path).expect("register Project");
    let owner = ConversationOwner {
        issuer: "https://issuer.example".into(),
        subject: "account-1".into(),
        tenant_id: "tenant-1".into(),
    };
    let conversation = store
        .create_owned_conversation(
            &owner,
            &ConversationScope::Project(project.id.clone()),
            "project session",
            "conversation-key",
        )
        .expect("create owned Project Conversation");
    (root, store, owner, project.id, conversation.id)
}

fn fixture_grant(
    store: &SqliteHubStore,
    owner: &ConversationOwner,
    project_id: &str,
    grant_expiry: u64,
    with_grant: bool,
) -> String {
    if !with_grant {
        return String::new();
    }
    store
        .grant_project_execution_consent(
            owner,
            project_id,
            PROFILE_ID,
            &PROFILE_DIGEST,
            grant_expiry,
            "grant-key",
        )
        .expect("grant exact Project profile consent")
        .grant
        .grant_id
}
