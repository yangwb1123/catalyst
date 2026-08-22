use crate::kernel_operational_contract::ObservedUsage;

use super::{golden, *};

type ClosureMutation = fn(&mut KernelDecisionReferenceClosure);

fn blank_transaction() -> DecisionTransaction {
    let mut transaction = golden().decision_transaction;
    transaction.decision_transaction_id.clear();
    transaction.decision_transaction_sha256.clear();
    transaction
}

fn expect_error<T>(result: Result<T, KernelDecisionContractError>, expected: &str) {
    let Err(error) = result else {
        panic!("expected Kernel decision contract error: {expected}");
    };
    assert_eq!(error.message, expected);
}

fn selected_option(transaction: &mut DecisionTransaction) -> &mut DecisionOption {
    let selected = transaction.selected_option_id.clone();
    transaction
        .options
        .iter_mut()
        .find(|option| option.option_id == selected)
        .expect("selected option")
}

fn oversized_atom_set() -> Vec<CognitiveAtom> {
    let source = golden();
    let template = source
        .cognitive_atoms
        .iter()
        .find(|atom| atom.source.source_kind == "artifact" && atom.atom_type == "risk")
        .expect("artifact risk atom");
    let mut atoms = Vec::with_capacity(256);
    for index in 0..256 {
        let mut atom = template.clone();
        atom.atom_id.clear();
        atom.atom_sha256.clear();
        atom.proposition.object_value = serde_json::json!(format!("aggregate-{index:03}"));
        atom.source.source_selector = Some(format!("/{}", "x".repeat(4094)));
        atoms.push(seal_cognitive_atom(&atom).expect("individually valid atom"));
    }
    atoms.sort_by(|left, right| left.atom_id.as_bytes().cmp(right.atom_id.as_bytes()));
    atoms
}

fn exact_budget_closure() -> KernelDecisionReferenceClosure {
    let mut value = golden();
    value.decision_transaction.budget.max_calls = 2;
    value.decision_transaction.budget.max_cost_usd_micros = 20;
    value.decision_transaction.budget.timeout_ms = 700;
    value.decision_transaction.budget.max_input_tokens = 14;
    value.decision_transaction.budget.max_network_bytes = 18;
    value.decision_transaction.budget.max_output_bytes = 34;
    value.decision_transaction.budget.max_output_tokens = 6;
    value
}

#[test]
fn atom_inventory_and_public_byte_ceilings_reject_n_plus_one() {
    let source = golden();
    let atoms = vec![source.cognitive_atoms[0].clone(); 257];
    expect_error(
        super::super::atom::validate_atoms(&atoms),
        "cognitive_atoms cardinality must be 1..=256",
    );

    let atom_bytes = vec![b' '; MAX_ATOM_BYTES + 1];
    let transaction_bytes = vec![b' '; MAX_TRANSACTION_BYTES + 1];
    let closure_bytes = vec![b' '; MAX_CLOSURE_BYTES + 1];
    expect_error(
        decode_cognitive_atom(&atom_bytes),
        &format!("JSON byte length must be 1..={MAX_ATOM_BYTES}"),
    );
    expect_error(
        decode_decision_transaction(&transaction_bytes),
        &format!("JSON byte length must be 1..={MAX_TRANSACTION_BYTES}"),
    );
    expect_error(
        decode_closure(&closure_bytes),
        &format!("JSON byte length must be 1..={MAX_CLOSURE_BYTES}"),
    );
}

#[test]
fn source_selector_accepts_4096_and_rejects_4097_bytes() {
    let source = golden();
    let mut atom = source
        .cognitive_atoms
        .iter()
        .find(|atom| atom.source.source_kind == "artifact")
        .expect("artifact atom")
        .clone();
    atom.atom_id.clear();
    atom.atom_sha256.clear();
    atom.source.source_selector = Some(format!("/{}", "x".repeat(4095)));
    seal_cognitive_atom(&atom).expect("4096-byte selector");
    atom.source
        .source_selector
        .as_mut()
        .expect("selector")
        .push('x');
    expect_error(
        seal_cognitive_atom(&atom),
        "source_selector must be nonempty UTF-8 text <= 4096 bytes",
    );
}

