use serde_json::json;

use super::{golden, *};

fn atom_candidate(
    source: &KernelDecisionReferenceClosure,
    kind: &str,
    mutate: impl FnOnce(&mut CognitiveAtom),
) -> KernelDecisionReferenceClosure {
    let mut candidate = source.clone();
    candidate.closure_id.clear();
    candidate.closure_sha256.clear();
    let index = candidate
        .cognitive_atoms
        .iter()
        .position(|atom| atom.source.source_kind == kind)
        .expect("source kind");
    let mut atom = candidate.cognitive_atoms[index].clone();
    atom.atom_id.clear();
    atom.atom_sha256.clear();
    mutate(&mut atom);
    candidate.cognitive_atoms[index] = seal_cognitive_atom(&atom).expect("reseal atom");
    candidate
        .cognitive_atoms
        .sort_by(|left, right| left.atom_id.as_bytes().cmp(right.atom_id.as_bytes()));
    candidate
}

#[test]
fn fully_resealed_atom_task_binding_and_post_time_drift_rejected() {
    let value = golden();
    let candidate = atom_candidate(&value, "capability_invocation", |atom| {
        atom.task_binding.task_id = "drifted-task".to_owned();
    });
    assert!(seal_closure(&candidate).is_err());
    let candidate = atom_candidate(&value, "capability_invocation", |atom| {
        atom.bindings.context_sha256.replace_range(..1, "f");
    });
    assert!(seal_closure(&candidate).is_err());
    let created = value.decision_transaction.created_at_unix_ms;
    let candidate = atom_candidate(&value, "capability_invocation", |atom| {
        atom.validity.valid_from_unix_ms = created - 1;
    });
    assert!(seal_closure(&candidate).is_err());
    let candidate = transaction_candidate(&value, |transaction| {
        let selected = transaction
            .options
            .iter_mut()
            .find(|option| option.option_id == transaction.selected_option_id)
            .expect("selected");
        selected.requested_action_sha256.replace_range(..1, "0");
    });
    assert!(seal_closure(&candidate).is_err());
}

#[test]
fn declared_input_cannot_source_postdecision_atom() {
    let value = golden();
    let input = value
        .operational_closure
        .artifact_receipts
        .iter()
        .find(|receipt| receipt.receipt_role == "declared_input")
        .expect("input");
    let created = value.decision_transaction.created_at_unix_ms;
    let candidate = atom_candidate(&value, "artifact_receipt", |atom| {
        atom.source.source_ref = json!({
            "artifact_receipt_id": input.artifact_receipt_id,
            "artifact_receipt_sha256": input.artifact_receipt_sha256
        });
        atom.validity.valid_from_unix_ms = created;
    });
    assert!(seal_closure(&candidate).is_err());
}

#[test]
fn post_atom_cannot_fall_between_transaction_and_source() {
    let value = golden();
    let created = value.decision_transaction.created_at_unix_ms;
    let requested = value.operational_closure.capability_invocations[0].requested_at_unix_ms;
    let candidate = atom_candidate(&value, "capability_invocation", |atom| {
        atom.validity.valid_from_unix_ms = i64::midpoint(created, requested);
    });
    let error = seal_closure(&candidate).expect_err("post atom before source");
    assert!(error.message.contains("predates"));
}

#[test]
fn transaction_and_read_temporal_edges() {
    let value = golden();

    let mut after_request = value.clone();
    let first = after_request
        .operational_closure
        .capability_invocations
        .iter()
        .map(|invocation| invocation.requested_at_unix_ms)
        .min()
        .expect("invocation");
    after_request.decision_transaction.created_at_unix_ms = first + 1;

    let mut future_read = value.clone();
    let read_id = future_read.decision_transaction.read_artifact_receipt_refs[0]
        .artifact_receipt_id
        .clone();
    future_read
        .operational_closure
        .artifact_receipts
        .iter_mut()
        .find(|receipt| receipt.artifact_receipt_id == read_id)
        .expect("read receipt")
        .created_at_unix_ms = future_read.decision_transaction.created_at_unix_ms + 1;

    for candidate in [after_request, future_read] {
        assert!(
            super::super::graph::times(
                &candidate.cognitive_atoms,
                &candidate.decision_transaction,
                &candidate.operational_closure,
            )
            .is_err()
        );
    }
}

#[test]
fn predecision_atom_temporal_edges() {
    let value = golden();
    let mut future_atom = value.clone();
    future_atom
        .cognitive_atoms
        .iter_mut()
        .find(|atom| atom.source.source_phase == "predecision")
        .expect("predecision atom")
        .validity
        .valid_from_unix_ms = future_atom.decision_transaction.created_at_unix_ms + 1;

    let mut expired_atom = value;
    expired_atom
        .cognitive_atoms
        .iter_mut()
        .find(|atom| atom.source.source_phase == "predecision")
        .expect("predecision atom")
        .validity
        .valid_until_unix_ms = Some(expired_atom.decision_transaction.created_at_unix_ms);
    for candidate in [future_atom, expired_atom] {
        assert!(
            super::super::graph::times(
                &candidate.cognitive_atoms,
                &candidate.decision_transaction,
                &candidate.operational_closure,
            )
            .is_err()
        );
    }
}

fn transaction_candidate(
    source: &KernelDecisionReferenceClosure,
    mutate: impl FnOnce(&mut DecisionTransaction),
) -> KernelDecisionReferenceClosure {
    let mut candidate = source.clone();
    candidate.closure_id.clear();
    candidate.closure_sha256.clear();
    let mut transaction = candidate.decision_transaction.clone();
    transaction.decision_transaction_id.clear();
    transaction.decision_transaction_sha256.clear();
    mutate(&mut transaction);
    candidate.decision_transaction =
        seal_decision_transaction(&transaction).expect("reseal transaction");
    candidate
}

