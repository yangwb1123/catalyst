use std::collections::{BTreeMap, BTreeSet};

use crate::kernel_operational_contract::{KernelOperationalReferenceClosure, ObservedUsage};

use super::{
    CognitiveAtom, DecisionOption, DecisionTransaction, KernelDecisionContractError, invalid,
    source::raw_reference,
};

pub(super) fn role_closure(
    atoms: &[CognitiveAtom],
    transaction: &DecisionTransaction,
) -> Result<(), KernelDecisionContractError> {
    let index = atoms
        .iter()
        .map(|atom| (atom.atom_id.as_str(), atom))
        .collect::<BTreeMap<_, _>>();
    let role_refs = std::iter::once(&transaction.goal_atom_ref)
        .chain(&transaction.trigger_atom_refs)
        .chain(&transaction.guard_atom_refs)
        .collect::<Vec<_>>();
    let mut role_ids = BTreeSet::new();
    for (role_index, reference) in role_refs.into_iter().enumerate() {
        let atom = index
            .get(reference.atom_id.as_str())
            .ok_or_else(|| invalid("transaction references a missing CognitiveAtom"))?;
        if atom.atom_sha256 != reference.atom_sha256 {
            return Err(invalid("transaction CognitiveAtom digest drift"));
        }
        if !matches!(
            atom.source.source_kind.as_str(),
            "artifact" | "cognitive_atom_v1" | "evidence_record" | "work_intent"
        ) {
            return Err(invalid(
                "DecisionTransaction may reference only predecision atoms",
            ));
        }
        if (role_index == 0) != (atom.atom_type == "goal") {
            return Err(invalid(
                "goal_atom_ref must resolve the only predecision goal CognitiveAtom",
            ));
        }
        role_ids.insert(reference.atom_id.as_str());
    }
    let predecision = atoms
        .iter()
        .filter(|atom| atom.source.source_phase == "predecision")
        .map(|atom| atom.atom_id.as_str())
        .collect::<BTreeSet<_>>();
    if predecision != role_ids {
        return Err(invalid(
            "predecision atoms must equal exact transaction role union",
        ));
    }
    Ok(())
}

fn operational_source(
    value: &KernelOperationalReferenceClosure,
    kind: &str,
    id: &str,
    digest: &str,
) -> Option<(i64, bool)> {
    match kind {
        "artifact_receipt" => value
            .artifact_receipts
            .iter()
            .find(|record| {
                record.artifact_receipt_id == id && record.artifact_receipt_sha256 == digest
            })
            .map(|record| {
                (
                    record.created_at_unix_ms,
                    record.receipt_role == "declared_output",
                )
            }),
        "capability_invocation" => value
            .capability_invocations
            .iter()
            .find(|record| record.invocation_id == id && record.invocation_sha256 == digest)
            .map(|record| (record.requested_at_unix_ms, false)),
        "interaction_event" => value
            .interaction_events
            .iter()
            .find(|record| record.event_id == id && record.event_sha256 == digest)
            .map(|record| (record.occurred_at_unix_ms, false)),
        "execution_receipt" => value
            .execution_receipts
            .iter()
            .find(|record| {
                record.execution_receipt_id == id && record.execution_receipt_sha256 == digest
            })
            .map(|record| (record.ended_at_unix_ms, false)),
        _ => None,
    }
}

fn post_sources(
    atoms: &[CognitiveAtom],
    transaction: &DecisionTransaction,
    operational: &KernelOperationalReferenceClosure,
) -> Result<(), KernelDecisionContractError> {
    for atom in atoms {
        if atom.source.source_phase != "postdecision" {
            continue;
        }
        let (id, digest) = raw_reference(&atom.source.source_ref)?;
        let (source_time, declared_output) =
            operational_source(operational, &atom.source.source_kind, id, digest)
                .ok_or_else(|| invalid("postdecision source_ref does not resolve exact record"))?;
        if atom.source.source_kind == "artifact_receipt" && !declared_output {
            return Err(invalid(
                "postdecision ArtifactReceipt source must be declared_output",
            ));
        }
        if atom.validity.valid_from_unix_ms < transaction.created_at_unix_ms
            || atom.validity.valid_from_unix_ms < source_time
        {
            return Err(invalid(
                "postdecision Atom validity predates transaction or source",
            ));
        }
    }
    Ok(())
}

fn context(
    atoms: &[CognitiveAtom],
    transaction: &DecisionTransaction,
    operational: &KernelOperationalReferenceClosure,
) -> Result<(), KernelDecisionContractError> {
    if atoms.iter().any(|atom| {
        atom.task_binding != transaction.task_binding || atom.bindings != transaction.bindings
    }) {
        return Err(invalid(
            "every CognitiveAtom must share transaction task and bindings",
        ));
    }
    let receipt_drift = operational.artifact_receipts.iter().any(|record| {
        record.task_binding != transaction.task_binding || record.bindings != transaction.bindings
    });
    let invocation_drift = operational.capability_invocations.iter().any(|record| {
        record.task_binding != transaction.task_binding || record.bindings != transaction.bindings
    });
    let event_drift = operational.interaction_events.iter().any(|record| {
        record.task_binding != transaction.task_binding || record.bindings != transaction.bindings
    });
    let execution_drift = operational.execution_receipts.iter().any(|record| {
        record.task_binding != transaction.task_binding || record.bindings != transaction.bindings
    });
    if receipt_drift || invocation_drift || event_drift || execution_drift {
        return Err(invalid("operational record task or bindings drift"));
    }
    Ok(())
}

