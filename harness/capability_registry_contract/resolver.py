"""Pure exact declared-resolution semantics; no ambient reads or execution."""

from __future__ import annotations

from .codec import ContractError, canonical_json, decode_canonical
from .constants import (
    ASSESSMENT_API, ASSESSMENT_FIELDS, CANONICALIZATION, MAX_ASSESSMENT_BYTES,
    RELATION_FIELDS, RESULT,
)
from .digests import require_digest, seal
from .shapes import exact_object, hash_value, identifier, sorted_unique
from .validation import validate_registry, validate_request


def _matching_entry(registry: dict[str, object], request: dict[str, object]):
    reference = request["expected_reference"]
    id_matches = [entry for entry in registry["entries"]
                  if entry["contract"]["capability_id"] == reference["capability_id"]]
    if not id_matches:
        return None, "capability_id_not_found"
    version_matches = [entry for entry in id_matches if
                       entry["contract"]["capability_version"] ==
                       reference["capability_version"]]
    if not version_matches:
        return None, "capability_version_not_found"
    entry = version_matches[0]
    if (entry["contract"]["capability_contract_sha256"] !=
            reference["capability_contract_sha256"]):
        return entry, "capability_contract_digest_mismatch"
    expected = request["expected_contract"]
    if expected is not None and canonical_json(expected) != canonical_json(entry["contract"]):
        raise ContractError("equal contract digest has unequal canonical contract bytes")
    return entry, "resolved_exact"


def _relations(entry: dict[str, object] | None, request: dict[str, object],
               resolution: str) -> dict[str, str]:
    expected = request["expected_contract"]
    reference = request["expected_reference"]
    contract = entry["contract"] if entry is not None else None
    relations = {field: "not_evaluated" for field in RELATION_FIELDS}
    relations["identity"] = ("same_declared_identity" if resolution == "resolved_exact"
                             else resolution)
    if resolution != "resolved_exact" or expected is None or contract is None:
        return relations
    for field in RELATION_FIELDS:
        if field == "identity":
            continue
        relations[field] = (f"same_declared_{field}" if expected[field] == contract[field]
                            else f"{field}_mismatch")
    return relations


def _base_assessment(registry: dict[str, object], request: dict[str, object],
                     entry: dict[str, object] | None, resolution: str):
    return {
        "api_version": ASSESSMENT_API,
        "assessment_mode": "authority_neutral_read_only_exact_declared_contract",
        "assessment_sha256": "",
        "authorization_decision": "none",
        "canonicalization": CANONICALIZATION,
        "effect_attestation": False,
        "gate_applicability_state": "not_evaluated",
        "implementation_availability_attestation": False,
        "invocation_attestation": False,
        "kind": "CapabilityRegistryDeclaredResolution",
        "matched_key_entry_id": entry["entry_id"] if entry is not None else None,
        "matched_key_entry_sha256": entry["entry_sha256"] if entry is not None else None,
        "owner_authentication_state": "not_evaluated",
        "permission_attestation": False,
        "persistence_attestation": False,
        "proof_satisfaction_state": "not_evaluated",
        "reason_codes": [resolution] if resolution != "resolved_exact" else [],
        "registry_authentication_state": "not_evaluated",
        "registry_sha256": registry["registry_sha256"],
        "relations": _relations(entry, request, resolution),
        "request_sha256": request["request_sha256"],
        "resolution": resolution,
        "result": RESULT,
        "rule_applicability_state": "not_evaluated",
        "runtime_routing_attestation": False,
        "test_pass_attestation": False,
        "transition_attestation": False,
    }


def resolve_declared(registry: object, request: object) -> dict[str, object]:
    """Resolve exact declarations without trust, permission, routing, or effects."""
    checked_registry = validate_registry(registry)
    checked_request = validate_request(request)
    if checked_request["registry_sha256"] != checked_registry["registry_sha256"]:
        raise ContractError("request.registry_sha256 does not bind the supplied registry")
    entry, resolution = _matching_entry(checked_registry, checked_request)
    return seal("assessment", _base_assessment(
        checked_registry, checked_request, entry, resolution))


