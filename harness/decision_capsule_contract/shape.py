"""Exact nested shapes and references for structural capsule replay."""

from __future__ import annotations

from kernel_decision_contract.constants import (
    ATOM_PREFIX, CLOSURE_PREFIX as DECISION_CLOSURE_PREFIX,
    TRANSACTION_PREFIX,
)
from kernel_operational_contract.constants import (
    ARTIFACT_RECEIPT_PREFIX, CLOSURE_PREFIX as OPERATIONAL_CLOSURE_PREFIX,
    EVENT_PREFIX, EXECUTION_RECEIPT_PREFIX, INVOCATION_PREFIX,
)
from kernel_operational_contract.shape import (
    artifact_ref, bare_hash, exact_object,
)

from .codec import ContractError, canonical_json
from .constants import *  # noqa: F403 - frozen vocabulary is intentionally central


def attestations(value: object) -> dict[str, object]:
    member = exact_object(value, ATTESTATION_FIELDS, "attestations")  # noqa: F405
    if any(item is not False for item in member.values()):
        raise ContractError("all thirty-two replay attestations must be false")
    return member


def _reference(value: object, fields: set[str], id_field: str, hash_field: str,
               prefix: str, label: str) -> dict[str, object]:
    member = exact_object(value, fields, label)
    digest = bare_hash(member[hash_field], f"{label}.{hash_field}")
    if member[id_field] != f"{prefix}{digest}":
        raise ContractError(f"{label}.{id_field} must bind {hash_field}")
    return member


def atom_ref(value: object, label: str) -> dict[str, object]:
    return _reference(value, {"atom_id", "atom_sha256"}, "atom_id",
                      "atom_sha256", ATOM_PREFIX, label)


def decision_closure_ref(value: object) -> dict[str, object]:
    return _reference(value, DECISION_CLOSURE_REF_FIELDS, "closure_id",  # noqa: F405
                      "closure_sha256", DECISION_CLOSURE_PREFIX,
                      "decision_closure_ref")


def transaction_ref(value: object) -> dict[str, object]:
    return _reference(value, TRANSACTION_REF_FIELDS, "decision_transaction_id",  # noqa: F405
                      "decision_transaction_sha256", TRANSACTION_PREFIX,
                      "decision_transaction_ref")


def operational_closure_ref(value: object) -> dict[str, object]:
    return _reference(value, OPERATIONAL_CLOSURE_REF_FIELDS, "closure_id",  # noqa: F405
                      "closure_sha256", OPERATIONAL_CLOSURE_PREFIX,
                      "operational_closure_ref")


def manifest_ref(value: object) -> dict[str, object]:
    return _reference(value, MANIFEST_REF_FIELDS, "manifest_id", "manifest_sha256",  # noqa: F405
                      MANIFEST_PREFIX, "manifest_ref")  # noqa: F405


def capsule_ref(value: object) -> dict[str, object]:
    return _reference(value, CAPSULE_REF_FIELDS, "capsule_id", "capsule_sha256",  # noqa: F405
                      CAPSULE_PREFIX, "capsule_ref")  # noqa: F405


def _reference_array(value: object, label: str, minimum: int, maximum: int,
                     validator, *, sorted_ids: bool = False,
                     id_field: str = "") -> list[dict[str, object]]:
    if type(value) is not list or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} cardinality must be {minimum}..{maximum}")
    result = [validator(item, f"{label}[{index}]")
              for index, item in enumerate(value)]
    encoded = [canonical_json(item) for item in result]
    if len(encoded) != len(set(encoded)):
        raise ContractError(f"{label} must be unique")
    if sorted_ids:
        identities = [item[id_field].encode("utf-8") for item in result]
        if identities != sorted(identities):
            raise ContractError(f"{label} must preserve identity order")
    return result


def atom_refs(value: object, label: str, minimum: int) -> list[dict[str, object]]:
    return _reference_array(value, label, minimum, MAX_ATOMS, atom_ref,  # noqa: F405
                            sorted_ids=True, id_field="atom_id")


def artifact_refs(value: object, label: str, maximum: int) -> list[dict[str, object]]:
    if type(value) is not list or len(value) > maximum:
        raise ContractError(f"{label} cardinality must be 0..{maximum}")
    result = [artifact_ref(item, f"{label}[{index}]")
              for index, item in enumerate(value)]
    encoded = [canonical_json(item) for item in result]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise ContractError(f"{label} must be canonical-byte sorted and unique")
    return result


def artifact_receipt_refs(value: object) -> list[dict[str, object]]:
    from kernel_operational_contract.shape import artifact_receipt_ref
    return _reference_array(
        value, "artifact_receipt_refs", 0, MAX_ARTIFACT_RECEIPTS,  # noqa: F405
        artifact_receipt_ref, sorted_ids=True, id_field="artifact_receipt_id")


def invocation_refs(value: object) -> list[dict[str, object]]:
    from kernel_operational_contract.shape import invocation_ref
    return _reference_array(value, "capability_invocation_refs", 1,
                            MAX_INVOCATIONS, invocation_ref)  # noqa: F405


def event_refs(value: object) -> list[dict[str, object]]:
    from kernel_operational_contract.shape import event_ref
    return _reference_array(value, "interaction_event_refs", 0,
                            MAX_EVENTS, event_ref)  # noqa: F405


def execution_refs(value: object) -> list[dict[str, object]]:
    from kernel_operational_contract.shape import execution_receipt_ref
    return _reference_array(value, "execution_receipt_refs", 1,
                            MAX_EXECUTION_RECEIPTS, execution_receipt_ref)  # noqa: F405


def reflection_refs(value: object) -> list[dict[str, object]]:
    refs = artifact_refs(value, "reflection_report_artifact_refs",
                         MAX_REFLECTION_REPORT_REFS)  # noqa: F405
    if any(item["artifact_kind"] != "reflection_report" for item in refs):
        raise ContractError("reflection report ArtifactRefs require reflection_report kind")
    return refs
