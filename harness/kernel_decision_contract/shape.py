"""Exact nested shapes for CognitiveAtom v2 and DecisionTransaction v1."""

from __future__ import annotations

from capability_grant_contract.constants import (MAX_BYTES, MAX_COST_MICROS,
                                                  MAX_USAGE)
from kernel_operational_contract.shape import (
    artifact_receipt_ref, artifact_ref, bare_hash, bindings, capability,
    event_ref, exact_object, execution_receipt_ref, identifier, invocation_ref,
    principal, task_binding, text, unsigned,
)

from .codec import ContractError, canonical_json
from .constants import *  # noqa: F403 - frozen vocabulary is intentionally central


def attestations(value: object) -> dict[str, object]:
    member = exact_object(value, ATTESTATION_FIELDS, "attestations")  # noqa: F405
    if any(item is not False for item in member.values()):
        raise ContractError("all twenty-two decision attestations must be false")
    return member


def atom_ref(value: object, label: str) -> dict[str, object]:
    member = exact_object(value, ATOM_REF_FIELDS, label)  # noqa: F405
    digest = bare_hash(member["atom_sha256"], f"{label}.atom_sha256")
    if member["atom_id"] != f"{ATOM_PREFIX}{digest}":  # noqa: F405
        raise ContractError(f"{label}.atom_id must bind atom_sha256")
    return member


def _identity_ref(value: object, fields: set[str], id_field: str,
                  hash_field: str, prefix: str, label: str) -> dict[str, object]:
    member = exact_object(value, fields, label)
    digest = bare_hash(member[hash_field], f"{label}.{hash_field}")
    if member[id_field] != f"{prefix}{digest}":
        raise ContractError(f"{label}.{id_field} must bind {hash_field}")
    return member


def legacy_atom_ref(value: object) -> dict[str, object]:
    member = exact_object(value, LEGACY_ATOM_REF_FIELDS, "source_ref")  # noqa: F405
    if (not isinstance(member["atom_id"], str) or
            LEGACY_ATOM_ID_RE.fullmatch(member["atom_id"]) is None):  # noqa: F405
        raise ContractError("source_ref.atom_id must be an ADR-0047 atom identifier")
    bare_hash(member["canonical_sha256"], "source_ref.canonical_sha256")
    return member


def evidence_ref(value: object) -> dict[str, object]:
    member = exact_object(value, EVIDENCE_REF_FIELDS, "source_ref")  # noqa: F405
    bare_hash(member["canonical_sha256"], "source_ref.canonical_sha256")
    text(member["record_id"], "source_ref.record_id")
    return member


def work_intent_ref(value: object) -> dict[str, object]:
    return _identity_ref(value, WORK_INTENT_REF_FIELDS, "work_intent_id",  # noqa: F405
                         "work_intent_sha256", "work-intent-", "source_ref")


def approval_ref(value: object) -> dict[str, object]:
    member = exact_object(value, APPROVAL_REF_FIELDS, "authority_ref")  # noqa: F405
    digest = bare_hash(member["approval_sha256"], "authority_ref.approval_sha256")
    if member["approval_id"] != f"approval-record-{digest}":
        raise ContractError("authority_ref.approval_id must bind approval_sha256")
    text(member["authority_domain"], "authority_ref.authority_domain")
    return member


def adr_ref(value: object) -> dict[str, object]:
    member = exact_object(value, ADR_REF_FIELDS, "authority_ref")  # noqa: F405
    if not isinstance(member["adr_id"], str) or ADR_ID_RE.fullmatch(member["adr_id"]) is None:  # noqa: F405
        raise ContractError("authority_ref.adr_id must be a canonical ADR identifier")
    bare_hash(member["adr_self_sha256"], "authority_ref.adr_self_sha256")
    return member


def declared_authority(value: object) -> dict[str, object]:
    member = exact_object(value, AUTHORITY_FIELDS, "declared_authority")  # noqa: F405
    kind = member["authority_kind"]
    if not isinstance(kind, str) or kind not in AUTHORITY_KINDS:  # noqa: F405
        raise ContractError("declared_authority.authority_kind is unsupported")
    reference = member["authority_ref"]
    if kind == "none":
        if reference is not None:
            raise ContractError("none authority requires a null authority_ref")
    elif kind == "approval_record":
        approval_ref(reference)
    elif kind == "architecture_decision":
        adr_ref(reference)
    else:
        artifact_ref(reference, "authority_ref")
    return member


def proposition(value: object) -> dict[str, object]:
    member = exact_object(value, PROPOSITION_FIELDS, "proposition")  # noqa: F405
    object_type, object_value = member["object_type"], member["object_value"]
    if not isinstance(object_type, str) or object_type not in OBJECT_TYPES:  # noqa: F405
        raise ContractError("proposition.object_type is unsupported")
    matches = {
        "artifact_ref": isinstance(object_value, str),
        "boolean": isinstance(object_value, bool),
        "integer": isinstance(object_value, int) and not isinstance(object_value, bool),
        "null": object_value is None,
        "string": isinstance(object_value, str),
    }
    if not matches[object_type]:
        raise ContractError("proposition.object_value does not match object_type")
    if object_type == "artifact_ref":
        identifier(object_value, "proposition.object_value")
    elif isinstance(object_value, str):
        text(object_value, "proposition.object_value", 16_384)
    identifier(member["predicate"], "proposition.predicate")
    identifier(member["subject"], "proposition.subject")
    return member


