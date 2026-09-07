use std::{fs, path::PathBuf};

use forge_runtime_application::ToolCatalog;
use forge_runtime_infrastructure::CapStdAgentWorkspace;
use serde_json::json;
use tempfile::TempDir;

use super::{
    AgentAccess, StartMode, StartOptions, StartOutput, configured_read_paths, execution_for_opened,
};
use crate::{
    agent_workspace,
    runtime_domain::{
        CURRENT_AGENT_TOOLSET_VERSION, Cancellation, Capability, LEGACY_AGENT_TOOLSET_VERSION,
        RunProvider, ToolContext,
    },
};

fn live_options(allowed_read_paths: &[String]) -> StartOptions<'_> {
    StartOptions {
        conversation_id: "conversation",
        prompt_id: "prompt",
        read_path: "README.md",
        allowed_read_paths,
        mode: StartMode::Live,
        model: None,
        max_output_tokens: 4_096,
        output: StartOutput::JsonLines,
        max_turns: 4,
        max_tool_calls: 4,
    }
}

#[test]
fn live_without_read_consent_configures_no_path() {
    let options = live_options(&[]);
    assert!(configured_read_paths(&options).is_empty());
}

#[test]
fn explicit_live_read_consent_keeps_exact_paths() {
    let allowed = vec!["src/lib.rs".to_owned(), ".env".to_owned()];
    let options = live_options(&allowed);
    assert_eq!(configured_read_paths(&options), allowed);
}

#[test]
fn offline_read_is_scoped_to_its_deterministic_target() {
    let options = StartOptions {
        mode: StartMode::Deterministic,
        ..live_options(&[])
    };
    assert_eq!(configured_read_paths(&options), ["README.md"]);
}

#[test]
fn read_only_agent_registers_only_workspace_read() {
    let workspace = TempDir::new().expect("workspace");
    let options = agent_options(AgentAccess::ReadOnly);
    let bundle = CapStdAgentWorkspace::open(workspace.path()).expect("Agent workspace");
    let execution = execution_for_opened(&options, Some(&bundle)).expect("Agent execution");
    let (tools, capabilities) =
        agent_workspace::runtime_tools(&execution, Some(&bundle)).expect("runtime tools");

    assert!(matches!(
        execution.provider,
        RunProvider::OpenAiAgent {
            dev: false,
            toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
            workspace_identity: Some(_),
            ..
        }
    ));
    assert!(
        execution
            .system_prompt
            .contains("do not invent or continue a Sprint")
    );
    assert_eq!(capabilities, [Capability::WorkspaceRead]);
    assert_eq!(
        tool_names(&tools),
        ["list_files", "read_file", "search_text"]
    );
}

#[test]
fn dev_agent_registers_the_explicit_mutating_tools() {
    let workspace = TempDir::new().expect("workspace");
    let options = agent_options(AgentAccess::Dev);
    let bundle = CapStdAgentWorkspace::open(workspace.path()).expect("Agent workspace");
    let execution = execution_for_opened(&options, Some(&bundle)).expect("Agent execution");
    let (tools, capabilities) =
        agent_workspace::runtime_tools(&execution, Some(&bundle)).expect("runtime tools");

    assert!(
        execution
            .system_prompt
            .contains("do not invent or continue a Sprint")
    );
    assert!(matches!(
        execution.provider,
        RunProvider::OpenAiAgent {
            toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
            workspace_identity: Some(_),
            ..
        }
    ));
    assert_eq!(
        capabilities,
        [
            Capability::WorkspaceRead,
            Capability::WorkspaceWrite,
            Capability::Process,
        ]
    );
    assert_eq!(
        tool_names(&tools),
        [
            "edit_file",
            "exec_command",
            "list_files",
            "read_file",
            "search_text",
        ]
    );
}

#[test]
fn legacy_agent_toolset_restores_only_the_original_read_tool() {
    let workspace = TempDir::new().expect("workspace");
    let options = agent_options(AgentAccess::ReadOnly);
    let bundle = CapStdAgentWorkspace::open(workspace.path()).expect("Agent workspace");
    let mut execution = execution_for_opened(&options, Some(&bundle)).expect("Agent execution");
    let RunProvider::OpenAiAgent {
        toolset_version, ..
    } = &mut execution.provider
    else {
        panic!("Agent provider expected");
    };
    *toolset_version = LEGACY_AGENT_TOOLSET_VERSION;

    let (tools, capabilities) =
        agent_workspace::runtime_tools(&execution, Some(&bundle)).expect("legacy tools");

    assert_eq!(capabilities, [Capability::WorkspaceRead]);
    assert_eq!(tool_names(&tools), ["read_file"]);
}

