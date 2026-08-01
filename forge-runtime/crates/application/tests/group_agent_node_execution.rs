mod group_agent_node_execution_support;

use std::sync::Arc;

use forge_runtime_application::{
    AdmitGroupAgentNodeExecutionContractDisposition, AdmitGroupAgentNodeExecutionContractInput,
    GroupAgentGraphRunEventKind, GroupAgentNodeExecutionContractInspection,
    GroupAgentNodeExecutionContractService, GroupAgentNodeExecutionContractServiceError,
};

use group_agent_node_execution_support::{MemoryContractHub, fixture};

#[test]
fn export_control_reconstructs_exact_bytes_from_the_shared_topology() {
    let fixture = fixture();
    let (service, _) = harness(&fixture);
    let exported = service
        .export_control(&fixture.run.run.graph_run_id)
        .expect("export control");

    assert_eq!(exported.snapshot_json, fixture.snapshot_json);
    assert_eq!(
        exported.snapshot.snapshot_sha256,
        exported
            .snapshot
            .expected_sha256()
            .expect("snapshot digest")
    );
    assert!(!exported.snapshot.execution_contract_present);
    assert!(!exported.snapshot.dispatch_authority_released);
}

#[test]
fn admission_transitions_only_to_awaiting_core_dispatch() {
    let fixture = fixture();
    let (service, _) = harness(&fixture);
    let result = service
        .admit(&input(&fixture, "contract-key", 80))
        .expect("admit contract");

    assert_eq!(
        result.disposition,
        AdmitGroupAgentNodeExecutionContractDisposition::Created
    );
    assert_eq!(result.inspection.graph_run.run.v, 2);
    assert_eq!(result.inspection.graph_run.run.last_event_seq, 2);
    assert!(result.inspection.graph_run.run.execution_contract_present);
    assert!(!result.inspection.graph_run.run.dispatch_authority_released);
    assert_eq!(result.inspection.graph_run.events.len(), 2);
    assert_eq!(result.inspection.record.created_at_ms, 80);
}

