use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    sync::Arc,
};

use forge_runtime_application::{GroupAgentGraphService, PrepareGroupAgentGraphInput};
use forge_runtime_domain::{
    GroupAgentGraphEdge, GroupAgentGraphManager, GroupAgentGraphNode,
    MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES,
};
use forge_runtime_infrastructure::SqliteHubStore;
use serde::Deserialize;

use crate::{
    args::{Args, GroupGraphCommand},
    state_path::{hub_database_path, idempotency_key, unique_id, unix_time_millis},
};

use super::output::GroupAgentGraphCliOutput;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphSpecInput {
    v: u16,
    manager: ManagerInput,
    nodes: Vec<NodeInput>,
    edges: Vec<EdgeInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagerInput {
    agent_profile: String,
    instruction: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeInput {
    node_id: String,
    project_id: String,
    member_role: String,
    agent_profile: String,
    task: String,
    acceptance: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeInput {
    from_node_id: String,
    to_node_id: String,
}

pub fn execute(
    args: &Args,
    command: &GroupGraphCommand,
) -> Result<GroupAgentGraphCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphCommand::Prepare {
            group_run_id,
            spec_source,
        } => {
            let spec = read_spec(spec_source)?;
            let service = service(args)?;
            prepare(args, &service, group_run_id, spec, spec_source != "-")
        }
        GroupGraphCommand::Show {
            graph_id,
            include_spec,
        } => {
            let service = service(args)?;
            Ok(GroupAgentGraphCliOutput::graph(
                service.inspect(graph_id)?,
                *include_spec,
            ))
        }
        GroupGraphCommand::List {
            group_run_id,
            limit,
        } => {
            let service = service(args)?;
            Ok(GroupAgentGraphCliOutput::list(
                service.list(group_run_id.as_deref(), *limit)?,
            ))
        }
        GroupGraphCommand::Run(_) => {
            Err(invalid_input("Group Agent Graph Run uses its dedicated command path").into())
        }
    }
}

fn service(args: &Args) -> Result<GroupAgentGraphService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentGraphService::new(store.clone(), store))
}

fn prepare(
    args: &Args,
    service: &GroupAgentGraphService,
    group_run_id: &str,
    spec: GraphSpecInput,
    explicit_spec_file_read: bool,
) -> Result<GroupAgentGraphCliOutput, Box<dyn Error>> {
    let result = service.prepare(&PrepareGroupAgentGraphInput {
        graph_id: unique_id("group-agent-graph"),
        group_run_id: group_run_id.into(),
        manager: spec.manager.into(),
        nodes: spec.nodes.into_iter().map(Into::into).collect(),
        edges: spec.edges.into_iter().map(Into::into).collect(),
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-graph")),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(GroupAgentGraphCliOutput::prepared(
        result.disposition,
        result.inspection,
        explicit_spec_file_read,
    ))
}

fn read_spec(source: &str) -> Result<GraphSpecInput, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_bounded(io::stdin().lock())?
    } else {
        read_bounded(File::open(source)?)?
    };
    let text = String::from_utf8(bytes)
        .map_err(|_| invalid_input("Group Agent Graph spec must be UTF-8"))?;
    let spec: GraphSpecInput = serde_json::from_str(&text)
        .map_err(|_| invalid_input("invalid Group Agent Graph spec JSON"))?;
    if spec.v != forge_runtime_domain::GROUP_AGENT_GRAPH_VERSION {
        return Err(invalid_input("unsupported Group Agent Graph spec version").into());
    }
    Ok(spec)
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let limit = MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES
        .checked_add(1)
        .expect("graph manifest bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("graph manifest bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES {
        return Err(invalid_input(
            "Group Agent Graph spec exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

impl From<ManagerInput> for GroupAgentGraphManager {
    fn from(value: ManagerInput) -> Self {
        Self {
            agent_profile: value.agent_profile,
            instruction: value.instruction,
        }
    }
}

impl From<NodeInput> for GroupAgentGraphNode {
    fn from(value: NodeInput) -> Self {
        Self {
            node_id: value.node_id,
            project_id: value.project_id,
            member_role: value.member_role,
            agent_profile: value.agent_profile,
            task: value.task,
            acceptance: value.acceptance,
        }
    }
}

impl From<EdgeInput> for GroupAgentGraphEdge {
    fn from(value: EdgeInput) -> Self {
        Self {
            from_node_id: value.from_node_id,
            to_node_id: value.to_node_id,
        }
    }
}