def atom_scope(value: object, task: dict, proposition_value: dict) -> dict[str, object]:
    member = exact_object(value, SCOPE_FIELDS, "scope")  # noqa: F405
    project = text(member["project"], "scope.project")
    if project != task["project_id"]:
        raise ContractError("scope.project must equal task_binding.project_id")
    for field in ("module", "object"):
        if member[field] is not None:
            text(member[field], f"scope.{field}")
    if member["object"] is not None and member["object"] != proposition_value["subject"]:
        raise ContractError("non-null scope.object must equal proposition.subject")
    return member


def validity(value: object) -> dict[str, object]:
    member = exact_object(value, VALIDITY_FIELDS, "validity")  # noqa: F405
    start = unsigned(member["valid_from_unix_ms"], "validity.valid_from_unix_ms")
    end = member["valid_until_unix_ms"]
    if end is not None and unsigned(end, "validity.valid_until_unix_ms") <= start:
        raise ContractError("validity.valid_until_unix_ms must be greater than valid_from")
    return member


def source(value: object, atom_type: str) -> dict[str, object]:
    member = exact_object(value, SOURCE_FIELDS, "source")  # noqa: F405
    kind = member["source_kind"]
    if (not isinstance(kind, str) or kind not in SOURCE_TYPES or  # noqa: F405
            atom_type not in SOURCE_TYPES[kind]):
        raise ContractError("source_kind does not admit atom_type")
    expected_phase = "predecision" if kind in PREDECISION_SOURCES else "postdecision"  # noqa: F405
    if member["source_phase"] != expected_phase:
        raise ContractError("source_phase does not match source_kind")
    _source_ref(kind, member["source_ref"])
    selector = member["source_selector"]
    if selector is not None:
        text(selector, "source.source_selector", MAX_SELECTOR_BYTES)  # noqa: F405
        if JSON_POINTER_RE.fullmatch(selector) is None:  # noqa: F405
            raise ContractError("source_selector must be a canonical non-empty JSON pointer")
    if kind == "cognitive_atom_v1" and selector is not None:
        raise ContractError("cognitive_atom_v1 source_selector must be null")
    return member


def _source_ref(kind: str, value: object) -> None:
    if kind == "artifact":
        artifact_ref(value, "source_ref")
    elif kind == "artifact_receipt":
        artifact_receipt_ref(value, "source_ref")
    elif kind == "capability_invocation":
        invocation_ref(value, "source_ref")
    elif kind == "cognitive_atom_v1":
        legacy_atom_ref(value)
    elif kind == "evidence_record":
        evidence_ref(value)
    elif kind == "execution_receipt":
        execution_receipt_ref(value, "source_ref")
    elif kind == "interaction_event":
        event_ref(value, "source_ref")
    else:
        work_intent_ref(value)


def sorted_atom_refs(value: object, label: str, maximum: int = MAX_ATOM_REFS,  # noqa: F405
                     nonempty: bool = False) -> list[dict[str, object]]:
    if not isinstance(value, list) or len(value) > maximum or (nonempty and not value):
        raise ContractError(f"{label} cardinality is outside the frozen bound")
    result = [atom_ref(item, f"{label}[{index}]") for index, item in enumerate(value)]
    identities = [item["atom_id"].encode("utf-8") for item in result]
    if identities != sorted(identities) or len(identities) != len(set(identities)):
        raise ContractError(f"{label} must be strictly atom-id sorted and unique")
    return result


def budget(value: object) -> dict[str, object]:
    member = exact_object(value, BUDGET_FIELDS, "budget")  # noqa: F405
    unsigned(member["max_calls"], "budget.max_calls", MAX_USAGE)
    if member["max_calls"] == 0:
        raise ContractError("budget.max_calls must be positive")
    unsigned(member["max_cost_usd_micros"], "budget.max_cost_usd_micros", MAX_COST_MICROS)
    for field in ("max_input_tokens", "max_output_tokens"):
        unsigned(member[field], f"budget.{field}", MAX_USAGE)
    for field in ("max_network_bytes", "max_output_bytes"):
        unsigned(member[field], f"budget.{field}", MAX_BYTES)
    timeout = unsigned(member["timeout_ms"], "budget.timeout_ms", MAX_TIMEOUT_MS)  # noqa: F405
    if timeout == 0:
        raise ContractError("budget.timeout_ms must be positive")
    return member


def sorted_canonical(value: object, label: str, minimum: int, maximum: int,
                     validator) -> list:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} cardinality must be {minimum}..{maximum}")
    result = [validator(item, f"{label}[{index}]") for index, item in enumerate(value)]
    encoded = [canonical_json(item) for item in result]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise ContractError(f"{label} must be strictly canonical-byte sorted and unique")
    return result
