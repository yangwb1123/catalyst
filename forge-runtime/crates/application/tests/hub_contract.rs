mod hub_support;

use std::{path::Path, sync::Arc};

use forge_runtime_application::{
    HubError, HubField, HubService, MAX_GROUP_CONTEXT_CONTENT_BYTES, MAX_GROUP_NAME_BYTES,
    MAX_IDEMPOTENCY_KEY_BYTES, MAX_PROMPT_BYTES, MAX_PROMPT_LIST_LIMIT, MAX_ROLE_BYTES,
    MAX_TITLE_BYTES,
};
use forge_runtime_domain::{ConversationScope, HubEntity, HubStore, HubStoreError};
use hub_support::MemoryHubStore;

#[test]
fn open_project_requires_a_normalized_absolute_path() {
    let service = service();

    assert!(matches!(
        service.open_project(Path::new("relative/project")),
        Err(HubError::InvalidProjectPath)
    ));
    assert!(matches!(
        service.open_project(Path::new("/workspace/../escape")),
        Err(HubError::InvalidProjectPath)
    ));
    for path in [
        "/workspace/./frontend",
        "/workspace//frontend",
        "/workspace/frontend/",
    ] {
        assert!(matches!(
            service.open_project(Path::new(path)),
            Err(HubError::InvalidProjectPath)
        ));
    }

    let project = service
        .open_project(Path::new("/workspace/frontend"))
        .expect("absolute project opens");
    assert_eq!(project.name, "frontend");
    assert_eq!(project.path, Path::new("/workspace/frontend"));
}

#[test]
fn global_and_scoped_snapshots_expose_expected_conversations() {
    let service = service();
    let frontend = open(&service, "/workspace/frontend");
    let backend = open(&service, "/workspace/backend");
    create_project_session(&service, &frontend.id, "Frontend", "session-frontend");
    create_project_session(&service, &backend.id, "Backend", "session-backend");
    service
        .create_session(&ConversationScope::Global, "Global", "session-global")
        .expect("global session");

    let global = service.global_snapshot().expect("global snapshot");
    let project = service
        .project_snapshot(&frontend.id)
        .expect("project snapshot");

    assert_eq!(global.projects.len(), 2);
    assert_eq!(global.conversations.len(), 3);
    assert_eq!(project.projects, vec![frontend]);
    assert_eq!(project.conversations.len(), 1);
    assert_eq!(project.conversations[0].title, "Frontend");
}

#[test]
fn sessions_are_conversations_and_creation_is_idempotent() {
    let service = service();
    let scope = ConversationScope::Global;
    let first = service
        .create_session(&scope, "Architecture", "create-session")
        .expect("session created");
    let replay = service
        .create_session(&scope, "Architecture", "create-session")
        .expect("idempotent replay");
    let conflict = service
        .create_session(&scope, "Different", "create-session")
        .expect_err("different replay conflicts");

    assert_eq!(first, replay);
    assert!(matches!(
        conflict,
        HubError::Store(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            ..
        })
    ));
    assert_eq!(
        service.list_sessions(&scope).expect("sessions"),
        vec![first]
    );
}

#[test]
fn global_prompt_memory_is_bounded_newest_first_and_filterable() {
    let service = service();
    let first = create_global_session(&service, "First", "first-session");
    let second = create_global_session(&service, "Second", "second-session");
    append(&service, &first.id, "user", "one", "prompt-one");
    append(&service, &second.id, "assistant", "two", "prompt-two");
    let replay = append(&service, &second.id, "assistant", "two", "prompt-two");

    let global = service.list_prompts(None, 10).expect("global prompts");
    let filtered = service
        .list_prompts(Some(&first.id), 10)
        .expect("conversation prompts");

    assert_eq!(global.len(), 2);
    assert_eq!(global[0], replay);
    assert_eq!(global[0].content, "two");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].conversation_id, first.id);
}

