use std::{collections::BTreeSet, fs};

use forge_runtime_domain::{
    BeginRun, Conversation, ConversationScope, GroupContextPolicy, HubEntity, HubStore,
    HubStoreError, Message, PROTOCOL_VERSION, Project, RUN_STORE_VERSION, RunExecution, RunLimits,
    RunOutcome, RunProvider, RunStore, RuntimeEvent, RuntimeEventKind, SessionGroup,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

struct Fixture {
    root: TempDir,
    store: SqliteHubStore,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("Group context root");
        let store =
            SqliteHubStore::open(root.path().join("state").join("hub.sqlite3")).expect("open Hub");
        Self { root, store }
    }

    fn project(&self, name: &str) -> Project {
        let path = self.root.path().join(name);
        fs::create_dir(&path).expect("project directory");
        let canonical = path.canonicalize().expect("canonical project");
        self.store.open_project(&canonical).expect("Project")
    }

    fn conversation(&self, scope: &ConversationScope, title: &str, key: &str) -> Conversation {
        self.store
            .create_conversation(scope, title, key)
            .expect("Conversation")
    }

    fn prompt(&self, conversation: &Conversation, role: &str, content: &str, key: &str) -> String {
        self.store
            .append_prompt(&conversation.id, role, content, key)
            .expect("Prompt")
            .id
    }

    fn group(&self, name: &str, key: &str) -> SessionGroup {
        self.store.create_group(name, key).expect("Group")
    }

    fn link(&self, group: &SessionGroup, project: &Project, role: &str, key: &str) {
        self.store
            .add_project_to_group(&group.id, &project.id, role, key)
            .expect("Group member");
    }

    fn conversation_prompt(
        &self,
        scope: &ConversationScope,
        title: &str,
        key: &str,
        content: &str,
    ) -> Conversation {
        let conversation = self.conversation(scope, title, key);
        self.prompt(&conversation, "user", content, &format!("{key}-prompt"));
        conversation
    }
}

struct ScopeCase {
    group: SessionGroup,
    visible_ids: BTreeSet<String>,
    frontend_path: String,
}

#[test]
fn context_is_atomic_deterministic_and_scope_isolated() {
    let fixture = Fixture::new();
    let case = seed_scope_case(&fixture);
    let first = fixture
        .store
        .load_group_context(&case.group.id, &GroupContextPolicy::default())
        .expect("Group context");
    let replay = fixture
        .store
        .load_group_context(&case.group.id, &GroupContextPolicy::default())
        .expect("deterministic context");

    assert_eq!(first, replay);
    assert_eq!(first.slice_sha256.len(), 64);
    assert_eq!(first.payload.members.len(), 1);
    assert_eq!(conversation_ids(&first), case.visible_ids);
    let json = serde_json::to_string(&first).expect("context JSON");
    assert!(json.contains("group-visible"));
    assert!(json.contains("frontend-visible"));
    assert!(!json.contains(&case.frontend_path));
    for forbidden in [
        "outsider-secret",
        "global-secret",
        "other-group-secret",
        "secret-group-key",
        "secret-member-key",
    ] {
        assert!(!json.contains(forbidden), "leaked {forbidden}");
    }
}

fn seed_scope_case(fixture: &Fixture) -> ScopeCase {
    let frontend = fixture.project("private-frontend-path");
    let outsider = fixture.project("private-outsider-path");
    let group = fixture.group("SSO delivery", "secret-group-key");
    let other_group = fixture.group("Other", "other-group-key");
    fixture.link(&group, &frontend, "frontend", "secret-member-key");
    let group_session = fixture.conversation_prompt(
        &ConversationScope::Group(group.id.clone()),
        "Group discussion",
        "group-session-key",
        "group-visible",
    );
    let frontend_session = fixture.conversation_prompt(
        &ConversationScope::Project(frontend.id.clone()),
        "Frontend",
        "frontend-session-key",
        "frontend-visible",
    );
    fixture.conversation_prompt(
        &ConversationScope::Project(outsider.id),
        "Outsider",
        "outsider-session-key",
        "outsider-secret",
    );
    fixture.conversation_prompt(
        &ConversationScope::Global,
        "Global",
        "global-session-key",
        "global-secret",
    );
    fixture.conversation_prompt(
        &ConversationScope::Group(other_group.id),
        "Other group",
        "other-group-session-key",
        "other-group-secret",
    );
    ScopeCase {
        group,
        visible_ids: BTreeSet::from([group_session.id, frontend_session.id]),
        frontend_path: frontend.path.display().to_string(),
    }
}

