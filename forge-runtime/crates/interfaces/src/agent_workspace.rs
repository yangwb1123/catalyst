use std::{error::Error, path::Path, sync::Arc};

use forge_runtime_infrastructure::{CapStdAgentWorkspace, CapStdWorkspaceFactory, ReadFileTool};

use crate::{
    runtime_application::{
        RuntimeError, ToolCatalog, validate_agent_tool_catalog, validate_resume_workspace_identity,
    },
    runtime_domain::{
        CURRENT_AGENT_TOOLSET_VERSION, Capability, LEGACY_AGENT_TOOLSET_VERSION, RunExecution,
        RunProvider, WorkspaceIdentity, WorkspaceReadFactory,
    },
};

pub(crate) fn open(
    required: bool,
    workspace: &Path,
) -> Result<Option<CapStdAgentWorkspace>, RuntimeError> {
    if !required {
        return Ok(None);
    }
    CapStdAgentWorkspace::open(workspace)
        .map(Some)
        .map_err(|error| RuntimeError::Workspace(error.to_string()))
}

pub(crate) fn identity(
    workspace: Option<&CapStdAgentWorkspace>,
) -> Result<&WorkspaceIdentity, RuntimeError> {
    workspace
        .map(CapStdAgentWorkspace::workspace_identity)
        .ok_or_else(missing)
}

pub(crate) fn validate_execution(
    execution: &RunExecution,
    workspace: Option<&CapStdAgentWorkspace>,
) -> Result<(), RuntimeError> {
    #[cfg(not(unix))]
    ensure_dev_supported(matches!(
        &execution.provider,
        RunProvider::OpenAiAgent { dev: true, .. }
    ))?;
    validate_resume_workspace_identity(
        execution,
        workspace.map(CapStdAgentWorkspace::workspace_identity),
    )
}

#[cfg(not(unix))]
pub(crate) fn ensure_dev_supported(dev: bool) -> Result<(), RuntimeError> {
    if dev {
        return Err(RuntimeError::Workspace(
            "dev Agent requires Unix descriptor-anchored process execution".into(),
        ));
    }
    Ok(())
}

pub(crate) fn runtime_tools(
    execution: &RunExecution,
    agent_workspace: Option<&CapStdAgentWorkspace>,
) -> Result<(ToolCatalog, Vec<Capability>), Box<dyn Error>> {
    let mut tools = ToolCatalog::default();
    if let RunProvider::OpenAiAgent {
        dev,
        toolset_version,
        ..
    } = &execution.provider
    {
        validate_toolset_version(execution)?;
        let workspace = agent_workspace.ok_or_else(missing)?;
        tools.register(Arc::new(ReadFileTool))?;
        if *toolset_version == CURRENT_AGENT_TOOLSET_VERSION {
            tools.register(Arc::new(workspace.list_files_tool()))?;
            tools.register(Arc::new(workspace.search_text_tool()))?;
        }
        if *dev {
            tools.register(Arc::new(workspace.edit_file_tool()))?;
            tools.register(Arc::new(workspace.exec_command_tool()))?;
        }
        validate_agent_tool_catalog(execution, &tools)?;
        return Ok((tools, execution.allowed_capabilities()));
    }
    if !execution.allowed_read_paths.is_empty() {
        tools.register(Arc::new(ReadFileTool::restricted(
            execution.allowed_read_paths.iter().cloned(),
        )))?;
    }
    Ok((tools, execution.allowed_capabilities()))
}

pub(crate) fn runtime_factory(
    execution: &RunExecution,
    agent_workspace: Option<CapStdAgentWorkspace>,
) -> Result<Arc<dyn WorkspaceReadFactory>, RuntimeError> {
    if execution.is_agent() {
        return agent_workspace
            .map(|workspace| Arc::new(workspace) as Arc<dyn WorkspaceReadFactory>)
            .ok_or_else(missing);
    }
    Ok(Arc::new(CapStdWorkspaceFactory))
}

fn missing() -> RuntimeError {
    RuntimeError::Workspace("Agent execution requires one anchored workspace bundle".into())
}

fn validate_toolset_version(execution: &RunExecution) -> Result<(), RuntimeError> {
    let RunProvider::OpenAiAgent {
        toolset_version, ..
    } = &execution.provider
    else {
        return Ok(());
    };
    if [LEGACY_AGENT_TOOLSET_VERSION, CURRENT_AGENT_TOOLSET_VERSION].contains(toolset_version) {
        return Ok(());
    }
    Err(RuntimeError::Protocol(format!(
        "unsupported persisted Agent toolset version {toolset_version}"
    )))
}
