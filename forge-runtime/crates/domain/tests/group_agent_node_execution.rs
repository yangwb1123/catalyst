use std::fmt::Write;

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION, GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
    GroupAgentGraphControlSnapshot, GroupAgentNodeExecutionContract,
    group_agent_node_system_prompt, group_agent_node_user_prompt,
};
use serde::Deserialize;

const SNAPSHOT_SHA: &str = "5ad45490af64a1129a668e6c5c9824ff1ecc403fe30066a08fecc914bfba30ba";
const REQUEST_SHA: &str = "70923d2a0396f412f5c048d698c2ceafe13195788638bdea73fae9dcf732d325";
const CONTRACT_SHA: &str = "25cffbc860138b8e3a90f4e7e98fabdd56d6c7175d5f8f634809e6e6a7b4793c";

#[derive(Deserialize)]
struct GoldenFixture {
    v: u16,
    input: GoldenInput,
    expected: GoldenExpected,
}

#[derive(Deserialize)]
struct GoldenInput {
    canonical_control_snapshot_json: String,
}

#[derive(Deserialize)]
struct GoldenExpected {
    selected_node_id: String,
    canonical_user_prompt_json: String,
    canonical_request_payload_json: String,
    request_sha256: String,
    canonical_contract_payload_json: String,
    contract_sha256: String,
    contract_id: String,
    canonical_contract_json: String,
}

#[test]
fn rust_locks_every_shared_go_contract_byte_and_digest() {
    let fixture = fixture();
    let snapshot: GroupAgentGraphControlSnapshot =
        serde_json::from_str(&fixture.input.canonical_control_snapshot_json)
            .expect("decode shared control snapshot");
    let contract: GroupAgentNodeExecutionContract =
        serde_json::from_str(&fixture.expected.canonical_contract_json)
            .expect("decode shared Node Execution Contract");

    assert_snapshot(&fixture, &snapshot);
    assert_contract(&fixture, &contract);
    snapshot.validate().expect("valid shared control snapshot");
    contract.validate().expect("valid shared contract");
}

fn assert_snapshot(fixture: &GoldenFixture, snapshot: &GroupAgentGraphControlSnapshot) {
    assert_eq!(fixture.v, GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION);
    assert_eq!(snapshot.v, GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION);
    assert_eq!(snapshot.snapshot_sha256, SNAPSHOT_SHA);
    assert_eq!(
        snapshot.expected_sha256().expect("snapshot digest"),
        SNAPSHOT_SHA
    );
    assert_eq!(
        snapshot.canonical_json().expect("snapshot bytes"),
        fixture.input.canonical_control_snapshot_json
    );
    assert_eq!(
        payload_without_final_fields(
            &fixture.input.canonical_control_snapshot_json,
            &[("snapshot_sha256", SNAPSHOT_SHA)]
        ),
        snapshot_payload(snapshot)
    );
}

fn assert_contract(fixture: &GoldenFixture, contract: &GroupAgentNodeExecutionContract) {
    assert_eq!(contract.node.node_id, fixture.expected.selected_node_id);
    assert_eq!(contract.request.request_sha256, REQUEST_SHA);
    assert_eq!(
        contract.request.request_sha256,
        fixture.expected.request_sha256
    );
    assert_eq!(
        contract.expected_sha256().expect("contract digest"),
        CONTRACT_SHA
    );
    assert_eq!(contract.contract_sha256, fixture.expected.contract_sha256);
    assert_eq!(contract.contract_id, fixture.expected.contract_id);
    assert_eq!(
        contract.canonical_json().expect("contract bytes"),
        fixture.expected.canonical_contract_json
    );
    assert_eq!(
        request_payload(contract),
        fixture.expected.canonical_request_payload_json
    );
    assert_eq!(
        contract_payload(contract),
        fixture.expected.canonical_contract_payload_json
    );
}