fn selected(value: &DecisionTransaction) -> &DecisionOption {
    value
        .options
        .iter()
        .find(|option| option.option_id == value.selected_option_id)
        .expect("validated selected option")
}

fn selected_operation(
    transaction: &DecisionTransaction,
    operational: &KernelOperationalReferenceClosure,
) -> Result<(), KernelDecisionContractError> {
    let option = selected(transaction);
    for invocation in &operational.capability_invocations {
        if invocation.correlation_id != transaction.decision_transaction_id
            || invocation.subject != transaction.actor
            || invocation.capability != option.capability
            || invocation.requested_action_sha256 != option.requested_action_sha256
            || invocation.idempotency_key != transaction.idempotency_key
            || invocation.input_artifact_receipt_refs != transaction.read_artifact_receipt_refs
            || invocation.declared_output_slots != transaction.write_slots
        {
            return Err(invalid(
                "Invocation differs from selected transaction declarations",
            ));
        }
    }
    if operational
        .interaction_events
        .iter()
        .any(|event| event.correlation_id != transaction.decision_transaction_id)
        || operational
            .execution_receipts
            .iter()
            .any(|receipt| receipt.correlation_id != transaction.decision_transaction_id)
    {
        return Err(invalid("operational correlation differs from transaction"));
    }
    Ok(())
}

pub(super) fn times(
    atoms: &[CognitiveAtom],
    transaction: &DecisionTransaction,
    operational: &KernelOperationalReferenceClosure,
) -> Result<(), KernelDecisionContractError> {
    let first = operational
        .capability_invocations
        .iter()
        .map(|value| value.requested_at_unix_ms)
        .min()
        .expect("operational closure has invocation");
    if transaction.created_at_unix_ms > first {
        return Err(invalid("transaction creation follows first request"));
    }
    read_times(transaction, operational)?;
    if atoms.iter().any(|atom| {
        atom.source.source_phase == "predecision"
            && (atom.validity.valid_from_unix_ms > transaction.created_at_unix_ms
                || atom
                    .validity
                    .valid_until_unix_ms
                    .is_some_and(|end| transaction.created_at_unix_ms >= end))
    }) {
        return Err(invalid(
            "predecision Atom is future or expired at transaction creation",
        ));
    }
    Ok(())
}

fn read_times(
    transaction: &DecisionTransaction,
    operational: &KernelOperationalReferenceClosure,
) -> Result<(), KernelDecisionContractError> {
    for reference in &transaction.read_artifact_receipt_refs {
        let found = operational.artifact_receipts.iter().any(|receipt| {
            receipt.artifact_receipt_id == reference.artifact_receipt_id
                && receipt.artifact_receipt_sha256 == reference.artifact_receipt_sha256
                && receipt.receipt_role == "declared_input"
                && receipt.created_at_unix_ms <= transaction.created_at_unix_ms
        });
        if !found {
            return Err(invalid(
                "transaction read must resolve nonfuture declared-input receipt",
            ));
        }
    }
    Ok(())
}

fn usage_values(value: &ObservedUsage) -> [i64; 7] {
    [
        value.call_count,
        value.cost_usd_micros,
        value.elapsed_ms,
        value.input_tokens,
        value.network_bytes,
        value.output_bytes,
        value.output_tokens,
    ]
}

pub(super) fn checked_add_usage(
    total: i64,
    increment: i64,
) -> Result<i64, KernelDecisionContractError> {
    total
        .checked_add(increment)
        .ok_or_else(|| invalid("caller-declared aggregate usage overflows signed int64"))
}

pub(super) fn budget_closure(
    transaction: &DecisionTransaction,
    operational: &KernelOperationalReferenceClosure,
) -> Result<(), KernelDecisionContractError> {
    if i64::try_from(operational.capability_invocations.len()).unwrap_or(i64::MAX)
        > transaction.budget.max_calls
    {
        return Err(invalid("Invocation count exceeds transaction budget"));
    }
    let mut totals = [0_i64; 7];
    for receipt in &operational.execution_receipts {
        for (index, increment) in usage_values(&receipt.observed_usage)
            .into_iter()
            .enumerate()
        {
            totals[index] = checked_add_usage(totals[index], increment)?;
        }
    }
    let limits = [
        transaction.budget.max_calls,
        transaction.budget.max_cost_usd_micros,
        transaction.budget.timeout_ms,
        transaction.budget.max_input_tokens,
        transaction.budget.max_network_bytes,
        transaction.budget.max_output_bytes,
        transaction.budget.max_output_tokens,
    ];
    if totals
        .iter()
        .zip(limits)
        .any(|(total, limit)| *total > limit)
    {
        return Err(invalid(
            "caller-declared aggregate usage exceeds transaction budget",
        ));
    }
    Ok(())
}

pub(super) fn validate_reference_graph(
    atoms: &[CognitiveAtom],
    transaction: &DecisionTransaction,
    operational: &KernelOperationalReferenceClosure,
) -> Result<(), KernelDecisionContractError> {
    role_closure(atoms, transaction)?;
    post_sources(atoms, transaction, operational)?;
    context(atoms, transaction, operational)?;
    selected_operation(transaction, operational)?;
    times(atoms, transaction, operational)?;
    budget_closure(transaction, operational)
}
