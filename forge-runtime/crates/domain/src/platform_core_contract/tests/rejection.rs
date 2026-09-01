use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use serde::Deserialize;
use serde_json::Value;

use super::receipt_fixture;
use crate::platform_core_contract::{
    ActionState, ActorType, AttemptState, ExecutionReceipt, MAX_RECEIPT_BYTES,
    PlatformCoreContractError, RecordRef, VerificationReceipt, WorkItemState,
    decode_canonical_artifact_ref, decode_canonical_command_envelope,
    decode_canonical_event_envelope, decode_canonical_execution_receipt,
    decode_canonical_verification_receipt, decode_canonical_verification_request,
    validate_action_transition, validate_attempt_transition, validate_execution_receipt,
    validate_verification_exchange, validate_verification_receipt, validate_work_item_transition,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RejectionCorpus {
    api_version: String,
    evidence_ref_cases: Vec<EvidenceRefCase>,
    executor_role_cases: Vec<ExecutorRoleCase>,
    state_machines: Vec<StateMachineCase>,
    transition_cases: Vec<TransitionCase>,
    wire_cases: Vec<WireCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceRefCase {
    arrangement: String,
    case_id: String,
    count: usize,
    expected_code: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutorRoleCase {
    actor_type: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateMachineCase {
    allowed_targets: HashMap<String, Vec<String>>,
    machine: String,
    states: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionCase {
    case_id: String,
    expected_code: String,
    machine: String,
    source: String,
    target: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCase {
    case_id: String,
    expected_code: String,
    mutations: Vec<Mutation>,
    target: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    operation: String,
    pointer: String,
    value: Value,
}

#[test]
fn shared_receipt_rejection_corpus_has_exact_codes() {
    let corpus = rejection_corpus();
    assert_eq!(
        corpus.api_version,
        "forge.platform-core-rejection-corpus/v1"
    );
    assert_corpus_coverage(&corpus);
    let fixture = receipt_fixture();
    assert_evidence_ref_corpus(&corpus.evidence_ref_cases, &fixture.verification_receipt);
    assert_executor_role_corpus(&corpus.executor_role_cases, &fixture.execution_receipt);
    assert_state_machine_corpus(&corpus.state_machines);
    let root = receipt_fixture_value();
    for test_case in corpus.wire_cases {
        let field = if test_case.target == "verification_exchange" {
            "verification_receipt"
        } else {
            &test_case.target
        };
        let mut value = root[field].clone();
        for mutation in &test_case.mutations {
            apply_mutation(&mut value, mutation);
        }
        let raw = super::super::wire::canonical_value(&value, MAX_RECEIPT_BYTES).unwrap();
        let error = rejected_wire(&test_case.target, raw.as_bytes(), &fixture);
        assert_eq!(
            error.code.as_str(),
            test_case.expected_code,
            "{}: {error}",
            test_case.case_id
        );
    }
    for test_case in corpus.transition_cases {
        let error = rejected_transition(&test_case);
        assert_eq!(
            error.code.as_str(),
            test_case.expected_code,
            "{}: {error}",
            test_case.case_id
        );
    }
}

fn assert_evidence_ref_corpus(cases: &[EvidenceRefCase], source: &VerificationReceipt) {
    assert_eq!(cases.len(), 4);
    let (result_index, template) = source
        .results
        .iter()
        .enumerate()
        .find_map(|(index, result)| result.evidence_refs.first().map(|value| (index, value)))
        .expect("receipt golden evidence template");
    for test_case in cases {
        let mut receipt = source.clone();
        receipt.results[result_index].evidence_refs = corpus_evidence_refs(template, test_case);
        let result = validate_verification_receipt(&receipt);
        match &test_case.expected_code {
            None => result.unwrap_or_else(|error| panic!("{}: {error}", test_case.case_id)),
            Some(expected) => assert_eq!(
                result.unwrap_err().code.as_str(),
                expected,
                "{}",
                test_case.case_id
            ),
        }
    }
}

fn corpus_evidence_refs(template: &RecordRef, test_case: &EvidenceRefCase) -> Vec<RecordRef> {
    let mut values: Vec<RecordRef> = (0..test_case.count)
        .map(|index| {
            let mut value = template.clone();
            value.record_id = format!("evidence-{index:02}");
            value
        })
        .collect();
    match test_case.arrangement.as_str() {
        "ascending" => {}
        "duplicate" => values
            .iter_mut()
            .for_each(|value| value.record_id = "evidence-00".into()),
        "descending" => values.reverse(),
        arrangement => panic!("unknown evidence arrangement {arrangement}"),
    }
    values
}

fn assert_executor_role_corpus(cases: &[ExecutorRoleCase], source: &ExecutionReceipt) {
    let actual: HashSet<&str> = cases.iter().map(|case| case.actor_type.as_str()).collect();
    let expected = HashSet::from(["agent", "service", "system"]);
    assert_eq!(cases.len(), expected.len());
    assert_eq!(actual, expected);
    for test_case in cases {
        let actor_type = match test_case.actor_type.as_str() {
            "agent" => ActorType::Agent,
            "service" => ActorType::Service,
            "system" => ActorType::System,
            role => panic!("unknown executor role {role}"),
        };
        let mut receipt = source.clone();
        receipt.executor.actor_ref.actor_type = actor_type;
        validate_execution_receipt(&receipt)
            .unwrap_or_else(|error| panic!("executor role {}: {error}", test_case.actor_type));
    }
}

fn assert_state_machine_corpus(cases: &[StateMachineCase]) {
    let expected_states = HashMap::from([("work_item", 12), ("attempt", 8), ("action", 9)]);
    assert_eq!(cases.len(), expected_states.len());
    let mut seen = HashSet::new();
    let mut total_edges = 0;
    for test_case in cases {
        assert!(seen.insert(test_case.machine.as_str()));
        assert_eq!(
            test_case.states.len(),
            *expected_states
                .get(test_case.machine.as_str())
                .expect("known state machine")
        );
        let edges = state_machine_edges(test_case);
        total_edges += edges.len();
        assert_all_state_pairs(test_case, &edges);
    }
    assert_eq!(total_edges, 61);
}

fn state_machine_edges(test_case: &StateMachineCase) -> HashSet<(String, String)> {
    let states: HashSet<&str> = test_case.states.iter().map(String::as_str).collect();
    assert_eq!(states.len(), test_case.states.len());
    assert_eq!(test_case.allowed_targets.len(), states.len());
    let mut edges = HashSet::new();
    for (source, targets) in &test_case.allowed_targets {
        assert!(states.contains(source.as_str()));
        for target in targets {
            assert!(states.contains(target.as_str()));
            assert!(edges.insert((source.clone(), target.clone())));
        }
    }
    edges
}

fn assert_all_state_pairs(test_case: &StateMachineCase, allowed: &HashSet<(String, String)>) {
    for source in &test_case.states {
        for target in &test_case.states {
            let result = transition_result(&test_case.machine, source, target);
            if allowed.contains(&(source.clone(), target.clone())) {
                result.unwrap_or_else(|error| {
                    panic!("{} {source} -> {target}: {error}", test_case.machine)
                });
            } else {
                assert_eq!(result.unwrap_err().code.as_str(), "pc_transition_invalid");
            }
        }
    }
}

fn assert_corpus_coverage(corpus: &RejectionCorpus) {
    assert_eq!(corpus.wire_cases.len(), 62);
    assert_eq!(corpus.transition_cases.len(), 9);
    let covered: HashSet<&str> = corpus
        .wire_cases
        .iter()
        .map(|test_case| test_case.expected_code.as_str())
        .chain(
            corpus
                .transition_cases
                .iter()
                .map(|test_case| test_case.expected_code.as_str()),
        )
        .collect();
    let required = HashSet::from([
        "pc_document_invalid",
        "pc_identifier_invalid",
        "pc_value_invalid",
        "pc_reference_mismatch",
        "pc_state_invalid",
        "pc_transition_invalid",
        "pc_relation_mismatch",
    ]);
    assert_eq!(covered, required);
}

#[test]
fn declared_state_edges_accept_only_explicit_transitions() {
    validate_work_item_transition(&WorkItemState::Verifying, &WorkItemState::Completed).unwrap();
    validate_attempt_transition(&AttemptState::Running, &AttemptState::Completed).unwrap();
    validate_action_transition(&ActionState::Started, &ActionState::Finished).unwrap();
}

fn rejection_corpus() -> RejectionCorpus {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/platform-core-rejection-corpus-v1.json");
    let raw = fs::read(path).unwrap();
    serde_json::from_slice(&raw).unwrap()
}

fn receipt_fixture_value() -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/contracts/fixtures");
    let receipt_path = root.join("platform-core-receipt-v1.json");
    let envelope_path = root.join("platform-core-envelope-v1.json");
    let mut receipt: Value = serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();
    let envelope: Value = serde_json::from_slice(&fs::read(envelope_path).unwrap()).unwrap();
    for field in ["artifact_ref", "command_envelope", "event_envelope"] {
        receipt
            .as_object_mut()
            .unwrap()
            .insert(field.to_owned(), envelope[field].clone());
    }
    receipt
}

fn apply_mutation(root: &mut Value, mutation: &Mutation) {
    let (parent_pointer, raw_key) = mutation.pointer.rsplit_once('/').unwrap();
    let parent = if parent_pointer.is_empty() {
        root
    } else {
        root.pointer_mut(parent_pointer)
            .expect("mutation parent exists")
    };
    match parent {
        Value::Object(object) => match mutation.operation.as_str() {
            "replace" => {
                object.insert(raw_key.to_owned(), mutation.value.clone());
            }
            "remove" => {
                assert!(object.remove(raw_key).is_some(), "mutation field exists");
            }
            operation => panic!("unsupported mutation operation {operation}"),
        },
        Value::Array(array) => {
            assert_eq!(mutation.operation, "replace");
            let index: usize = raw_key.parse().unwrap();
            array[index] = mutation.value.clone();
        }
        _ => panic!("mutation parent must be object or array"),
    }
}

fn rejected_wire(
    target: &str,
    raw: &[u8],
    fixture: &super::ReceiptGoldenFixture,
) -> PlatformCoreContractError {
    match target {
        "execution_receipt" => decode_canonical_execution_receipt(raw).unwrap_err(),
        "verification_request" => decode_canonical_verification_request(raw).unwrap_err(),
        "verification_receipt" => decode_canonical_verification_receipt(raw).unwrap_err(),
        "verification_exchange" => {
            let receipt = decode_canonical_verification_receipt(raw).unwrap();
            validate_verification_exchange(&fixture.verification_request, &receipt).unwrap_err()
        }
        "event_envelope" => decode_canonical_event_envelope(raw).unwrap_err(),
        "artifact_ref" => decode_canonical_artifact_ref(raw).unwrap_err(),
        "command_envelope" => decode_canonical_command_envelope(raw).unwrap_err(),
        _ => panic!("unknown rejection target {target}"),
    }
}

fn rejected_transition(test_case: &TransitionCase) -> PlatformCoreContractError {
    transition_result(&test_case.machine, &test_case.source, &test_case.target).unwrap_err()
}

fn transition_result(
    machine: &str,
    source: &str,
    target: &str,
) -> Result<(), PlatformCoreContractError> {
    match machine {
        "work_item" => {
            validate_work_item_transition(&work_item_state(source), &work_item_state(target))
        }
        "attempt" => validate_attempt_transition(&attempt_state(source), &attempt_state(target)),
        "action" => validate_action_transition(&action_state(source), &action_state(target)),
        _ => panic!("unknown state machine {machine}"),
    }
}

fn work_item_state(value: &str) -> WorkItemState {
    match value {
        "draft" => WorkItemState::Draft,
        "planned" => WorkItemState::Planned,
        "awaiting_approval" => WorkItemState::AwaitingApproval,
        "ready" => WorkItemState::Ready,
        "dispatched" => WorkItemState::Dispatched,
        "verifying" => WorkItemState::Verifying,
        "completed" => WorkItemState::Completed,
        "blocked" => WorkItemState::Blocked,
        "failed" => WorkItemState::Failed,
        "uncertain" => WorkItemState::Uncertain,
        "cancelled" => WorkItemState::Cancelled,
        "running" => WorkItemState::Running,
        other => WorkItemState::Unknown(other.to_owned()),
    }
}

fn attempt_state(value: &str) -> AttemptState {
    match value {
        "requested" => AttemptState::Requested,
        "accepted" => AttemptState::Accepted,
        "starting" => AttemptState::Starting,
        "interrupted" => AttemptState::Interrupted,
        "completed" => AttemptState::Completed,
        "failed" => AttemptState::Failed,
        "uncertain" => AttemptState::Uncertain,
        "running" => AttemptState::Running,
        other => AttemptState::Unknown(other.to_owned()),
    }
}

fn action_state(value: &str) -> ActionState {
    match value {
        "requested" => ActionState::Requested,
        "awaiting_approval" => ActionState::AwaitingApproval,
        "approved" => ActionState::Approved,
        "finished" => ActionState::Finished,
        "rejected" => ActionState::Rejected,
        "failed" => ActionState::Failed,
        "cancelled" => ActionState::Cancelled,
        "uncertain" => ActionState::Uncertain,
        "started" => ActionState::Started,
        other => ActionState::Unknown(other.to_owned()),
    }
}