#[test]
fn groups_link_projects_with_collaboration_roles_not_permissions() {
    let service = service();
    let frontend = open(&service, "/workspace/frontend");
    let identity = open(&service, "/workspace/sso");
    let group = service
        .create_group("Login delivery", "create-group")
        .expect("group");
    add_member(&service, &group.id, &frontend.id, "frontend", "member-1");
    add_member(&service, &group.id, &identity.id, "sso", "member-2");
    let replay = service
        .add_project_to_group(&group.id, &identity.id, "sso", "member-2")
        .expect("idempotent member replay");
    let alias_conflict = service
        .add_project_to_group(&group.id, &identity.id, "sso", "member-alias")
        .expect_err("a successful link cannot acquire an unbound retry key");
    let conflict = service
        .add_project_to_group(&group.id, &identity.id, "backend", "member-3")
        .expect_err("one project cannot acquire a second role");

    let snapshot = service.group_snapshot(&group.id).expect("group snapshot");

    assert_eq!(service.list_groups().expect("groups"), vec![group]);
    assert_eq!(snapshot.projects.len(), 2);
    assert_eq!(snapshot.group_project_members[0].role, "frontend");
    assert_eq!(snapshot.group_project_members[1].role, "sso");
    assert_eq!(replay.role, "sso");
    assert!(matches!(
        alias_conflict,
        HubError::Store(HubStoreError::Conflict {
            entity: HubEntity::GroupProjectMember,
            ..
        })
    ));
    assert!(matches!(
        conflict,
        HubError::Store(HubStoreError::Conflict {
            entity: HubEntity::GroupProjectMember,
            ..
        })
    ));
}

#[test]
fn path_registration_and_group_linking_are_one_atomic_use_case() {
    let service = service();
    let path = Path::new("/workspace/atomic-frontend");
    let missing = service
        .add_project_path_to_group("missing", path, "frontend", "atomic-link")
        .expect_err("missing group must fail");
    assert!(matches!(
        missing,
        HubError::Store(HubStoreError::NotFound {
            entity: HubEntity::Group,
            ..
        })
    ));
    assert!(
        service
            .global_snapshot()
            .expect("global snapshot")
            .projects
            .is_empty()
    );

    let group = service
        .create_group("Atomic delivery", "atomic-group")
        .expect("group");
    let first = service
        .add_project_path_to_group(&group.id, path, "frontend", "atomic-link")
        .expect("atomic link");
    let replay = service
        .add_project_path_to_group(&group.id, path, "frontend", "atomic-link")
        .expect("atomic replay");
    assert_eq!(first, replay);
}

#[test]
fn textual_limits_are_enforced_before_storage() {
    let service = service();
    assert_empty_and_long_title(&service);
    assert_group_name_limit(&service);
    assert_prompt_limits(&service);
    assert_idempotency_limit(&service);
    assert_role_limit(&service);
}

#[test]
fn prompt_query_limit_and_optional_conversation_id_are_validated() {
    let service = service();

    assert!(matches!(
        service.list_prompts(None, 0),
        Err(HubError::OutOfRange {
            field: HubField::PromptLimit,
            min: 1,
            max: MAX_PROMPT_LIST_LIMIT
        })
    ));
    assert!(matches!(
        service.list_prompts(None, MAX_PROMPT_LIST_LIMIT + 1),
        Err(HubError::OutOfRange { .. })
    ));
    assert!(matches!(
        service.list_prompts(Some(" "), 1),
        Err(HubError::Empty {
            field: HubField::ConversationId
        })
    ));
}

#[test]
fn group_context_scope_and_byte_budget_are_validated_before_storage() {
    let service = service();
    assert!(matches!(
        service.group_context(" ", 1),
        Err(HubError::Empty {
            field: HubField::GroupId
        })
    ));
    assert!(matches!(
        service.group_context("group", 0),
        Err(HubError::OutOfRange {
            field: HubField::GroupContextBytes,
            min: 1,
            max: MAX_GROUP_CONTEXT_CONTENT_BYTES
        })
    ));
    assert!(matches!(
        service.group_context("group", MAX_GROUP_CONTEXT_CONTENT_BYTES + 1),
        Err(HubError::OutOfRange {
            field: HubField::GroupContextBytes,
            ..
        })
    ));
}

