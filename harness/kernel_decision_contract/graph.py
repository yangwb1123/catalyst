"""Cross-closure DAG and projection relations for the decision reference core."""

from __future__ import annotations

from kernel_operational_contract.constants import MAX_I64

from .codec import ContractError, canonical_json
from .constants import PREDECISION_SOURCES


def _ref(record: dict, id_field: str, hash_field: str) -> dict[str, object]:
    return {id_field: record[id_field], hash_field: record[hash_field]}


def _atom_index(atoms: list[dict]) -> dict[str, dict]:
    return {atom["atom_id"]: atom for atom in atoms}


def _transaction_atom_ids(transaction: dict) -> set[str]:
    values = [transaction["goal_atom_ref"]]
    values.extend(transaction["trigger_atom_refs"])
    values.extend(transaction["guard_atom_refs"])
    return {value["atom_id"] for value in values}


def _validate_atom_roles(atoms: list[dict], transaction: dict) -> None:
    by_id = _atom_index(atoms)
    referenced = _transaction_atom_ids(transaction)
    role_refs = [(transaction["goal_atom_ref"], "goal")]
    role_refs.extend((reference, "trigger") for reference in transaction["trigger_atom_refs"])
    role_refs.extend((reference, "guard") for reference in transaction["guard_atom_refs"])
    for reference, role in role_refs:
        atom_id = reference["atom_id"]
        atom = by_id.get(atom_id)
        if atom is None:
            raise ContractError(f"transaction references missing CognitiveAtom {atom_id!r}")
        if reference["atom_sha256"] != atom["atom_sha256"]:
            raise ContractError(f"transaction CognitiveAtom digest drift for {atom_id!r}")
        if atom["source"]["source_kind"] not in PREDECISION_SOURCES:
            raise ContractError("DecisionTransaction may reference only predecision atoms")
        if (role == "goal") != (atom["atom_type"] == "goal"):
            raise ContractError(
                "goal_atom_ref must resolve the only predecision goal CognitiveAtom")
    predecision = {atom["atom_id"] for atom in atoms
                   if atom["source"]["source_phase"] == "predecision"}
    if predecision != referenced:
        raise ContractError("predecision CognitiveAtoms must be the exact transaction role union")


def _operational_indexes(operational: dict) -> dict[str, dict[str, dict]]:
    specs = {
        "artifact_receipt": ("artifact_receipts", "artifact_receipt_id"),
        "capability_invocation": ("capability_invocations", "invocation_id"),
        "interaction_event": ("interaction_events", "event_id"),
        "execution_receipt": ("execution_receipts", "execution_receipt_id"),
    }
    return {kind: {record[id_field]: record for record in operational[array]}
            for kind, (array, id_field) in specs.items()}


def _operational_ref(record: dict, kind: str) -> dict[str, object]:
    fields = {
        "artifact_receipt": ("artifact_receipt_id", "artifact_receipt_sha256"),
        "capability_invocation": ("invocation_id", "invocation_sha256"),
        "interaction_event": ("event_id", "event_sha256"),
        "execution_receipt": ("execution_receipt_id", "execution_receipt_sha256"),
    }[kind]
    return _ref(record, *fields)


def _source_time(record: dict, kind: str) -> int:
    field = {"artifact_receipt": "created_at_unix_ms",
             "capability_invocation": "requested_at_unix_ms",
             "interaction_event": "occurred_at_unix_ms",
             "execution_receipt": "ended_at_unix_ms"}[kind]
    return record[field]


def _validate_postdecision_sources(atoms: list[dict], transaction: dict,
                                   operational: dict) -> None:
    indexes = _operational_indexes(operational)
    for atom in atoms:
        source = atom["source"]
        if source["source_phase"] != "postdecision":
            continue
        kind, reference = source["source_kind"], source["source_ref"]
        id_field = next(field for field in reference if field.endswith("_id"))
        record = indexes[kind].get(reference[id_field])
        if record is None or _operational_ref(record, kind) != reference:
            raise ContractError("postdecision source_ref must resolve exact operational bytes")
        if kind == "artifact_receipt" and record["receipt_role"] != "declared_output":
            raise ContractError("postdecision ArtifactReceipt sources must be declared outputs")
        valid_from = atom["validity"]["valid_from_unix_ms"]
        if valid_from < transaction["created_at_unix_ms"]:
            raise ContractError("postdecision Atom validity predates its transaction")
        if valid_from < _source_time(record, kind):
            raise ContractError("postdecision Atom validity predates its operational source")


