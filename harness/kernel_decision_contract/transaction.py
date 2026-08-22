"""DecisionTransaction v1 strict validation, sealing, and decoding."""

from __future__ import annotations

import copy
import hashlib

from kernel_operational_contract.shape import (
    artifact_receipt_ref, bare_hash, bindings, capability, exact_object, identifier,
    principal, string_set, task_binding, text, unsigned,
)

from .codec import ContractError, canonical_json, decode_canonical_json
from .constants import *  # noqa: F403 - frozen contract vocabulary
from .shape import atom_ref, attestations, budget, sorted_atom_refs


def _identity(transaction: dict[str, object], allow_blank: bool) -> None:
    if (allow_blank and transaction["decision_transaction_id"] ==
            transaction["decision_transaction_sha256"] == ""):
        return
    digest = bare_hash(transaction["decision_transaction_sha256"],
                       "decision_transaction_sha256")
    if transaction["decision_transaction_id"] != f"{TRANSACTION_PREFIX}{digest}":  # noqa: F405
        raise ContractError("decision_transaction_id must bind its digest")


def _sorted_by_id(value: object, label: str, minimum: int, maximum: int,
                  id_field: str, validator) -> list[dict[str, object]]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} cardinality must be {minimum}..{maximum}")
    result = [validator(item, f"{label}[{index}]") for index, item in enumerate(value)]
    identities = [item[id_field].encode("utf-8") for item in result]
    if identities != sorted(identities) or len(identities) != len(set(identities)):
        raise ContractError(f"{label} must be strictly {id_field}-sorted and unique")
    return result


def _option(value: object, label: str) -> dict[str, object]:
    member = exact_object(value, OPTION_FIELDS, label)  # noqa: F405
    capability(member["capability"])
    identifier(member["option_id"], f"{label}.option_id")
    bare_hash(member["requested_action_sha256"], f"{label}.requested_action_sha256")
    return member


def _condition(value: object) -> dict[str, object]:
    member = exact_object(value, CONDITION_FIELDS, "completion_condition")  # noqa: F405
    text(member["condition_ref"], "completion_condition.condition_ref", 4_096)
    bare_hash(member["condition_sha256"], "completion_condition.condition_sha256")
    return member


def _compensation(value: object) -> dict[str, object]:
    member = exact_object(value, COMPENSATION_FIELDS, "compensation")  # noqa: F405
    applicability = member["applicability"]
    if not isinstance(applicability, str) or applicability not in {"not_applicable", "required"}:
        raise ContractError("compensation.applicability is unsupported")
    capability_present = member["capability"] is not None
    action_present = member["requested_action_sha256"] is not None
    if capability_present != action_present:
        raise ContractError("compensation capability and action must be jointly null or present")
    if (applicability == "required") != capability_present:
        raise ContractError("required compensation needs capability and action; N/A needs nulls")
    if capability_present:
        capability(member["capability"])
        bare_hash(member["requested_action_sha256"], "compensation.requested_action_sha256")
    return member


def _proof(value: object, label: str) -> dict[str, object]:
    member = exact_object(value, PROOF_FIELDS, label)  # noqa: F405
    identifier(member["obligation_id"], f"{label}.obligation_id")
    bare_hash(member["predicate_sha256"], f"{label}.predicate_sha256")
    string_set(member["required_evidence_kinds"], f"{label}.required_evidence_kinds",
               MAX_EVIDENCE_KINDS, True)  # noqa: F405
    return member


def _verifier(value: object, actor: dict, owner: dict) -> dict[str, object]:
    member = exact_object(value, VERIFIER_FIELDS, "verifier")  # noqa: F405
    capability(member["capability"])
    bare_hash(member["independence_basis_sha256"], "verifier.independence_basis_sha256")
    verifier_principal = principal(member["principal"], "verifier.principal")
    timeout = unsigned(member["timeout_ms"], "verifier.timeout_ms", MAX_TIMEOUT_MS)  # noqa: F405
    if timeout == 0:
        raise ContractError("verifier.timeout_ms must be positive")
    if verifier_principal == actor or verifier_principal == owner:
        raise ContractError("declared verifier principal must differ from actor and owner")
    return member


def _precondition(value: object, label: str) -> dict[str, object]:
    member = exact_object(value, PRECONDITION_FIELDS, label)  # noqa: F405
    bare_hash(member["expected_sha256"], f"{label}.expected_sha256")
    identifier(member["precondition_id"], f"{label}.precondition_id")
    text(member["resource_ref"], f"{label}.resource_ref", 4_096)
    return member


def _read_refs(value: object) -> list[dict[str, object]]:
    if not isinstance(value, list) or len(value) > MAX_IO_ITEMS:  # noqa: F405
        raise ContractError("read_artifact_receipt_refs exceeds the frozen bound")
    result = [artifact_receipt_ref(item, f"read_artifact_receipt_refs[{index}]")
              for index, item in enumerate(value)]
    encoded = [canonical_json(item) for item in result]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise ContractError("read_artifact_receipt_refs must be sorted and unique")
    return result