#[test]
fn structured_store_errors_remain_available_to_callers() {
    let service = service();
    let error = service
        .append_prompt("missing", "user", "hello", "missing-conversation")
        .expect_err("missing conversation");

    assert!(matches!(
        error,
        HubError::Store(HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            ref id
        }) if id == "missing"
    ));
}

fn service() -> HubService {
    let store: Arc<dyn HubStore> = MemoryHubStore::shared();
    HubService::new(store)
}

fn open(service: &HubService, path: &str) -> forge_runtime_domain::Project {
    service
        .open_project(Path::new(path))
        .expect("project opens")
}

fn create_project_session(service: &HubService, project_id: &str, title: &str, key: &str) {
    service
        .create_session(&ConversationScope::Project(project_id.into()), title, key)
        .expect("project session");
}

fn create_global_session(
    service: &HubService,
    title: &str,
    key: &str,
) -> forge_runtime_domain::Conversation {
    service
        .create_session(&ConversationScope::Global, title, key)
        .expect("global session")
}

fn append(
    service: &HubService,
    conversation_id: &str,
    role: &str,
    content: &str,
    key: &str,
) -> forge_runtime_domain::PromptRecord {
    service
        .append_prompt(conversation_id, role, content, key)
        .expect("prompt appended")
}

fn add_member(service: &HubService, group_id: &str, project_id: &str, role: &str, key: &str) {
    service
        .add_project_to_group(group_id, project_id, role, key)
        .expect("member added");
}

fn assert_empty_and_long_title(service: &HubService) {
    let scope = ConversationScope::Global;
    assert!(matches!(
        service.create_session(&scope, " ", "key"),
        Err(HubError::Empty {
            field: HubField::Title
        })
    ));
    let title = "x".repeat(MAX_TITLE_BYTES + 1);
    assert!(matches!(
        service.create_session(&scope, &title, "key"),
        Err(HubError::TooLong {
            field: HubField::Title,
            ..
        })
    ));
}

fn assert_group_name_limit(service: &HubService) {
    let name = "x".repeat(MAX_GROUP_NAME_BYTES + 1);
    assert!(matches!(
        service.create_group(&name, "key"),
        Err(HubError::TooLong {
            field: HubField::GroupName,
            ..
        })
    ));
}

fn assert_prompt_limits(service: &HubService) {
    assert!(matches!(
        service.append_prompt("conversation", "user", " ", "key"),
        Err(HubError::Empty {
            field: HubField::Prompt
        })
    ));
    let prompt = "x".repeat(MAX_PROMPT_BYTES + 1);
    assert!(matches!(
        service.append_prompt("conversation", "user", &prompt, "key"),
        Err(HubError::TooLong {
            field: HubField::Prompt,
            ..
        })
    ));
}

fn assert_idempotency_limit(service: &HubService) {
    let key = "x".repeat(MAX_IDEMPOTENCY_KEY_BYTES + 1);
    assert!(matches!(
        service.create_group("group", &key),
        Err(HubError::TooLong {
            field: HubField::IdempotencyKey,
            ..
        })
    ));
}

fn assert_role_limit(service: &HubService) {
    assert!(matches!(
        service.add_project_to_group("group", "project", " ", "key"),
        Err(HubError::Empty {
            field: HubField::Role
        })
    ));
    let role = "x".repeat(MAX_ROLE_BYTES + 1);
    assert!(matches!(
        service.add_project_to_group("group", "project", &role, "key"),
        Err(HubError::TooLong {
            field: HubField::Role,
            ..
        })
    ));
}