#[test]
fn nullable_module_and_object_scope_seals() {
    let mut atom = golden().cognitive_atoms[0].clone();
    atom.atom_id.clear();
    atom.atom_sha256.clear();
    atom.scope.module = None;
    atom.scope.object = None;
    seal_cognitive_atom(&atom).expect("nullable scope");
}

#[test]
fn atom_set_aggregate_ceiling_rejects_256_valid_atoms() {
    let atoms = oversized_atom_set();
    assert_eq!(atoms.len(), 256);
    super::super::atom::validate_atoms(&atoms).expect("all atoms valid and ordered");
    let canonical = canonical_json(&atoms).expect("outer-sized canonical atom set");
    assert!(canonical.len() > MAX_ATOM_SET_BYTES);
    expect_error(
        super::super::wire::canonical_with_max(&atoms, MAX_ATOM_SET_BYTES),
        &format!("canonical JSON byte length must be 1..={MAX_ATOM_SET_BYTES}"),
    );
}

#[test]
fn option_and_proof_collections_reject_n_plus_one() {
    let mut transaction = blank_transaction();
    transaction.options = vec![transaction.options[0].clone(); 17];
    expect_error(
        seal_decision_transaction(&transaction),
        "options cardinality must be 1..=16",
    );

    let mut transaction = blank_transaction();
    transaction.proof_obligations = vec![transaction.proof_obligations[0].clone(); 33];
    expect_error(
        seal_decision_transaction(&transaction),
        "proof_obligations cardinality must be 1..=32",
    );

    let mut transaction = blank_transaction();
    transaction.proof_obligations[0].required_evidence_kinds = (0..17)
        .map(|index| format!("evidence-{index:02}"))
        .collect();
    expect_error(
        seal_decision_transaction(&transaction),
        "required_evidence_kinds cardinality is outside frozen bounds",
    );
}

#[test]
fn read_and_write_collections_reject_n_plus_one() {
    let mut transaction = blank_transaction();
    transaction.read_artifact_receipt_refs =
        vec![transaction.read_artifact_receipt_refs[0].clone(); 33];
    expect_error(
        seal_decision_transaction(&transaction),
        "read_artifact_receipt_refs cardinality must be 0..=32",
    );

    let mut transaction = blank_transaction();
    transaction.write_preconditions = vec![transaction.write_preconditions[0].clone(); 33];
    expect_error(
        seal_decision_transaction(&transaction),
        "write_preconditions cardinality must be 0..=32",
    );

    let mut transaction = blank_transaction();
    transaction.write_slots = (0..33).map(|index| format!("slot-{index:02}")).collect();
    expect_error(
        seal_decision_transaction(&transaction),
        "write_slots cardinality is outside frozen bounds",
    );
}

#[test]
fn trigger_and_guard_collections_reject_sixty_five_members() {
    let mut transaction = blank_transaction();
    transaction.trigger_atom_refs = vec![transaction.goal_atom_ref.clone(); 65];
    expect_error(
        seal_decision_transaction(&transaction),
        "trigger_atom_refs cardinality must be 0..=64",
    );

    let mut transaction = blank_transaction();
    transaction.guard_atom_refs = vec![transaction.goal_atom_ref.clone(); 65];
    expect_error(
        seal_decision_transaction(&transaction),
        "guard_atom_refs cardinality must be 0..=64",
    );
}

#[test]
fn zero_triggers_and_empty_read_write_declarations_seal() {
    let mut transaction = blank_transaction();
    transaction
        .guard_atom_refs
        .append(&mut transaction.trigger_atom_refs);
    transaction
        .guard_atom_refs
        .sort_by(|left, right| left.atom_id.as_bytes().cmp(right.atom_id.as_bytes()));
    transaction.read_artifact_receipt_refs.clear();
    transaction.write_preconditions.clear();
    transaction.write_slots.clear();
    seal_decision_transaction(&transaction).expect("zero/empty declarations");
}