#[test]
fn legacy_dev_toolset_restores_the_original_effect_surface() {
    let workspace = TempDir::new().expect("workspace");
    let options = agent_options(AgentAccess::Dev);
    let bundle = CapStdAgentWorkspace::open(workspace.path()).expect("Agent workspace");
    let mut execution = execution_for_opened(&options, Some(&bundle)).expect("Agent execution");
    let RunProvider::OpenAiAgent {
        toolset_version, ..
    } = &mut execution.provider
    else {
        panic!("Agent provider expected")
    };
    *toolset_version = LEGACY_AGENT_TOOLSET_VERSION;

    let (tools, capabilities) =
        agent_workspace::runtime_tools(&execution, Some(&bundle)).expect("legacy dev tools");

    assert_eq!(
        capabilities,
        [
            Capability::WorkspaceRead,
            Capability::WorkspaceWrite,
            Capability::Process,
        ]
    );
    assert_eq!(
        tool_names(&tools),
        ["edit_file", "exec_command", "read_file"]
    );
}

#[test]
fn unknown_agent_toolset_version_fails_closed() {
    let workspace = TempDir::new().expect("workspace");
    let options = agent_options(AgentAccess::ReadOnly);
    let bundle = CapStdAgentWorkspace::open(workspace.path()).expect("Agent workspace");
    let mut execution = execution_for_opened(&options, Some(&bundle)).expect("Agent execution");
    let RunProvider::OpenAiAgent {
        toolset_version, ..
    } = &mut execution.provider
    else {
        panic!("Agent provider expected");
    };
    *toolset_version = u16::MAX;

    let error = agent_workspace::validate_execution(&execution, Some(&bundle))
        .expect_err("unknown toolset must fail");

    assert!(
        error
            .to_string()
            .contains("unsupported persisted Agent toolset")
    );
}

#[cfg(not(unix))]
#[test]
fn dev_agent_is_rejected_before_an_execution_can_be_persisted() {
    let error = execution_for_opened(&agent_options(AgentAccess::Dev), None)
        .expect_err("non-Unix dev Agent must fail closed");

    assert!(error.to_string().contains("requires Unix"));
}

#[cfg(unix)]
#[tokio::test]
async fn agent_bundle_never_follows_a_replaced_project_path() {
    let fixture = replaced_workspace();
    let options = agent_options(AgentAccess::Dev);
    let execution = execution_for_opened(&options, Some(&fixture.bundle)).expect("Agent execution");
    let (tools, _) =
        agent_workspace::runtime_tools(&execution, Some(&fixture.bundle)).expect("anchored tools");
    let factory = agent_workspace::runtime_factory(&execution, Some(fixture.bundle.clone()))
        .expect("anchored runtime factory");
    let read_capability = factory
        .open(&fixture.selected)
        .expect("anchored read capability");
    let context = ToolContext {
        workspace: read_capability,
        cancellation: Cancellation::default(),
        max_output_bytes: 16 * 1024,
    };

    let read = execute(
        &tools,
        "read_file",
        json!({"path": "note.txt"}),
        context.clone(),
    )
    .await;
    assert_eq!(read.content, "original\n");
    execute(
        &tools,
        "edit_file",
        json!({"path":"note.txt","old_text":"original\n","new_text":"edited\n"}),
        context.clone(),
    )
    .await;
    let process = execute(
        &tools,
        "exec_command",
        json!({"program":"/bin/sh","argv":["-c","cat note.txt"]}),
        context,
    )
    .await;
    assert!(process.content.contains("edited\n"));
    assert_eq!(
        fs::read_to_string(fixture.original.join("note.txt")).unwrap(),
        "edited\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.selected.join("note.txt")).unwrap(),
        "replacement\n"
    );
}

#[cfg(unix)]
struct ReplacedWorkspace {
    _root: TempDir,
    selected: PathBuf,
    original: PathBuf,
    bundle: CapStdAgentWorkspace,
}

#[cfg(unix)]
fn replaced_workspace() -> ReplacedWorkspace {
    let root = TempDir::new().expect("temporary root");
    let selected = root.path().join("project");
    let original = root.path().join("original-project");
    fs::create_dir(&selected).expect("selected project");
    fs::write(selected.join("note.txt"), "original\n").expect("original fixture");
    let bundle = CapStdAgentWorkspace::open(&selected).expect("anchored Agent workspace");
    fs::rename(&selected, &original).expect("move original project");
    fs::create_dir(&selected).expect("replacement project");
    fs::write(selected.join("note.txt"), "replacement\n").expect("replacement fixture");
    ReplacedWorkspace {
        _root: root,
        selected,
        original,
        bundle,
    }
}

#[cfg(unix)]
async fn execute(
    tools: &ToolCatalog,
    name: &str,
    arguments: serde_json::Value,
    context: ToolContext,
) -> crate::runtime_domain::ToolOutput {
    tools
        .get(name)
        .expect("registered Agent tool")
        .execute(arguments, context)
        .await
        .expect("anchored tool succeeds")
}

fn agent_options(access: AgentAccess) -> StartOptions<'static> {
    StartOptions {
        mode: StartMode::Agent(access),
        max_turns: 24,
        max_tool_calls: 64,
        ..live_options(&[])
    }
}

fn tool_names(tools: &ToolCatalog) -> Vec<String> {
    tools.specs().into_iter().map(|spec| spec.name).collect()
}