#[test]
fn prompts_match_the_shared_multibyte_fixture_exactly() {
    let fixture = fixture();
    let snapshot: GroupAgentGraphControlSnapshot =
        serde_json::from_str(&fixture.input.canonical_control_snapshot_json).expect("snapshot");
    let first = &snapshot.manifest.nodes[0];

    assert_eq!(
        group_agent_node_system_prompt(&snapshot.manifest.manager.instruction).len(),
        336
    );
    assert_eq!(
        group_agent_node_user_prompt(&first.node_id, &first.task, &first.acceptance)
            .expect("user Prompt"),
        fixture.expected.canonical_user_prompt_json
    );
    assert_eq!(fixture.expected.canonical_user_prompt_json.len(), 157);
}

#[test]
fn provider_endpoint_rejects_credentials_query_fragment_and_normalization() {
    let fixture = fixture();
    let mut contract: GroupAgentNodeExecutionContract =
        serde_json::from_str(&fixture.expected.canonical_contract_json).expect("contract");

    for endpoint in [
        "http://api.openai.com/v1/responses",
        "https://user@api.openai.com/v1/responses",
        "https://api.openai.com/v1/responses?key=value",
        "https://api.openai.com/v1/responses#fragment",
        "HTTPS://api.openai.com/v1/responses",
        "https://api.openai.com/v1/../responses",
        "https://api.openai.com/v1/%2e/responses",
        "https://API.openai.com/v1/responses",
        "https://api.openai.com:443/v1/responses",
        "https://[::1]/v1/responses",
        "https://api.openai.com/v1:responses",
    ] {
        contract.provider.endpoint = endpoint.into();
        resign(&mut contract);
        assert!(contract.validate().is_err(), "{endpoint}");
    }
}

#[test]
fn provider_endpoint_accepts_the_shared_canonical_subset() {
    let fixture = fixture();
    let mut contract: GroupAgentNodeExecutionContract =
        serde_json::from_str(&fixture.expected.canonical_contract_json).expect("contract");

    for endpoint in [
        "https://api.openai.com/v1/responses",
        "https://api.example",
        "https://localhost/",
        "https://127.0.0.1/v1/responses",
        "https://api.example:8443/v1-beta_1/~models.json",
        "https://api.example//v1/responses",
    ] {
        contract.provider.endpoint = endpoint.into();
        resign(&mut contract);
        contract
            .validate()
            .unwrap_or_else(|error| panic!("{endpoint}: {error}"));
    }
}

fn fixture() -> GoldenFixture {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("shared Go fixture")
}

fn request_payload(contract: &GroupAgentNodeExecutionContract) -> String {
    let full = serde_json::to_string(&contract.request).expect("request JSON");
    payload_without_final_fields(
        &full,
        &[("request_sha256", &contract.request.request_sha256)],
    )
}

fn contract_payload(contract: &GroupAgentNodeExecutionContract) -> String {
    let full = contract.canonical_json().expect("contract JSON");
    payload_without_final_fields(
        &full,
        &[
            ("contract_id", &contract.contract_id),
            ("contract_sha256", &contract.contract_sha256),
        ],
    )
}

fn snapshot_payload(snapshot: &GroupAgentGraphControlSnapshot) -> String {
    let full = snapshot.canonical_json().expect("snapshot JSON");
    payload_without_final_fields(&full, &[("snapshot_sha256", &snapshot.snapshot_sha256)])
}

fn payload_without_final_fields(full: &str, fields: &[(&str, &str)]) -> String {
    let mut suffix = String::new();
    for (name, value) in fields {
        write!(
            suffix,
            ",\"{name}\":{}",
            serde_json::to_string(value).expect("field JSON")
        )
        .expect("write suffix");
    }
    let suffix = format!("{suffix}}}");
    format!(
        "{}}}",
        full.strip_suffix(&suffix)
            .expect("digest identity fields are final")
    )
}

fn resign(contract: &mut GroupAgentNodeExecutionContract) {
    let digest = contract.expected_sha256().expect("rehash contract");
    contract.contract_id = format!("node-contract-{digest}");
    contract.contract_sha256 = digest;
}