#[test]
fn goal_atom_ref_rejects_non_goal_atom() {
    let value = golden();
    let wrong_goal = transaction_candidate(&value, |transaction| {
        let prior_goal = transaction.goal_atom_ref.clone();
        transaction.goal_atom_ref = transaction.trigger_atom_refs[0].clone();
        transaction.trigger_atom_refs[0] = prior_goal;
    });
    let error =
        super::super::graph::role_closure(&value.cognitive_atoms, &wrong_goal.decision_transaction)
            .expect_err("non-goal goal_atom_ref");
    assert!(error.message.contains("only predecision goal"));
}

#[test]
fn trigger_and_guard_reject_additional_goal_atom() {
    let value = golden();
    let mut atoms = value.cognitive_atoms.clone();
    let index = atoms
        .iter()
        .position(|atom| {
            atom.source.source_phase == "predecision" && atom.atom_type == "preference"
        })
        .expect("predecision preference");
    let old_id = atoms[index].atom_id.clone();
    let mut changed = atoms[index].clone();
    changed.atom_id.clear();
    changed.atom_sha256.clear();
    changed.atom_type = "goal".to_owned();
    atoms[index] = seal_cognitive_atom(&changed).expect("resealed additional goal");
    let replacement = AtomRef {
        atom_id: atoms[index].atom_id.clone(),
        atom_sha256: atoms[index].atom_sha256.clone(),
    };
    atoms.sort_by(|left, right| left.atom_id.as_bytes().cmp(right.atom_id.as_bytes()));
    let extra_goal = transaction_candidate(&value, |transaction| {
        for reference in transaction
            .trigger_atom_refs
            .iter_mut()
            .chain(transaction.guard_atom_refs.iter_mut())
        {
            if reference.atom_id == old_id {
                *reference = replacement.clone();
            }
        }
        transaction
            .trigger_atom_refs
            .sort_by(|left, right| left.atom_id.as_bytes().cmp(right.atom_id.as_bytes()));
        transaction
            .guard_atom_refs
            .sort_by(|left, right| left.atom_id.as_bytes().cmp(right.atom_id.as_bytes()));
    });
    let error = super::super::graph::role_closure(&atoms, &extra_goal.decision_transaction)
        .expect_err("additional goal in trigger/guard");
    assert!(error.message.contains("only predecision goal"));
}

#[test]
fn non_goal_trigger_and_guard_roles_remain_untyped() {
    let value = golden();
    let candidate = transaction_candidate(&value, |transaction| {
        let trigger = transaction.trigger_atom_refs[0].clone();
        transaction.trigger_atom_refs[0] = transaction.guard_atom_refs[0].clone();
        transaction.guard_atom_refs[0] = trigger;
        transaction
            .guard_atom_refs
            .sort_by(|left, right| left.atom_id.as_bytes().cmp(right.atom_id.as_bytes()));
    });
    super::super::graph::role_closure(&value.cognitive_atoms, &candidate.decision_transaction)
        .expect("non-goal trigger/guard swap");
}

#[test]
fn selected_projection_and_orphan_role_drift_rejected() {
    let value = golden();
    let mutations: [fn(&mut DecisionTransaction); 4] = [
        |value| value.idempotency_key = "drifted-idempotency".to_owned(),
        |value| value.read_artifact_receipt_refs.clear(),
        |value| value.write_slots.clear(),
        |value| {
            value.guard_atom_refs.remove(0);
        },
    ];
    for mutation in mutations {
        assert!(seal_closure(&transaction_candidate(&value, mutation)).is_err());
    }
    let candidate = transaction_candidate(&value, |transaction| {
        let selected = transaction
            .options
            .iter_mut()
            .find(|option| option.option_id == transaction.selected_option_id)
            .expect("selected");
        selected.capability.capability_id = "drifted.capability".to_owned();
    });
    assert!(seal_closure(&candidate).is_err());
}

#[test]
fn every_operational_record_class_binding_drift_rejected() {
    let value = golden();
    for index in 0..4 {
        let mut candidate = value.clone();
        candidate.closure_id.clear();
        candidate.closure_sha256.clear();
        let binding = match index {
            0 => &mut candidate.operational_closure.artifact_receipts[0].bindings,
            1 => &mut candidate.operational_closure.capability_invocations[0].bindings,
            2 => &mut candidate.operational_closure.interaction_events[0].bindings,
            _ => &mut candidate.operational_closure.execution_receipts[0].bindings,
        };
        binding.context_sha256.replace_range(..1, "f");
        assert!(seal_closure(&candidate).is_err());
    }
}

#[test]
fn exact_aggregate_n_n_plus_one_and_overflow() {
    let value = golden();
    let mut transaction = value.decision_transaction;
    transaction.budget.max_calls = 2;
    transaction.budget.max_cost_usd_micros = 20;
    transaction.budget.timeout_ms = 700;
    transaction.budget.max_input_tokens = 14;
    transaction.budget.max_network_bytes = 18;
    transaction.budget.max_output_bytes = 34;
    transaction.budget.max_output_tokens = 6;
    super::super::graph::budget_closure(&transaction, &value.operational_closure)
        .expect("exact aggregate");
    transaction.budget.max_output_bytes -= 1;
    assert!(super::super::graph::budget_closure(&transaction, &value.operational_closure).is_err());
    assert!(super::super::graph::checked_add_usage(i64::MAX, 1).is_err());
}
