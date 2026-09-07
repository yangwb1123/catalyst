use crate::{
    RuntimeError,
    runtime_domain::{
        CURRENT_AGENT_TOOLSET_VERSION, LEGACY_AGENT_TOOLSET_VERSION, RunExecution, RunProvider,
        RunRequest, WorkspaceIdentity, WorkspaceReadCapability, WorkspaceReadFactory,
    },
};

pub(crate) fn open_agent_resume_workspace(
    factory: &dyn WorkspaceReadFactory,
    request: &RunRequest,
    execution: &RunExecution,
) -> Result<Option<WorkspaceReadCapability>, RuntimeError> {
    if !execution.is_agent() {
        return Ok(None);
    }
    let workspace = open_workspace(factory, request)?;
    validate_resume_workspace_identity(execution, workspace.workspace_identity())?;
    Ok(Some(workspace))
}

pub(crate) fn open_workspace(
    factory: &dyn WorkspaceReadFactory,
    request: &RunRequest,
) -> Result<WorkspaceReadCapability, RuntimeError> {
    factory
        .open(&request.workspace)
        .map_err(|error| RuntimeError::Workspace(error.to_string()))
}

pub(crate) fn resolve_resume_workspace(
    factory: &dyn WorkspaceReadFactory,
    request: &RunRequest,
    agent_workspace: Option<WorkspaceReadCapability>,
) -> Result<WorkspaceReadCapability, RuntimeError> {
    match agent_workspace {
        Some(workspace) => Ok(workspace),
        None => open_workspace(factory, request),
    }
}

/// Validates an Agent continuation's persisted toolset and workspace identity.
///
/// # Errors
///
/// Returns an error for an unsupported toolset version, a missing identity, an
/// unavailable host identity, or a changed workspace.
pub fn validate_resume_workspace_identity(
    execution: &RunExecution,
    current: Option<&WorkspaceIdentity>,
) -> Result<(), RuntimeError> {
    let RunProvider::OpenAiAgent {
        toolset_version,
        workspace_identity,
        ..
    } = &execution.provider
    else {
        return Ok(());
    };
    if ![LEGACY_AGENT_TOOLSET_VERSION, CURRENT_AGENT_TOOLSET_VERSION].contains(toolset_version) {
        return Err(RuntimeError::Protocol(format!(
            "unsupported persisted Agent toolset version {toolset_version}"
        )));
    }
    let expected = workspace_identity.as_ref().ok_or_else(|| {
        RuntimeError::Protocol("Agent resume requires a persisted workspace identity".into())
    })?;
    let current = current.ok_or_else(|| {
        RuntimeError::Workspace("stable workspace identity is unavailable on this host".into())
    })?;
    if current != expected {
        return Err(RuntimeError::Workspace(
            "Agent resume refused because the workspace identity changed".into(),
        ));
    }
    Ok(())
}