#[test]
fn same_key_replay_from_v2_preserves_original_identity_time_event_and_bytes() {
    let fixture = fixture();
    let (service, _) = harness(&fixture);
    let created = service
        .admit(&input(&fixture, "contract-key", 80))
        .expect("create contract");
    let replayed = service
        .admit(&input(&fixture, "contract-key", 999))
        .expect("replay contract");

    assert_eq!(
        replayed.disposition,
        AdmitGroupAgentNodeExecutionContractDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_eq!(replayed.inspection.record.created_at_ms, 80);
    assert_eq!(
        replayed.inspection.admission_event_json,
        created.inspection.admission_event_json
    );
    assert_eq!(replayed.inspection.contract_json, fixture.contract_json);
}

#[test]
fn public_export_refuses_the_exact_v2_admitted_state() {
    let fixture = fixture();
    let (service, _) = harness(&fixture);
    service
        .admit(&input(&fixture, "contract-key", 80))
        .expect("admit contract");

    assert!(matches!(
        service.export_control(&fixture.run.run.graph_run_id),
        Err(GroupAgentNodeExecutionContractServiceError::Conflict { .. })
    ));
}

#[test]
fn malformed_or_noncanonical_contract_never_opens_the_hub() {
    for contract_json in invalid_contracts() {
        let fixture = fixture();
        let (service, hub) = harness(&fixture);
        let mut input = input(&fixture, "contract-key", 80);
        input.contract_json = contract_json;

        assert!(matches!(
            service.admit(&input),
            Err(GroupAgentNodeExecutionContractServiceError::InvalidInput { .. })
        ));
        assert_eq!(hub.run_reads(), 0);
    }
}

#[test]
fn show_and_list_revalidate_exact_metadata_and_filters() {
    let fixture = fixture();
    let (service, hub) = harness(&fixture);
    let created = service
        .admit(&input(&fixture, "contract-key", 80))
        .expect("admit");
    let shown = service
        .inspect(&created.inspection.record.contract_id)
        .expect("show");
    assert_eq!(shown, created.inspection);
    assert_eq!(
        service
            .list(Some(&fixture.run.run.graph_run_id), 10)
            .expect("list"),
        vec![shown.record.clone()]
    );

    hub.set_list(vec![shown.record.clone(), shown.record]);
    assert!(matches!(
        service.list(None, 10),
        Err(GroupAgentNodeExecutionContractServiceError::Corrupt { .. })
    ));
    assert!(matches!(
        service.list(None, 0),
        Err(GroupAgentNodeExecutionContractServiceError::InvalidInput { .. })
    ));
}

#[test]
fn show_rejects_self_consistent_contract_drift_from_reconstructed_control() {
    let fixture = fixture();
    let (service, hub) = harness(&fixture);
    let created = service
        .admit(&input(&fixture, "contract-key", 80))
        .expect("admit");
    let mut drifted = created.inspection;
    drift_contract_profile(&mut drifted);
    drifted.validate().expect("locally consistent inspection");
    let contract_id = drifted.record.contract_id.clone();
    hub.set_inspection(drifted);

    assert!(matches!(
        service.inspect(&contract_id),
        Err(GroupAgentNodeExecutionContractServiceError::Corrupt { .. })
    ));
}

fn harness(
    fixture: &group_agent_node_execution_support::FixtureBundle,
) -> (
    GroupAgentNodeExecutionContractService,
    Arc<MemoryContractHub>,
) {
    let hub = Arc::new(MemoryContractHub::new(fixture));
    let service =
        GroupAgentNodeExecutionContractService::new(hub.clone(), hub.clone(), hub.clone());
    (service, hub)
}

fn input(
    fixture: &group_agent_node_execution_support::FixtureBundle,
    key: &str,
    admitted_at_ms: u64,
) -> AdmitGroupAgentNodeExecutionContractInput {
    AdmitGroupAgentNodeExecutionContractInput {
        graph_run_id: fixture.run.run.graph_run_id.clone(),
        contract_json: fixture.contract_json.clone(),
        idempotency_key: key.into(),
        admitted_at_ms,
    }
}

fn invalid_contracts() -> Vec<String> {
    let fixture = fixture();
    vec![
        format!("{}\n", fixture.contract_json),
        fixture
            .contract_json
            .replacen("\"v\":1", "\"v\":1,\"unknown\":true", 1),
        fixture
            .contract_json
            .replacen("\"contract_sha256\":\"", "\"contract_sha256\":\"0", 1),
    ]
}

fn drift_contract_profile(inspection: &mut GroupAgentNodeExecutionContractInspection) {
    inspection.contract.node.agent_profile = "drifted-profile".into();
    let digest = inspection
        .contract
        .expected_sha256()
        .expect("contract digest");
    inspection.contract.contract_id = format!("node-contract-{digest}");
    inspection.contract.contract_sha256 = digest;
    inspection.contract_json = inspection.contract.canonical_json().expect("contract JSON");
    inspection.record.contract_id = inspection.contract.contract_id.clone();
    inspection.record.contract_sha256 = inspection.contract.contract_sha256.clone();
    inspection.record.contract_bytes = inspection.contract_json.len();
    let GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
        contract_id,
        contract_sha256,
        contract_bytes,
        ..
    } = &mut inspection.admission_event.kind
    else {
        panic!("admission event");
    };
    contract_id.clone_from(&inspection.contract.contract_id);
    contract_sha256.clone_from(&inspection.contract.contract_sha256);
    *contract_bytes = inspection.contract_json.len();
    inspection.admission_event_json = inspection
        .admission_event
        .canonical_json()
        .expect("event JSON");
    inspection.graph_run.events[1] = inspection.admission_event.clone();
    inspection.graph_run.event_jsons[1].clone_from(&inspection.admission_event_json);
    inspection.graph_run.run.journal_bytes = inspection
        .graph_run
        .event_jsons
        .iter()
        .map(String::len)
        .sum();
}