#[test]
fn selected_operation_relations_are_independently_rejected() {
    let source = golden();
    let mutations: [fn(&mut DecisionTransaction); 5] = [
        |value| selected_option(value).capability.capability_id = "drifted.capability".to_owned(),
        |value| selected_option(value).requested_action_sha256 = "0".repeat(64),
        |value| value.idempotency_key = "drifted-idempotency".to_owned(),
        |value| value.read_artifact_receipt_refs.clear(),
        |value| value.write_slots.clear(),
    ];
    for mutation in mutations {
        let mut transaction = source.decision_transaction.clone();
        mutation(&mut transaction);
        expect_error(
            super::super::graph::validate_reference_graph(
                &source.cognitive_atoms,
                &transaction,
                &source.operational_closure,
            ),
            "Invocation differs from selected transaction declarations",
        );
    }
}

#[test]
fn invocation_event_and_receipt_correlations_are_independently_rejected() {
    let source = golden();
    let mutations: [(&str, ClosureMutation); 3] = [
        (
            "Invocation differs from selected transaction declarations",
            |value| {
                value.operational_closure.capability_invocations[0].correlation_id =
                    "drift".to_owned();
            },
        ),
        (
            "operational correlation differs from transaction",
            |value| {
                value.operational_closure.interaction_events[0].correlation_id = "drift".to_owned();
            },
        ),
        (
            "operational correlation differs from transaction",
            |value| {
                value.operational_closure.execution_receipts[0].correlation_id = "drift".to_owned();
            },
        ),
    ];
    for (expected, mutation) in mutations {
        let mut candidate = source.clone();
        mutation(&mut candidate);
        expect_error(
            super::super::graph::validate_reference_graph(
                &candidate.cognitive_atoms,
                &candidate.decision_transaction,
                &candidate.operational_closure,
            ),
            expected,
        );
    }
}

#[test]
fn every_budget_usage_dimension_rejects_n_plus_one() {
    let exact = exact_budget_closure();
    super::super::graph::budget_closure(&exact.decision_transaction, &exact.operational_closure)
        .expect("exact seven-dimensional aggregate");
    let mutations: [fn(&mut ObservedUsage); 7] = [
        |value| value.call_count += 1,
        |value| value.cost_usd_micros += 1,
        |value| value.elapsed_ms += 1,
        |value| value.input_tokens += 1,
        |value| value.network_bytes += 1,
        |value| value.output_bytes += 1,
        |value| value.output_tokens += 1,
    ];
    for mutation in mutations {
        let mut candidate = exact.clone();
        mutation(&mut candidate.operational_closure.execution_receipts[0].observed_usage);
        expect_error(
            super::super::graph::budget_closure(
                &candidate.decision_transaction,
                &candidate.operational_closure,
            ),
            "caller-declared aggregate usage exceeds transaction budget",
        );
    }
}

#[test]
fn invocation_count_and_every_usage_overflow_are_independently_rejected() {
    let mut invocation_count = exact_budget_closure();
    let extra = invocation_count.operational_closure.capability_invocations[0].clone();
    invocation_count
        .operational_closure
        .capability_invocations
        .push(extra);
    expect_error(
        super::super::graph::budget_closure(
            &invocation_count.decision_transaction,
            &invocation_count.operational_closure,
        ),
        "Invocation count exceeds transaction budget",
    );
    let setters: [fn(&mut ObservedUsage, i64); 7] = [
        |value, number| value.call_count = number,
        |value, number| value.cost_usd_micros = number,
        |value, number| value.elapsed_ms = number,
        |value, number| value.input_tokens = number,
        |value, number| value.network_bytes = number,
        |value, number| value.output_bytes = number,
        |value, number| value.output_tokens = number,
    ];
    for setter in setters {
        let mut candidate = exact_budget_closure();
        setter(
            &mut candidate.operational_closure.execution_receipts[0].observed_usage,
            i64::MAX,
        );
        setter(
            &mut candidate.operational_closure.execution_receipts[1].observed_usage,
            1,
        );
        expect_error(
            super::super::graph::budget_closure(
                &candidate.decision_transaction,
                &candidate.operational_closure,
            ),
            "caller-declared aggregate usage overflows signed int64",
        );
    }
}