#[test]
fn content_budget_is_round_robin_and_utf8_safe() {
    let fixture = Fixture::new();
    let backend = fixture.project("backend");
    let frontend = fixture.project("frontend");
    let group = fixture.group("Delivery", "group-key");
    fixture.link(&group, &backend, "backend", "backend-link");
    fixture.link(&group, &frontend, "frontend", "frontend-link");
    let back_session = fixture.conversation(
        &ConversationScope::Project(backend.id),
        "Backend",
        "backend-session",
    );
    let front_session = fixture.conversation(
        &ConversationScope::Project(frontend.id),
        "Frontend",
        "frontend-session",
    );
    fixture.prompt(&back_session, "user", "你abcdef", "backend-prompt");
    fixture.prompt(&front_session, "user", "好ghijkl", "frontend-prompt");
    let policy = GroupContextPolicy {
        max_prompt_excerpt_bytes: 4,
        max_total_content_bytes: 8,
        ..GroupContextPolicy::default()
    };

    let context = fixture
        .store
        .load_group_context(&group.id, &policy)
        .expect("bounded context");
    let excerpts: Vec<_> = context
        .payload
        .conversations
        .iter()
        .map(|item| item.prompts[0].excerpt.as_str())
        .collect();

    assert_eq!(excerpts, ["你a", "好g"]);
    assert_eq!(context.payload.stats.content_bytes, 8);
    assert_eq!(context.payload.stats.truncated_prompt_count, 2);
}

#[test]
fn conversation_and_prompt_omissions_are_explicit() {
    let fixture = Fixture::new();
    let project = fixture.project("api");
    let group = fixture.group("API", "group-key");
    fixture.link(&group, &project, "backend", "member-key");
    let mut conversations = Vec::new();
    for index in 0..3 {
        let conversation = fixture.conversation(
            &ConversationScope::Project(project.id.clone()),
            &format!("Session {index}"),
            &format!("session-{index}"),
        );
        fixture.prompt(
            &conversation,
            "user",
            &format!("prompt-{index}"),
            &format!("prompt-{index}"),
        );
        conversations.push(conversation);
    }
    let latest = conversations.last().expect("latest Conversation");
    for index in 0..9 {
        fixture.prompt(
            latest,
            "user",
            &format!("latest-{index}"),
            &format!("latest-key-{index}"),
        );
    }

    let context = fixture
        .store
        .load_group_context(&group.id, &GroupContextPolicy::default())
        .expect("Group context");

    assert_eq!(context.payload.conversations.len(), 2);
    assert_eq!(context.payload.stats.omitted_conversation_count, 1);
    let selected_latest = context
        .payload
        .conversations
        .iter()
        .find(|item| item.conversation.id == latest.id)
        .expect("latest selected");
    assert_eq!(selected_latest.prompts.len(), 8);
    assert_eq!(selected_latest.omitted_prompt_count, 2);
    assert_eq!(context.payload.stats.omitted_prompt_count, 2);
}

#[test]
fn delayed_run_answer_stays_with_its_source_prompt() {
    let fixture = Fixture::new();
    let project = fixture.project("identity");
    let group = fixture.group("Identity", "group-key");
    fixture.link(&group, &project, "sso", "member-key");
    let conversation = fixture.conversation(
        &ConversationScope::Project(project.id.clone()),
        "Identity",
        "conversation-key",
    );
    let source = fixture.prompt(&conversation, "user", "source", "source-key");
    let begin = begin_run(&project, &conversation, &source);
    fixture.store.begin_run(&begin).expect("begin Run");
    let newer = fixture.prompt(&conversation, "user", "newer", "newer-key");
    append_completed(&fixture.store, &conversation.id, &begin.run_id, "source");
    let answer = fixture
        .store
        .reconcile_completed_assistant(&begin.run_id)
        .expect("assistant writeback");

    let context = fixture
        .store
        .load_group_context(&group.id, &GroupContextPolicy::default())
        .expect("Group context");
    let ids: Vec<_> = context.payload.conversations[0]
        .prompts
        .iter()
        .map(|prompt| prompt.id.as_str())
        .collect();

    assert_eq!(ids, [source, answer.id, newer]);
}

#[test]
fn causal_content_budget_keeps_source_before_its_run_answer() {
    let fixture = Fixture::new();
    let project = fixture.project("causal-budget");
    let group = fixture.group("Causal budget", "group-key");
    fixture.link(&group, &project, "backend", "member-key");
    let conversation = fixture.conversation(
        &ConversationScope::Project(project.id.clone()),
        "Causal budget",
        "conversation-key",
    );
    let source = fixture.prompt(&conversation, "user", "source", "source-key");
    let begin = begin_run(&project, &conversation, &source);
    fixture.store.begin_run(&begin).expect("begin Run");
    append_completed(&fixture.store, &conversation.id, &begin.run_id, "source");
    let answer = fixture
        .store
        .reconcile_completed_assistant(&begin.run_id)
        .expect("assistant writeback");
    let policy = GroupContextPolicy {
        max_total_content_bytes: 1,
        ..GroupContextPolicy::default()
    };

    let context = fixture
        .store
        .load_group_context(&group.id, &policy)
        .expect("bounded Group context");
    let prompts = &context.payload.conversations[0].prompts;

    assert_eq!(prompts[0].id, source);
    assert_eq!(prompts[0].excerpt, "s");
    assert_eq!(prompts[1].id, answer.id);
    assert!(prompts[1].excerpt.is_empty());
}

