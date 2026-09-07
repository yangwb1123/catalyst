use crate::runtime_domain::{
    CURRENT_AGENT_TOOLSET_VERSION, Capability, LEGACY_AGENT_TOOLSET_VERSION, RunExecution,
    RunProvider,
};

use super::{AuthorizationView, BoundaryView, CapabilityView};

pub(super) fn authorization_for(execution: &RunExecution) -> AuthorizationView {
    let capabilities = execution.allowed_capabilities();
    let exposed_status = runtime_exposure_status(execution);
    let workspace_read = if capabilities.contains(&Capability::WorkspaceRead) {
        CapabilityView {
            status: exposed_status,
            scope: read_scope(execution),
        }
    } else {
        CapabilityView {
            status: "not_granted",
            scope: Vec::new(),
        }
    };
    AuthorizationView {
        source: "persisted Project Run execution configuration; not an authenticated Grant/Approval/PDP decision",
        workspace_read,
        workspace_write: optional_capability(
            capabilities.contains(&Capability::WorkspaceWrite),
            "selected_workspace",
            exposed_status,
        ),
        process: optional_capability(
            capabilities.contains(&Capability::Process),
            "initial_cwd_anchored_to_selected_workspace; subprocess retains ambient same-user filesystem and network access",
            exposed_status,
        ),
        network: network_boundary(execution),
    }
}

pub(super) fn workspace_boundary(execution: &RunExecution) -> BoundaryView {
    if matches!(
        &execution.provider,
        RunProvider::OpenAiAgent { dev: true, .. }
    ) {
        BoundaryView {
            status: "explicit_dev_mode",
            reason: "the selected workspace is readable and mutable and local processes are exposed; this trusted local mode is not an OS sandbox",
        }
    } else if execution.is_agent() {
        BoundaryView {
            status: "read_only_agent",
            reason: "the selected workspace is readable, while write and process capabilities remain disabled; the same-user runtime is not an OS sandbox",
        }
    } else {
        BoundaryView {
            status: "not_exposed",
            reason: "the Project Run read tool is restricted to its persisted exact-path allowlist",
        }
    }
}

fn read_scope(execution: &RunExecution) -> Vec<String> {
    if execution.is_agent() {
        vec!["selected_workspace".into()]
    } else {
        execution.allowed_read_paths.clone()
    }
}

fn optional_capability(enabled: bool, scope: &str, exposed_status: &'static str) -> CapabilityView {
    if enabled {
        CapabilityView {
            status: exposed_status,
            scope: vec![scope.into()],
        }
    } else {
        not_exposed()
    }
}

fn not_exposed() -> CapabilityView {
    CapabilityView {
        status: "not_exposed_by_project_run_v1",
        scope: Vec::new(),
    }
}

fn network_boundary(execution: &RunExecution) -> CapabilityView {
    match &execution.provider {
        RunProvider::OpenAiAgent { .. }
            if runtime_exposure_status(execution) == "declared_but_runtime_unavailable" =>
        {
            CapabilityView {
                status: "declared_but_runtime_unavailable",
                scope: vec![
                    "provider and tool activity are blocked by an unsupported or incomplete persisted Agent configuration"
                        .into(),
                ],
            }
        }
        RunProvider::OpenAiAgent { dev, .. } => CapabilityView {
            status: "ambient_egress_without_network_tool_or_containment",
            scope: vec![if *dev {
                "provider egress plus possible subprocess ambient network access".into()
            } else {
                "provider egress".into()
            }],
        },
        RunProvider::OpenAiResponses { .. } => CapabilityView {
            status: "provider_egress_without_network_tool",
            scope: vec!["provider egress".into()],
        },
        RunProvider::DeterministicRead { .. } => not_exposed(),
    }
}

fn runtime_exposure_status(execution: &RunExecution) -> &'static str {
    let RunProvider::OpenAiAgent {
        toolset_version,
        workspace_identity,
        ..
    } = &execution.provider
    else {
        return "declared_and_runtime_exposed";
    };
    if workspace_identity.is_some()
        && [LEGACY_AGENT_TOOLSET_VERSION, CURRENT_AGENT_TOOLSET_VERSION].contains(toolset_version)
    {
        "declared_and_runtime_exposed"
    } else {
        "declared_but_runtime_unavailable"
    }
}
