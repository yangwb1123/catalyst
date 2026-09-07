use std::{fs, path::PathBuf, sync::Arc};

use crate::runtime_application::{AgentRuntime, ToolCatalog};
use crate::runtime_domain::{
    AgentTool, BeginRun, CURRENT_AGENT_TOOLSET_VERSION, Cancellation, Capability,
    ConversationScope, HubStore, LimitKind, MAX_RUN_EVENTS, MAX_RUN_JOURNAL_BYTES, ModelEvent,
    ModelFinishReason, RUN_STORE_VERSION, RunExecution, RunLimits, RunOutcome, RunProvider,
    RunRecoveryState, RunRequest, RunStore, ToolCall, ToolContext, ToolFuture, ToolOutput,
    ToolSpec,
};
use forge_runtime_infrastructure::{
    CapStdWorkspaceFactory, DurableFirstEventSink, MemoryEventSink, ScriptedProvider,
    SqliteHubStore,
};
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

use super::{
    JSON_STRING_EXPANSION, MAX_AGENT_MODEL_EVENTS, TOOL_OUTPUT_JOURNAL_BUDGET,
    TOOL_OUTPUT_JOURNAL_COPIES, bounded,
};

const MAX_AGENT_TURNS: u32 = 64;
const MAX_AGENT_TOOL_CALLS: u32 = 256;
const NON_TOOL_JOURNAL_RESERVE: usize = 32 * 1024 * 1024;

#[test]
fn public_agent_bounds_reserve_event_and_byte_capacity_for_terminal_state() {
    let limits = public_limits();

    let event_bound = 3 * usize::try_from(MAX_AGENT_MODEL_EVENTS).unwrap()
        + 2 * usize::try_from(MAX_AGENT_TURNS).unwrap()
        + 4;
    assert!(event_bound < MAX_RUN_EVENTS);

    let tool_bound = usize::try_from(MAX_AGENT_TOOL_CALLS).unwrap()
        * limits.max_tool_output_bytes
        * JSON_STRING_EXPANSION
        * TOOL_OUTPUT_JOURNAL_COPIES;
    assert_eq!(
        limits.max_tool_output_bytes,
        TOOL_OUTPUT_JOURNAL_BUDGET
            / (usize::try_from(MAX_AGENT_TOOL_CALLS).unwrap()
                * JSON_STRING_EXPANSION
                * TOOL_OUTPUT_JOURNAL_COPIES)
    );
    assert!(tool_bound <= TOOL_OUTPUT_JOURNAL_BUDGET);
    assert!(tool_bound + NON_TOOL_JOURNAL_RESERVE <= MAX_RUN_JOURNAL_BYTES);
}

#[tokio::test]
async fn one_turn_of_maximum_model_events_still_persists_tool_limit_terminal() {
    let limits = public_limits();
    let call_count = usize::try_from(limits.max_model_events).expect("model event count");
    let fixture = Fixture::new(limits.clone());
    let runtime = runtime(tool_call_turn(call_count), ToolCatalog::default());

    let outcome = fixture.run(&runtime).await;
    assert_eq!(
        outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::ToolCalls
        }
    );

    let inspection = fixture.store.inspect_run("capacity-run").expect("inspect");
    let expected_events = 5 + 2 * call_count;
    assert_eq!(inspection.events.len(), expected_events);
    assert_eq!(expected_events, 4_101);
    assert_eq!(
        inspection.events.last().expect("terminal").seq,
        u64::try_from(expected_events).expect("terminal sequence")
    );
    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::LimitExceeded {
                kind: LimitKind::ToolCalls
            }
        }
    ));
}

#[tokio::test]
async fn worst_case_tool_output_expansion_leaves_sqlite_terminal_capacity() {
    let limits = public_limits();
    let output = "\0".repeat(limits.max_tool_output_bytes);
    let mut catalog = ToolCatalog::default();
    catalog
        .register(Arc::new(ControlOutputTool { output }))
        .expect("register capacity tool");
    let fixture = Fixture::new(limits.clone());
    let runtime = runtime(output_turns(MAX_AGENT_TOOL_CALLS), catalog);

    let outcome = fixture.run(&runtime).await;
    assert_eq!(
        outcome,
        RunOutcome::Completed {
            answer: "done".into()
        }
    );
    let inspection = fixture.store.inspect_run("capacity-run").expect("inspect");
    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal { .. }
    ));
    let full_outputs = inspection
        .events
        .iter()
        .filter(|event| match &event.kind {
            forge_runtime_domain::RuntimeEventKind::ToolFinished {
                output, truncated, ..
            } => output.len() == limits.max_tool_output_bytes && !truncated,
            _ => false,
        })
        .count();
    assert_eq!(
        full_outputs,
        usize::try_from(MAX_AGENT_TOOL_CALLS).expect("tool count")
    );

    let (cursor_bytes, stored_bytes, event_count) = fixture.stored_journal_metrics();
    let expanded_output_bytes = usize::try_from(MAX_AGENT_TOOL_CALLS).expect("tool count")
        * limits.max_tool_output_bytes
        * JSON_STRING_EXPANSION
        * TOOL_OUTPUT_JOURNAL_COPIES;
    assert_eq!(cursor_bytes, stored_bytes);
    assert_eq!(event_count, inspection.events.len());
    assert!(stored_bytes >= expanded_output_bytes);
    assert!(stored_bytes <= MAX_RUN_JOURNAL_BYTES);
}

