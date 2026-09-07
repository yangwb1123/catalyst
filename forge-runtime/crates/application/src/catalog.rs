use std::{collections::BTreeMap, sync::Arc};

use forge_runtime_domain::{
    AgentTool, CURRENT_AGENT_TOOLSET_VERSION, Capability, LEGACY_AGENT_TOOLSET_VERSION,
    RunExecution, RunProvider, ToolSpec,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::RuntimeError;

#[derive(Clone, Default)]
pub struct ToolCatalog {
    tools: BTreeMap<String, Arc<dyn AgentTool>>,
}

impl ToolCatalog {
    /// Registers a tool under the name returned by its specification.
    ///
    /// # Errors
    ///
    /// Returns an error when another tool already owns the same name.
    pub fn register(&mut self, tool: Arc<dyn AgentTool>) -> Result<(), RuntimeError> {
        let name = tool.spec().name;
        if self.tools.contains_key(&name) {
            return Err(RuntimeError::ToolCatalog(format!(
                "duplicate tool name '{name}'"
            )));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn AgentTool>> {
        self.tools.get(name).cloned()
    }

    #[must_use]
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|tool| tool.spec()).collect()
    }
}

#[derive(Clone, Copy)]
struct ExpectedTool {
    name: &'static str,
    capability: Capability,
}

struct ExpectedSurface {
    tools: &'static [ExpectedTool],
    sha256: &'static str,
}

const TOOL_SURFACE_DIGEST_DOMAIN: &[u8] = b"forge-runtime.agent-tool-surface/v1\0";
const LEGACY_READ_ONLY_SHA256: &str =
    "9e85f70a0deac0ca0cdafdafd84a5e732dacd940fda1662d3c626910eb4263bf";
const LEGACY_DEV_SHA256: &str = "af9aaadb86b1cb0efe579570a0188a2e73a62c825489c5fea7dcc3c2050a26e1";
const CURRENT_READ_ONLY_SHA256: &str =
    "9477986d8ca549aa0811d0093ea05df63a291c449f6c8be9d26dd12de6e009d0";
const CURRENT_DEV_SHA256: &str = "165f689751b29579fa5c06f3119d0b192a2a9da077163e5b03dcd87838f6d939";

const LEGACY_READ_ONLY: &[ExpectedTool] = &[ExpectedTool {
    name: "read_file",
    capability: Capability::WorkspaceRead,
}];
const LEGACY_DEV: &[ExpectedTool] = &[
    ExpectedTool {
        name: "edit_file",
        capability: Capability::WorkspaceWrite,
    },
    ExpectedTool {
        name: "exec_command",
        capability: Capability::Process,
    },
    ExpectedTool {
        name: "read_file",
        capability: Capability::WorkspaceRead,
    },
];
const CURRENT_READ_ONLY: &[ExpectedTool] = &[
    ExpectedTool {
        name: "list_files",
        capability: Capability::WorkspaceRead,
    },
    ExpectedTool {
        name: "read_file",
        capability: Capability::WorkspaceRead,
    },
    ExpectedTool {
        name: "search_text",
        capability: Capability::WorkspaceRead,
    },
];
const CURRENT_DEV: &[ExpectedTool] = &[
    ExpectedTool {
        name: "edit_file",
        capability: Capability::WorkspaceWrite,
    },
    ExpectedTool {
        name: "exec_command",
        capability: Capability::Process,
    },
    ExpectedTool {
        name: "list_files",
        capability: Capability::WorkspaceRead,
    },
    ExpectedTool {
        name: "read_file",
        capability: Capability::WorkspaceRead,
    },
    ExpectedTool {
        name: "search_text",
        capability: Capability::WorkspaceRead,
    },
];

/// Validates that a persisted Agent version is paired with its exact tool surface.
///
/// # Errors
///
/// Returns a protocol error for an unknown version or a catalog error when a
/// known version has missing, additional, renamed, or recapabilitized tools.
pub fn validate_agent_tool_catalog(
    execution: &RunExecution,
    catalog: &ToolCatalog,
) -> Result<(), RuntimeError> {
    let RunProvider::OpenAiAgent {
        dev,
        toolset_version,
        ..
    } = &execution.provider
    else {
        return Ok(());
    };
    let expected = expected_surface(*toolset_version, *dev)?;
    let actual = catalog.specs();
    let shape_matches = actual.len() == expected.tools.len()
        && actual.iter().zip(expected.tools).all(|(actual, expected)| {
            actual.name == expected.name && actual.capability == expected.capability
        });
    let actual_sha256 = tool_surface_sha256(&actual)?;
    if shape_matches && actual_sha256 == expected.sha256 {
        return Ok(());
    }
    Err(RuntimeError::ToolCatalog(format!(
        "persisted Agent toolset version {toolset_version} does not match the runtime catalog (surface_sha256={actual_sha256})"
    )))
}

fn expected_surface(version: u16, dev: bool) -> Result<ExpectedSurface, RuntimeError> {
    let (tools, sha256) = match (version, dev) {
        (LEGACY_AGENT_TOOLSET_VERSION, false) => (LEGACY_READ_ONLY, LEGACY_READ_ONLY_SHA256),
        (LEGACY_AGENT_TOOLSET_VERSION, true) => (LEGACY_DEV, LEGACY_DEV_SHA256),
        (CURRENT_AGENT_TOOLSET_VERSION, false) => (CURRENT_READ_ONLY, CURRENT_READ_ONLY_SHA256),
        (CURRENT_AGENT_TOOLSET_VERSION, true) => (CURRENT_DEV, CURRENT_DEV_SHA256),
        _ => Err(RuntimeError::Protocol(format!(
            "unsupported persisted Agent toolset version {version}"
        )))?,
    };
    Ok(ExpectedSurface { tools, sha256 })
}

fn tool_surface_sha256(specs: &[ToolSpec]) -> Result<String, RuntimeError> {
    let value = serde_json::to_value(specs)
        .map_err(|error| RuntimeError::ToolCatalog(error.to_string()))?;
    let bytes = serde_json::to_vec(&sort_json(value))
        .map_err(|error| RuntimeError::ToolCatalog(error.to_string()))?;
    let mut digest = Sha256::new();
    digest.update(TOOL_SURFACE_DIGEST_DOMAIN);
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => Value::Object(
            items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        other => other,
    }
}