def _atom_roles(transaction: dict[str, object]) -> None:
    goal = atom_ref(transaction["goal_atom_ref"], "goal_atom_ref")
    triggers = sorted_atom_refs(transaction["trigger_atom_refs"], "trigger_atom_refs")
    guards = sorted_atom_refs(transaction["guard_atom_refs"], "guard_atom_refs")
    role_ids = [goal["atom_id"]] + [item["atom_id"] for item in triggers + guards]
    if len(role_ids) != len(set(role_ids)):
        raise ContractError("goal, trigger and guard atom roles must be disjoint")


def validate_decision_transaction_shape(value: object, *,
                                        allow_blank: bool = False) -> dict[str, object]:
    """Validate immutable structural proposal fields without granting authority."""
    transaction = exact_object(value, TRANSACTION_FIELDS, "DecisionTransaction")  # noqa: F405
    constants = {"api_version": TRANSACTION_API, "canonicalization": CANONICALIZATION,  # noqa: F405
                 "kind": TRANSACTION_KIND, "transaction_mode": TRANSACTION_MODE}  # noqa: F405
    for field, expected in constants.items():
        if transaction[field] != expected:
            raise ContractError(f"{field} must be {expected!r}")
    _identity(transaction, allow_blank)
    attestations(transaction["attestations"])
    bindings(transaction["bindings"])
    task_binding(transaction["task_binding"])
    actor = principal(transaction["actor"], "actor")
    owner = principal(transaction["accountable_owner"], "accountable_owner")
    budget(transaction["budget"])
    _condition(transaction["completion_condition"])
    _compensation(transaction["compensation"])
    unsigned(transaction["created_at_unix_ms"], "created_at_unix_ms")
    _atom_roles(transaction)
    identifier(transaction["idempotency_key"], "idempotency_key")
    options = _sorted_by_id(transaction["options"], "options", 1, MAX_OPTIONS,  # noqa: F405
                            "option_id", _option)
    selected = transaction["selected_option_id"]
    if (not isinstance(selected, str) or
            selected not in {item["option_id"] for item in options}):
        raise ContractError("selected_option_id must identify one option")
    bare_hash(transaction["selection_basis_sha256"], "selection_basis_sha256")
    _sorted_by_id(transaction["proof_obligations"], "proof_obligations", 1,
                  MAX_PROOFS, "obligation_id", _proof)  # noqa: F405
    _read_refs(transaction["read_artifact_receipt_refs"])
    _verifier(transaction["verifier"], actor, owner)
    _sorted_by_id(transaction["write_preconditions"], "write_preconditions", 0,
                  MAX_IO_ITEMS, "precondition_id", _precondition)  # noqa: F405
    string_set(transaction["write_slots"], "write_slots", MAX_IO_ITEMS, False)  # noqa: F405
    return transaction


def _blanked(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != TRANSACTION_FIELDS:  # noqa: F405
        raise ContractError("DecisionTransaction digest input must have exact fields")
    result = copy.deepcopy(value)
    result["decision_transaction_id"] = ""
    result["decision_transaction_sha256"] = ""
    return result


def decision_transaction_digest(value: object) -> str:
    blank = _blanked(value)
    validate_decision_transaction_shape(blank, allow_blank=True)
    raw = canonical_json(blank)
    if len(raw) > MAX_TRANSACTION_BYTES:  # noqa: F405
        raise ContractError("DecisionTransaction blank preimage exceeds its byte ceiling")
    return hashlib.sha256(TRANSACTION_DOMAIN + raw).hexdigest()  # noqa: F405


def validate_decision_transaction(value: object) -> dict[str, object]:
    transaction = validate_decision_transaction_shape(value)
    if len(canonical_json(transaction)) > MAX_TRANSACTION_BYTES:  # noqa: F405
        raise ContractError("DecisionTransaction exceeds its byte ceiling")
    if transaction["decision_transaction_sha256"] != decision_transaction_digest(transaction):
        raise ContractError("decision_transaction_sha256 does not match canonical preimage")
    return transaction


def seal_decision_transaction(value: object) -> dict[str, object]:
    transaction = copy.deepcopy(value)
    if (not isinstance(transaction, dict) or
            transaction.get("decision_transaction_id") != "" or
            transaction.get("decision_transaction_sha256") != ""):
        raise ContractError("sealing DecisionTransaction requires blank identity fields")
    digest = decision_transaction_digest(transaction)
    transaction["decision_transaction_id"] = f"{TRANSACTION_PREFIX}{digest}"  # noqa: F405
    transaction["decision_transaction_sha256"] = digest
    return validate_decision_transaction(transaction)


def decode_decision_transaction(raw: bytes) -> dict[str, object]:
    return validate_decision_transaction(decode_canonical_json(raw, MAX_TRANSACTION_BYTES))  # noqa: F405