def _validate_relations(value: object) -> None:
    node = exact_object(value, set(RELATION_FIELDS), "assessment.relations")
    for field in RELATION_FIELDS:
        relation = identifier(node[field], f"assessment.relations.{field}")
        allowed = ({"not_evaluated", "same_declared_identity", "capability_id_not_found",
                    "capability_version_not_found", "capability_contract_digest_mismatch"}
                   if field == "identity" else
                   {"not_evaluated", f"same_declared_{field}", f"{field}_mismatch"})
        if relation not in allowed:
            raise ContractError(f"assessment.relations.{field}: invalid relation")


def _validate_assessment_shape(value: object) -> dict[str, object]:
    node = exact_object(value, ASSESSMENT_FIELDS, "assessment")
    fixed = {
        "api_version": ASSESSMENT_API,
        "assessment_mode": "authority_neutral_read_only_exact_declared_contract",
        "authorization_decision": "none", "canonicalization": CANONICALIZATION,
        "gate_applicability_state": "not_evaluated",
        "kind": "CapabilityRegistryDeclaredResolution",
        "owner_authentication_state": "not_evaluated",
        "proof_satisfaction_state": "not_evaluated",
        "registry_authentication_state": "not_evaluated", "result": RESULT,
        "rule_applicability_state": "not_evaluated",
    }
    for field, expected in fixed.items():
        if node[field] != expected:
            raise ContractError(f"assessment.{field}: authority-neutral constant drifted")
    for field in ("effect_attestation", "implementation_availability_attestation",
                  "invocation_attestation", "permission_attestation",
                  "persistence_attestation", "runtime_routing_attestation",
                  "test_pass_attestation", "transition_attestation"):
        if node[field] is not False:
            raise ContractError(f"assessment.{field}: must remain false")
    return node


def _validate_assessment_values(node: dict[str, object]) -> None:
    hash_value(node["assessment_sha256"], "assessment.assessment_sha256")
    hash_value(node["registry_sha256"], "assessment.registry_sha256")
    hash_value(node["request_sha256"], "assessment.request_sha256")
    if node["matched_key_entry_id"] is not None:
        identifier(node["matched_key_entry_id"], "assessment.matched_key_entry_id")
        hash_value(node["matched_key_entry_sha256"], "assessment.matched_key_entry_sha256")
    elif node["matched_key_entry_sha256"] is not None:
        raise ContractError("assessment matched entry ID/digest nullability differs")
    reasons = node["reason_codes"]
    if not isinstance(reasons, list) or len(reasons) > 32:
        raise ContractError("assessment.reason_codes: expected at most 32 items")
    for index, reason in enumerate(reasons):
        identifier(reason, f"assessment.reason_codes[{index}]")
    sorted_unique(reasons, "assessment.reason_codes", lambda item: item.encode())
    if node["resolution"] not in {
        "capability_contract_digest_mismatch", "capability_id_not_found",
        "capability_version_not_found", "resolved_exact",
    }:
        raise ContractError("assessment.resolution: unsupported value")
    _validate_relations(node["relations"])
    require_digest("assessment", node)


def validate_assessment(registry: object, request: object,
                        assessment: object) -> dict[str, object]:
    node = _validate_assessment_shape(assessment)
    _validate_assessment_values(node)
    derived = resolve_declared(registry, request)
    if canonical_json(node) != canonical_json(derived):
        raise ContractError("assessment differs from complete deterministic reassembly")
    return node


def decode_assessment(raw: bytes, registry: object,
                      request: object) -> dict[str, object]:
    value = decode_canonical(raw, max_bytes=MAX_ASSESSMENT_BYTES,
                             label="Capability Registry resolution assessment")
    return validate_assessment(registry, request, value)