def _same_context(atoms: list[dict], transaction: dict, operational: dict) -> None:
    task, opaque = transaction["task_binding"], transaction["bindings"]
    for atom in atoms:
        if atom["task_binding"] != task or atom["bindings"] != opaque:
            raise ContractError("every CognitiveAtom must share transaction task and bindings")
    arrays = ("artifact_receipts", "capability_invocations", "interaction_events",
              "execution_receipts")
    for array in arrays:
        for record in operational[array]:
            if record["task_binding"] != task or record["bindings"] != opaque:
                raise ContractError("every operational record must share transaction context")


def _validate_selected_operation(transaction: dict, operational: dict) -> None:
    selected = next(item for item in transaction["options"]
                    if item["option_id"] == transaction["selected_option_id"])
    reads = transaction["read_artifact_receipt_refs"]
    writes = transaction["write_slots"]
    for invocation in operational["capability_invocations"]:
        if invocation["correlation_id"] != transaction["decision_transaction_id"]:
            raise ContractError("Invocation correlation_id must equal DecisionTransaction ID")
        if invocation["subject"] != transaction["actor"]:
            raise ContractError("Invocation subject must equal DecisionTransaction actor")
        relations = (invocation["capability"] == selected["capability"],
                     invocation["requested_action_sha256"] == selected["requested_action_sha256"],
                     invocation["idempotency_key"] == transaction["idempotency_key"],
                     invocation["input_artifact_receipt_refs"] == reads,
                     invocation["declared_output_slots"] == writes)
        if not all(relations):
            raise ContractError("Invocation differs from selected transaction declarations")
    for event in operational["interaction_events"]:
        if event["correlation_id"] != transaction["decision_transaction_id"]:
            raise ContractError("Event correlation_id must equal DecisionTransaction ID")
    for receipt in operational["execution_receipts"]:
        if receipt["correlation_id"] != transaction["decision_transaction_id"]:
            raise ContractError("ExecutionReceipt correlation_id must equal transaction ID")


def _validate_times(atoms: list[dict], transaction: dict, operational: dict) -> None:
    created = transaction["created_at_unix_ms"]
    first_request = min(item["requested_at_unix_ms"]
                        for item in operational["capability_invocations"])
    if created > first_request:
        raise ContractError("DecisionTransaction creation must not follow its first request")
    receipts = {item["artifact_receipt_id"]: item
                for item in operational["artifact_receipts"]}
    for reference in transaction["read_artifact_receipt_refs"]:
        receipt = receipts.get(reference["artifact_receipt_id"])
        if (receipt is None or receipt["receipt_role"] != "declared_input" or
                receipt["created_at_unix_ms"] > created):
            raise ContractError("transaction reads require nonfuture declared-input receipts")
    for atom in atoms:
        if atom["source"]["source_phase"] != "predecision":
            continue
        validity = atom["validity"]
        if validity["valid_from_unix_ms"] > created:
            raise ContractError("predecision Atom begins after transaction creation")
        if validity["valid_until_unix_ms"] is not None and created >= validity["valid_until_unix_ms"]:
            raise ContractError("predecision Atom expired before transaction creation")


def _validate_budget(transaction: dict, operational: dict) -> None:
    budget = transaction["budget"]
    invocations = operational["capability_invocations"]
    if len(invocations) > budget["max_calls"]:
        raise ContractError("Invocation count exceeds transaction budget")
    totals = {field: 0 for field in ("call_count", "cost_usd_micros", "elapsed_ms",
                                     "input_tokens", "network_bytes", "output_bytes",
                                     "output_tokens")}
    for receipt in operational["execution_receipts"]:
        for field in totals:
            increment = receipt["observed_usage"][field]
            if totals[field] > MAX_I64 - increment:
                raise ContractError("caller-declared aggregate usage exceeds signed int64")
            totals[field] += increment
    limits = {"call_count": "max_calls", "cost_usd_micros": "max_cost_usd_micros",
              "elapsed_ms": "timeout_ms", "input_tokens": "max_input_tokens",
              "network_bytes": "max_network_bytes", "output_bytes": "max_output_bytes",
              "output_tokens": "max_output_tokens"}
    if any(totals[field] > budget[limit] for field, limit in limits.items()):
        raise ContractError("caller-declared aggregate usage exceeds transaction budget")


def validate_reference_graph(atoms: list[dict], transaction: dict,
                             operational: dict) -> None:
    """Validate the one-way CognitiveAtom→Transaction→operation reference graph."""
    _validate_atom_roles(atoms, transaction)
    _validate_postdecision_sources(atoms, transaction, operational)
    _same_context(atoms, transaction, operational)
    _validate_selected_operation(transaction, operational)
    _validate_times(atoms, transaction, operational)
    _validate_budget(transaction, operational)
