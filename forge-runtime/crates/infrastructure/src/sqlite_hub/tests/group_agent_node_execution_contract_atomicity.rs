use crate::{
    runtime_domain::{
        AdmitGroupAgentNodeExecutionContract, GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION,
        GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION, GROUP_AGENT_GRAPH_RUN_VERSION,
        GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION, GROUP_AGENT_NODE_EXECUTION_PROTOCOL_VERSION,
        GroupAgentGraphControlSnapshot, GroupAgentGraphInspection, GroupAgentGraphNode,
        GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection,
        GroupAgentGraphRunStore, GroupAgentGraphStore, GroupAgentNodeArtifactKind,
        GroupAgentNodeDataflowPolicy, GroupAgentNodeEffectApproval,
        GroupAgentNodeExecutionApproval, GroupAgentNodeExecutionBudgets,
        GroupAgentNodeExecutionContract, GroupAgentNodeExecutionFailurePolicy,
        GroupAgentNodeExecutionNode, GroupAgentNodeExecutionProvider,
        GroupAgentNodeExecutionRequest, GroupAgentNodeExecutionResultPolicy,
        GroupAgentNodeExecutionWorkspace, GroupAgentNodeFailurePropagationOwner,
        GroupAgentNodePostClaimUncertainty, GroupAgentNodeProviderApproval,
        GroupAgentNodeProviderKind, GroupAgentNodeSameProjectPolicy, GroupAgentNodeWorkspaceMode,
        GroupAgentNodeWritebackPolicy, HubStoreError, group_agent_node_system_prompt,
        group_agent_node_user_prompt, group_agent_project_lane_sha256, group_agent_prompt_sha256,
    },
    sqlite_hub::{group_agent_graph, group_agent_graph_run},
};

use super::write;

#[test]
fn late_reread_fault_rolls_back_contract_event_and_run_transition() {
    let fixture = group_agent_graph::atomicity_tests::fixture();
    let graph = fixture
        .store
        .prepare_group_agent_graph(&fixture.request)
        .expect("prepare graph")
        .inspection;
    let run = fixture
        .store
        .begin_group_agent_graph_run(&group_agent_graph_run::atomicity_tests::request(&graph))
        .expect("prepare Graph Run")
        .inspection;
    let request = admission(&graph, &run);
    let mut connection = fixture.store.connect().expect("validated connection");

    let error = write::admit_with_before_reread(&mut connection, &request, || {
        Err(HubStoreError::Corrupt {
            message: "injected late reread fault".into(),
        })
    })
    .expect_err("late fault aborts admission");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_eq!(
        row_count(&connection, "group_agent_graph_node_execution_contracts"),
        0
    );
    assert_eq!(row_count(&connection, "group_agent_graph_run_events"), 1);
    assert_base_run(&connection);
    assert!(connection.is_autocommit());
}

fn admission(
    graph: &GroupAgentGraphInspection,
    run: &GroupAgentGraphRunInspection,
) -> AdmitGroupAgentNodeExecutionContract {
    let snapshot = snapshot(graph, run);
    let contract = contract(&snapshot);
    let event = event(&contract);
    AdmitGroupAgentNodeExecutionContract {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        graph_run_id: run.run.graph_run_id.clone(),
        control_snapshot_json: snapshot.canonical_json().expect("snapshot JSON"),
        control_snapshot: snapshot,
        contract_json: contract.canonical_json().expect("contract JSON"),
        contract,
        event_json: event.canonical_json().expect("event JSON"),
        event,
        idempotency_key: "contract-key".into(),
        admitted_at_ms: 40,
    }
}

fn snapshot(
    graph: &GroupAgentGraphInspection,
    run: &GroupAgentGraphRunInspection,
) -> GroupAgentGraphControlSnapshot {
    let mut snapshot = GroupAgentGraphControlSnapshot {
        v: GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION,
        scheduler_protocol_version: run.run.scheduler_protocol_version,
        graph_run_version: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: run.run.graph_run_id.clone(),
        graph_id: run.run.graph_id.clone(),
        source_snapshot_sha256: run.run.source_snapshot_sha256.clone(),
        graph_manifest_sha256: run.run.graph_manifest_sha256.clone(),
        core_plan_sha256: run.run.plan_sha256.clone(),
        last_event_seq: 1,
        last_event_sha256: run.events[0].expected_sha256().expect("event digest"),
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan: run.plan.clone(),
        manifest: graph.manifest.clone(),
        snapshot_sha256: String::new(),
    };
    snapshot.snapshot_sha256 = snapshot.expected_sha256().expect("snapshot digest");
    snapshot
}

fn contract(snapshot: &GroupAgentGraphControlSnapshot) -> GroupAgentNodeExecutionContract {
    let source = &snapshot.manifest.nodes[0];
    let mut contract = GroupAgentNodeExecutionContract {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        scheduler_protocol_version: snapshot.scheduler_protocol_version,
        node_execution_protocol_version: GROUP_AGENT_NODE_EXECUTION_PROTOCOL_VERSION,
        graph_run_id: snapshot.graph_run_id.clone(),
        graph_id: snapshot.graph_id.clone(),
        source_snapshot_sha256: snapshot.source_snapshot_sha256.clone(),
        graph_manifest_sha256: snapshot.graph_manifest_sha256.clone(),
        core_plan_sha256: snapshot.core_plan_sha256.clone(),
        control_snapshot_sha256: snapshot.snapshot_sha256.clone(),
        expected_last_event_seq: snapshot.last_event_seq,
        expected_last_event_sha256: snapshot.last_event_sha256.clone(),
        node: node(source),
        workspace: workspace(),
        provider: provider(),
        request: execution_request(snapshot, source),
        budgets: budgets(),
        approval: approval(),
        result: result_policy(),
        failure: failure_policy(),
        execution_contract_present: true,
        dispatch_authority_released: false,
        contract_id: String::new(),
        contract_sha256: String::new(),
    };
    contract.contract_sha256 = contract.expected_sha256().expect("contract digest");
    contract.contract_id = format!("node-contract-{}", contract.contract_sha256);
    contract
}

