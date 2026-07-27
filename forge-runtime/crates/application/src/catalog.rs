use std::{collections::BTreeMap, sync::Arc};

use forge_runtime_domain::{AgentTool, ToolSpec};

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