#[test]
fn utf8_source_without_an_excerpt_blocks_its_run_answer() {
    let fixture = Fixture::new();
    let project = fixture.project("utf8-causal-budget");
    let group = fixture.group("UTF-8 causal budget", "group-key");
    fixture.link(&group, &project, "backend", "member-key");
    let conversation = fixture.conversation(
        &ConversationScope::Project(project.id.clone()),
        "UTF-8 causal budget",
        "conversation-key",
    );
    let source = fixture.prompt(&conversation, "user", "你source", "source-key");
    let begin = begin_run(&project, &conversation, &source);
    fixture.store.begin_run(&begin).expect("begin Run");
    append_completed(&fixture.store, &conversation.id, &begin.run_id, "你source");
    fixture
        .store
        .reconcile_completed_assistant(&begin.run_id)
        .expect("assistant writeback");
    let policy = GroupContextPolicy {
        max_total_content_bytes: 1,
        ..GroupContextPolicy::default()
    };

    let context = fixture
        .store
        .load_group_context(&group.id, &policy)
        .expect("bounded Group context");
    let prompts = &context.payload.conversations[0].prompts;

    assert!(prompts[0].excerpt.is_empty());
    assert!(prompts[1].excerpt.is_empty());
    assert_eq!(context.payload.stats.content_bytes, 0);
}

#[test]
fn invalid_roles_and_member_overflow_fail_closed() {
    let fixture = Fixture::new();
    let first = fixture.project("first");
    let second = fixture.project("second");
    let group = fixture.group("Strict", "group-key");
    fixture.link(&group, &first, "first", "first-link");
    fixture.link(&group, &second, "second", "second-link");
    let conversation = fixture.conversation(
        &ConversationScope::Project(first.id),
        "Strict",
        "conversation-key",
    );
    fixture.prompt(&conversation, "system", "not allowed", "system-key");
    for index in 0..GroupContextPolicy::default().max_prompts_per_conversation {
        fixture.prompt(
            &conversation,
            "user",
            &format!("newer-{index}"),
            &format!("newer-key-{index}"),
        );
    }
    assert!(matches!(
        fixture
            .store
            .load_group_context(&group.id, &GroupContextPolicy::default()),
        Err(HubStoreError::Corrupt { .. })
    ));
    let policy = GroupContextPolicy {
        max_members: 1,
        ..GroupContextPolicy::default()
    };
    assert!(matches!(
        fixture.store.load_group_context(&group.id, &policy),
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupProjectMember,
            ..
        })
    ));
}

fn conversation_ids(context: &forge_runtime_domain::GroupContextSlice) -> BTreeSet<String> {
    context
        .payload
        .conversations
        .iter()
        .map(|item| item.conversation.id.clone())
        .collect()
}

fn begin_run(project: &Project, conversation: &Conversation, prompt_id: &str) -> BeginRun {
    BeginRun {
        v: RUN_STORE_VERSION,
        run_id: "context-run".into(),
        conversation_id: conversation.id.clone(),
        prompt_id: prompt_id.into(),
        project_id: project.id.clone(),
        execution: RunExecution {
            provider: RunProvider::DeterministicRead {
                path: "README.md".into(),
            },
            system_prompt: "test".into(),
            allowed_read_paths: vec!["README.md".into()],
            limits: RunLimits::default(),
        },
        idempotency_key: "context-run-key".into(),
        created_at_ms: 1,
    }
}

fn append_completed(store: &SqliteHubStore, conversation_id: &str, run_id: &str, prompt: &str) {
    let kinds = [
        RuntimeEventKind::RunStarted {
            prompt: prompt.into(),
        },
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: prompt.into(),
            },
        },
        RuntimeEventKind::TurnStarted { turn: 1 },
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant {
                text: "answer".into(),
                tool_calls: Vec::new(),
            },
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: "answer".into(),
            },
        },
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        store
            .append_event(&runtime_event(conversation_id, run_id, index, kind))
            .expect("append Run event");
    }
}

fn runtime_event(
    conversation_id: &str,
    run_id: &str,
    index: usize,
    kind: RuntimeEventKind,
) -> RuntimeEvent {
    RuntimeEvent {
        v: PROTOCOL_VERSION,
        session_id: conversation_id.into(),
        run_id: run_id.into(),
        seq: u64::try_from(index + 1).expect("sequence"),
        emitted_at_ms: u64::try_from(index + 1).expect("time"),
        kind,
    }
}
