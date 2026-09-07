#![cfg(unix)]

use std::{fs, process::Command, sync::Arc};

use forge_runtime_domain::{
    BeginRun, CURRENT_AGENT_TOOLSET_VERSION, ConversationScope, HubStore, Message,
    PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits, RunOutcome, RunProvider,
    RunStore, RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::{CapStdAgentWorkspace, SqliteHubStore};
use tempfile::TempDir;

const RUN_ID: &str = "completed-agent-run";

#[test]
fn completed_agent_resume_reconciles_without_credentials_or_new_events() {
    let fixture = Fixture::new();
    let before_events = fixture.event_count();

    let output = fixture.resume();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(fixture.event_count(), before_events);
    let prompts = fixture.prompts();
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0].role, "assistant");
    assert_eq!(prompts[0].content, "cached Agent answer");
}

#[test]
fn completed_agent_resume_rejects_replacement_without_conversation_writeback() {
    let fixture = Fixture::new();
    let before_prompts = fixture.prompts();
    let before_events = fixture.event_count();
    fixture.replace_workspace();

    let output = fixture.resume();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("workspace_unavailable"));
    assert_eq!(fixture.prompts(), before_prompts);
    assert_eq!(fixture.event_count(), before_events);
}

struct Fixture {
    _root: TempDir,
    state: TempDir,
    selected: std::path::PathBuf,
    original: std::path::PathBuf,
    store: Arc<SqliteHubStore>,
    conversation_id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("workspace parent");
        let state = TempDir::new().expect("state directory");
        let selected = root.path().join("project");
        let original = root.path().join("original-project");
        fs::create_dir(&selected).expect("selected project");
        fs::write(selected.join("note.txt"), "original\n").expect("workspace fixture");
        let store = Arc::new(
            SqliteHubStore::open(state.path().join("hub.sqlite3")).expect("open Hub fixture"),
        );
        let conversation_id = seed_completed_agent(&store, &selected);
        Self {
            _root: root,
            state,
            selected,
            original,
            store,
            conversation_id,
        }
    }

    fn replace_workspace(&self) {
        fs::rename(&self.selected, &self.original).expect("move original workspace");
        fs::create_dir(&self.selected).expect("create replacement workspace");
        fs::write(self.selected.join("note.txt"), "replacement\n").expect("replacement fixture");
    }

    fn resume(&self) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
            .args([
                "--state-dir",
                path_text(self.state.path()),
                "-C",
                path_text(&self.selected),
                "run",
                "resume",
                RUN_ID,
            ])
            .env_remove("OPENAI_API_KEY")
            .output()
            .expect("resume completed Agent Run")
    }

    fn prompts(&self) -> Vec<forge_runtime_domain::PromptRecord> {
        self.store
            .list_prompts(Some(&self.conversation_id), 10)
            .expect("list Conversation prompts")
    }

    fn event_count(&self) -> usize {
        self.store
            .inspect_run(RUN_ID)
            .expect("inspect Agent Run")
            .events
            .len()
    }
}

fn seed_completed_agent(store: &Arc<SqliteHubStore>, selected: &std::path::Path) -> String {
    let canonical = selected.canonicalize().expect("canonical project");
    let project = store.open_project(&canonical).expect("open Project");
    let scope = ConversationScope::Project(project.id.clone());
    let conversation = store
        .create_conversation(&scope, "Agent fixture", "agent-fixture-session")
        .expect("create Conversation");
    let prompt = store
        .append_prompt(
            &conversation.id,
            "user",
            "inspect the workspace",
            "agent-fixture-prompt",
        )
        .expect("append user Prompt");
    let identity = CapStdAgentWorkspace::open(&canonical)
        .expect("open Agent workspace")
        .workspace_identity()
        .clone();
    let begin = BeginRun {
        v: RUN_STORE_VERSION,
        run_id: RUN_ID.into(),
        conversation_id: conversation.id.clone(),
        prompt_id: prompt.id,
        project_id: project.id,
        execution: agent_execution(identity),
        idempotency_key: "completed-agent-key".into(),
        created_at_ms: 1,
    };
    store.begin_run(&begin).expect("seed Agent Run");
    append_terminal_events(store, &conversation.id);
    conversation.id
}

fn agent_execution(identity: forge_runtime_domain::WorkspaceIdentity) -> RunExecution {
    RunExecution {
        provider: RunProvider::OpenAiAgent {
            endpoint: "https://api.openai.com/v1".into(),
            model: "offline-test-model".into(),
            dev: false,
            toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
            workspace_identity: Some(identity),
        },
        system_prompt: "Read only".into(),
        allowed_read_paths: Vec::new(),
        limits: RunLimits::default(),
    }
}

fn append_terminal_events(store: &SqliteHubStore, conversation_id: &str) {
    let answer = "cached Agent answer";
    let events = [
        RuntimeEventKind::RunStarted {
            prompt: "inspect the workspace".into(),
        },
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: "inspect the workspace".into(),
            },
        },
        RuntimeEventKind::TurnStarted { turn: 1 },
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant {
                text: answer.into(),
                tool_calls: Vec::new(),
            },
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: answer.into(),
            },
        },
    ];
    for (index, kind) in events.into_iter().enumerate() {
        store
            .append_event(&RuntimeEvent {
                v: PROTOCOL_VERSION,
                session_id: conversation_id.into(),
                run_id: RUN_ID.into(),
                seq: u64::try_from(index + 1).expect("event sequence"),
                emitted_at_ms: u64::try_from(index + 1).expect("event timestamp"),
                kind,
            })
            .expect("append terminal Agent event");
    }
}

fn path_text(path: &std::path::Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}
