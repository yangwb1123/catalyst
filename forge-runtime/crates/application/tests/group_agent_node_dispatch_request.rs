#![allow(dead_code)]

mod group_agent_node_execution_support;

use std::sync::Arc;

use forge_runtime_application::{
    AdmitGroupAgentNodeExecutionContractInput, GroupAgentNodeDispatchRequestCodec,
    GroupAgentNodeDispatchRequestService, GroupAgentNodeDispatchRequestServiceError,
    GroupAgentNodeExecutionContractService, PrepareGroupAgentNodeDispatchRequestDisposition,
    PrepareGroupAgentNodeDispatchRequestInput,
};
use forge_runtime_domain::{GroupAgentGraphRunEventKind, Message, ModelRequest, ProviderError};

use group_agent_node_execution_support::{FixtureBundle, MemoryContractHub, fixture};

struct ExactTestCodec;

impl GroupAgentNodeDispatchRequestCodec for ExactTestCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        let Message::User { text } = &request.messages[0] else {
            return Err(ProviderError::new(
                "shape",
                "expected one user message",
                false,
            ));
        };
        Ok(format!(
            "{model}\0{}\0{text}\0{}",
            request.system_prompt, request.max_output_tokens
        )
        .into_bytes())
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        let encoded = self.encode_request(model, expected)?;
        (encoded == actual)
            .then_some(())
            .ok_or_else(|| ProviderError::new("mismatch", "request bytes disagree", false))
    }
}

#[test]
fn prepare_persists_exact_bytes_and_advances_only_to_waiting_authorization() {
    let fixture = fixture();
    let (service, contract_service, _) = harness(&fixture);
    admit(&contract_service, &fixture);
    let result = service
        .prepare(&input(&fixture.run.run.graph_run_id, "dispatch-key", 90))
        .expect("prepare dispatch request");

    assert_eq!(
        result.disposition,
        PrepareGroupAgentNodeDispatchRequestDisposition::Created
    );
    assert_eq!(result.inspection.record.created_at_ms, 90);
    assert_eq!(result.inspection.contract.graph_run.run.v, 3);
    assert_eq!(result.inspection.contract.graph_run.run.last_event_seq, 3);
    assert!(
        result
            .inspection
            .contract
            .graph_run
            .run
            .dispatch_request_present
    );
    assert!(
        !result
            .inspection
            .contract
            .graph_run
            .run
            .dispatch_authority_released
    );
    assert_ne!(
        result.inspection.record.request_sha256,
        result.inspection.record.provider_request_sha256
    );
    assert_eq!(
        result.inspection.record.dispatch_request_sha256,
        result
            .inspection
            .record
            .expected_sha256()
            .expect("dispatch digest")
    );
    assert!(matches!(
        result.inspection.preparation_event.kind,
        GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared { .. }
    ));
}

#[test]
fn exact_replay_preserves_original_id_time_event_and_body() {
    let fixture = fixture();
    let (service, contract_service, _) = harness(&fixture);
    admit(&contract_service, &fixture);
    let created = service
        .prepare(&input(&fixture.run.run.graph_run_id, "dispatch-key", 90))
        .expect("create");
    let replayed = service
        .prepare(&input(&fixture.run.run.graph_run_id, "dispatch-key", 999))
        .expect("replay");

    assert_eq!(
        replayed.disposition,
        PrepareGroupAgentNodeDispatchRequestDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_eq!(replayed.inspection.record.created_at_ms, 90);
}

#[test]
fn prepare_conflict_does_not_mint_a_second_request() {
    let fixture = fixture();
    let (service, contract_service, _) = harness(&fixture);
    admit(&contract_service, &fixture);
    service
        .prepare(&input(&fixture.run.run.graph_run_id, "dispatch-key", 90))
        .expect("create");

    assert!(matches!(
        service.prepare(&input(&fixture.run.run.graph_run_id, "other-key", 91)),
        Err(GroupAgentNodeDispatchRequestServiceError::Conflict { .. })
    ));
}

#[test]
fn prepare_reports_a_missing_projected_contract_as_corrupt() {
    let fixture = fixture();
    let (service, contract_service, hub) = harness(&fixture);
    admit(&contract_service, &fixture);
    hub.set_list(Vec::new());

    assert!(matches!(
        service.prepare(&input(&fixture.run.run.graph_run_id, "dispatch-key", 90)),
        Err(GroupAgentNodeDispatchRequestServiceError::Corrupt { .. })
    ));
}

#[test]
fn inspect_and_list_revalidate_v3_source_contract_and_exact_codec_bytes() {
    let fixture = fixture();
    let (service, contract_service, _) = harness(&fixture);
    let contract = admit(&contract_service, &fixture);
    let created = service
        .prepare(&input(&fixture.run.run.graph_run_id, "dispatch-key", 90))
        .expect("create");
    let id = &created.inspection.record.dispatch_request_id;

    assert_eq!(service.inspect(id).expect("inspect"), created.inspection);
    assert_eq!(
        service
            .list(Some(&fixture.run.run.graph_run_id), 10)
            .expect("list"),
        vec![created.inspection.record]
    );
    contract_service
        .inspect(&contract.record.contract_id)
        .expect("contract remains inspectable after v3");
}

fn harness(
    fixture: &FixtureBundle,
) -> (
    GroupAgentNodeDispatchRequestService,
    GroupAgentNodeExecutionContractService,
    Arc<MemoryContractHub>,
) {
    let hub = Arc::new(MemoryContractHub::new(fixture));
    let contracts =
        GroupAgentNodeExecutionContractService::new(hub.clone(), hub.clone(), hub.clone());
    let dispatch = GroupAgentNodeDispatchRequestService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        Arc::new(ExactTestCodec),
    );
    (dispatch, contracts, hub)
}

fn admit(
    service: &GroupAgentNodeExecutionContractService,
    fixture: &FixtureBundle,
) -> forge_runtime_application::GroupAgentNodeExecutionContractInspection {
    service
        .admit(&AdmitGroupAgentNodeExecutionContractInput {
            graph_run_id: fixture.run.run.graph_run_id.clone(),
            contract_json: fixture.contract_json.clone(),
            idempotency_key: "contract-key".into(),
            admitted_at_ms: 80,
        })
        .expect("admit contract")
        .inspection
}

fn input(
    graph_run_id: &str,
    key: &str,
    prepared_at_ms: u64,
) -> PrepareGroupAgentNodeDispatchRequestInput {
    PrepareGroupAgentNodeDispatchRequestInput {
        graph_run_id: graph_run_id.into(),
        idempotency_key: key.into(),
        prepared_at_ms,
    }
}