fn node(source: &GroupAgentGraphNode) -> GroupAgentNodeExecutionNode {
    GroupAgentNodeExecutionNode {
        node_id: source.node_id.clone(),
        authored_node_index: 0,
        topology_wave_index: 0,
        attempt: 1,
        project_id: source.project_id.clone(),
        member_role: source.member_role.clone(),
        agent_profile: source.agent_profile.clone(),
        project_lane_sha256: group_agent_project_lane_sha256(&source.project_id),
        same_project_policy: GroupAgentNodeSameProjectPolicy::ExclusiveUntilTerminal,
    }
}

fn execution_request(
    snapshot: &GroupAgentGraphControlSnapshot,
    source: &GroupAgentGraphNode,
) -> GroupAgentNodeExecutionRequest {
    let system_prompt = group_agent_node_system_prompt(&snapshot.manifest.manager.instruction);
    let user_prompt =
        group_agent_node_user_prompt(&source.node_id, &source.task, &source.acceptance)
            .expect("user Prompt");
    let mut request = GroupAgentNodeExecutionRequest {
        system_prompt_bytes: system_prompt.len(),
        system_prompt_sha256: group_agent_prompt_sha256(&system_prompt),
        system_prompt,
        user_prompt_bytes: user_prompt.len(),
        user_prompt_sha256: group_agent_prompt_sha256(&user_prompt),
        user_prompt,
        predecessor_result_receipts: Vec::new(),
        tools: Vec::new(),
        request_sha256: String::new(),
    };
    request.request_sha256 = request.expected_sha256().expect("request digest");
    request
}

fn workspace() -> GroupAgentNodeExecutionWorkspace {
    GroupAgentNodeExecutionWorkspace {
        mode: GroupAgentNodeWorkspaceMode::None,
        root_identity: None,
        isolation_id: None,
        allowed_read_paths: Vec::new(),
    }
}

fn provider() -> GroupAgentNodeExecutionProvider {
    GroupAgentNodeExecutionProvider {
        kind: GroupAgentNodeProviderKind::OpenAiResponses,
        endpoint: "https://api.openai.com/v1/responses".into(),
        model: "gpt-5.6-sol".into(),
        store: false,
        stream: true,
    }
}

fn budgets() -> GroupAgentNodeExecutionBudgets {
    GroupAgentNodeExecutionBudgets {
        max_turns: 1,
        max_tool_calls: 0,
        max_output_tokens: 4_096,
        max_model_output_bytes: 65_536,
        max_model_events: 4_096,
        timeout_ms: 60_000,
        max_cost_usd_micros: 1_000_000,
        pricing_snapshot_sha256: "4".repeat(64),
    }
}

fn approval() -> GroupAgentNodeExecutionApproval {
    GroupAgentNodeExecutionApproval {
        provider_dispatch: GroupAgentNodeProviderApproval::FreshOffMachineConsent,
        workspace: GroupAgentNodeEffectApproval::Forbidden,
        tools: GroupAgentNodeEffectApproval::Forbidden,
        writeback: GroupAgentNodeEffectApproval::Forbidden,
    }
}

fn result_policy() -> GroupAgentNodeExecutionResultPolicy {
    GroupAgentNodeExecutionResultPolicy {
        artifact_kind: GroupAgentNodeArtifactKind::LocalGraphNodeArtifact,
        max_result_bytes: 524_288,
        predecessor_dataflow: GroupAgentNodeDataflowPolicy::None,
        conversation_writeback: GroupAgentNodeWritebackPolicy::None,
        prompt_writeback: GroupAgentNodeWritebackPolicy::None,
        memory_writeback: GroupAgentNodeWritebackPolicy::None,
    }
}

fn failure_policy() -> GroupAgentNodeExecutionFailurePolicy {
    GroupAgentNodeExecutionFailurePolicy {
        automatic_retry: false,
        lease_retry: false,
        post_claim_uncertainty: GroupAgentNodePostClaimUncertainty::DispatchUnknown,
        failure_propagation_owner: GroupAgentNodeFailurePropagationOwner::ForgeCore,
    }
}

fn event(contract: &GroupAgentNodeExecutionContract) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
        graph_run_id: contract.graph_run_id.clone(),
        seq: 2,
        kind: GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
            previous_event_sha256: contract.expected_last_event_sha256.clone(),
            control_snapshot_sha256: contract.control_snapshot_sha256.clone(),
            contract_id: contract.contract_id.clone(),
            contract_sha256: contract.contract_sha256.clone(),
            contract_bytes: contract.canonical_json().expect("contract JSON").len(),
            node_id: contract.node.node_id.clone(),
            attempt: 1,
            request_sha256: contract.request.request_sha256.clone(),
            project_lane_sha256: contract.node.project_lane_sha256.clone(),
            admitted_at_ms: 40,
        },
    }
}

fn assert_base_run(connection: &rusqlite::Connection) {
    let row: (i64, String, i64, i64) = connection
        .query_row(
            "SELECT run_version,status,execution_contract_present,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("base Graph Run");
    assert_eq!(row, (1, "awaiting_execution_contract".into(), 0, 1));
}

fn row_count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