fn public_limits() -> RunLimits {
    bounded(MAX_AGENT_TURNS, MAX_AGENT_TOOL_CALLS, 32_768)
}

fn runtime(
    turns: Vec<Vec<Result<ModelEvent, forge_runtime_domain::ProviderError>>>,
    tools: ToolCatalog,
) -> AgentRuntime {
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(turns)),
        tools,
        Arc::new(CapStdWorkspaceFactory),
    )
}

fn tool_call_turn(
    count: usize,
) -> Vec<Vec<Result<ModelEvent, forge_runtime_domain::ProviderError>>> {
    let mut turn = calls(count);
    turn.push(Ok(ModelEvent::Finished {
        reason: ModelFinishReason::ToolUse,
    }));
    vec![turn]
}

fn output_turns(count: u32) -> Vec<Vec<Result<ModelEvent, forge_runtime_domain::ProviderError>>> {
    let mut first = calls(usize::try_from(count).expect("tool call count"));
    first.push(Ok(ModelEvent::Finished {
        reason: ModelFinishReason::ToolUse,
    }));
    vec![
        first,
        vec![
            Ok(ModelEvent::TextDelta {
                delta: "done".into(),
            }),
            Ok(ModelEvent::Finished {
                reason: ModelFinishReason::Completed,
            }),
        ],
    ]
}

fn calls(count: usize) -> Vec<Result<ModelEvent, forge_runtime_domain::ProviderError>> {
    (0..count)
        .map(|index| {
            Ok(ModelEvent::ToolCall {
                call: ToolCall {
                    id: format!("call-{index}"),
                    name: "capacity_output".into(),
                    arguments: json!({}),
                },
            })
        })
        .collect()
}

struct ControlOutputTool {
    output: String,
}

impl AgentTool for ControlOutputTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "capacity_output".into(),
            description: "Return a capacity-test output.".into(),
            input_schema: json!({"type": "object", "additionalProperties": false}),
            capability: Capability::WorkspaceRead,
        }
    }

    fn execute(&self, _arguments: serde_json::Value, _context: ToolContext) -> ToolFuture<'_> {
        let content = self.output.clone();
        Box::pin(async move {
            Ok(ToolOutput {
                content,
                truncated: false,
            })
        })
    }
}

struct Fixture {
    _root: TempDir,
    database: PathBuf,
    store: SqliteHubStore,
    request: RunRequest,
}

impl Fixture {
    fn new(limits: RunLimits) -> Self {
        let root = TempDir::new().expect("capacity root");
        let workspace = root.path().join("project");
        fs::create_dir(&workspace).expect("capacity workspace");
        let workspace = workspace.canonicalize().expect("canonical workspace");
        let database = root.path().join("private").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        let project = store.open_project(&workspace).expect("Project");
        let conversation = store
            .create_conversation(
                &ConversationScope::Project(project.id.clone()),
                "Capacity",
                "capacity-conversation-key",
            )
            .expect("Conversation");
        let prompt = store
            .append_prompt(&conversation.id, "user", "capacity", "capacity-prompt-key")
            .expect("Prompt");
        let execution = execution(limits.clone());
        store
            .begin_run(&BeginRun {
                v: RUN_STORE_VERSION,
                run_id: "capacity-run".into(),
                conversation_id: conversation.id.clone(),
                prompt_id: prompt.id,
                project_id: project.id,
                execution: execution.clone(),
                idempotency_key: "capacity-run-key".into(),
                created_at_ms: 1,
            })
            .expect("begin Run");
        Self {
            _root: root,
            database,
            store,
            request: RunRequest {
                session_id: conversation.id,
                run_id: "capacity-run".into(),
                prompt: "capacity".into(),
                system_prompt: execution.system_prompt,
                workspace,
                allowed_capabilities: vec![Capability::WorkspaceRead],
                limits,
            },
        }
    }

    async fn run(&self, runtime: &AgentRuntime) -> RunOutcome {
        let mut downstream = MemoryEventSink::default();
        let mut sink = DurableFirstEventSink::new(&self.store, &mut downstream);
        runtime
            .run(self.request.clone(), Cancellation::default(), &mut sink)
            .await
            .expect("capacity run")
            .outcome
    }

    fn stored_journal_metrics(&self) -> (usize, usize, usize) {
        let connection = Connection::open(&self.database).expect("open metrics connection");
        let (cursor, stored, count): (i64, i64, i64) = connection
            .query_row(
                "SELECT r.journal_bytes, \
                 COALESCE(SUM(length(CAST(e.event_json AS BLOB))), 0), COUNT(e.seq) \
                 FROM runs r LEFT JOIN run_events e ON e.run_id = r.id \
                 WHERE r.id = ?1 GROUP BY r.id",
                ["capacity-run"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("journal metrics");
        (
            usize::try_from(cursor).expect("cursor bytes"),
            usize::try_from(stored).expect("stored bytes"),
            usize::try_from(count).expect("event count"),
        )
    }
}

fn execution(limits: RunLimits) -> RunExecution {
    RunExecution {
        provider: RunProvider::OpenAiAgent {
            endpoint: "https://example.invalid/v1/responses".into(),
            model: "capacity-model".into(),
            dev: true,
            toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
            workspace_identity: None,
        },
        system_prompt: "Capacity test.".into(),
        allowed_read_paths: Vec::new(),
        limits,
    }
}
