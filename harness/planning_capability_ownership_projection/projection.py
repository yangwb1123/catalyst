"""Pure declared ownership projection and exact reassembly validator."""

from __future__ import annotations

import copy
import hashlib

from .codec import ContractError, canonical_json, decode_canonical
from .constants import (
    API_PROJECTION, AUTHORITY, BINDING_DOMAIN, CANONICAL, MAX_BINDING_BYTES,
    MAX_PROJECTION_BYTES, POSITIVE_RESULT, PROJECTION_DOMAIN,
)
from .request import validate_request
from .sources import Ownership


def _hash(domain: bytes, value: object, maximum: int | None = None) -> str:
    return hashlib.sha256(domain + canonical_json(value, maximum)).hexdigest()


def _occurrences(ownership: Ownership) -> dict[str, list[str]]:
    result: dict[str, list[str]] = {capability: [] for capability in ownership.owners}
    for node, capabilities in ownership.nodes.items():
        for capability in capabilities:
            result[capability].append(node)
    return result


def _binding(capability: str, nodes: list[str], owner: tuple[str, int],
             request_sha256: str) -> dict[str, object]:
    skill, wave = owner
    unique_nodes = sorted(set(nodes), key=lambda item: item.encode("utf-8"))
    value = {
        "binding_sha256": "",
        "capability_id": capability,
        "catalog_node_ids": unique_nodes,
        "catalog_occurrence_count": len(nodes),
        "declared_logical_adapter_ref": f".agent/skills/{skill}.md",
        "implementation_wave": wave,
        "owner_skill": skill,
        "physical_resolution": "not_performed",
        "request_sha256": request_sha256,
        "skill_availability": "not_evaluated",
    }
    value["binding_sha256"] = _hash(BINDING_DOMAIN, value, MAX_BINDING_BYTES)
    return value


def _coverage(ownership: Ownership, count: int) -> dict[str, object]:
    return {
        "binding_count": count,
        "capability_occurrence_count": ownership.occurrences,
        "catalog_node_count": len(ownership.nodes),
        "mapped_capability_count": len(ownership.owners),
        "mapping_package_count": ownership.package_count,
        "unique_capability_count": len(ownership.owners),
        "unmapped_capability_ids": [],
        "unreferenced_mapping_capability_ids": [],
    }


def project(request_value: object) -> dict[str, object]:
    request, ownership = validate_request(request_value)
    request_copy = copy.deepcopy(request)
    occurrences = _occurrences(ownership)
    capabilities = sorted(ownership.owners, key=lambda item: item.encode("utf-8"))
    bindings = [_binding(item, occurrences[item], ownership.owners[item],
                         request["request_sha256"]) for item in capabilities]
    result = {
        "api_version": API_PROJECTION,
        "authority_semantics": copy.deepcopy(AUTHORITY),
        "bindings": bindings,
        "canonicalization": CANONICAL,
        "coverage": _coverage(ownership, len(bindings)),
        "kind": "PlanningCapabilityOwnershipProjection",
        "positive_result": POSITIVE_RESULT,
        "projection_mode": "planning_only_declared_ownership_and_logical_adapter_refs",
        "projection_sha256": "",
        "request": request_copy,
        "request_sha256": request["request_sha256"],
        "status": "planning_only",
    }
    result["projection_sha256"] = _hash(PROJECTION_DOMAIN, result, MAX_PROJECTION_BYTES)
    canonical_json(result, MAX_PROJECTION_BYTES)
    return result


def validate_projection(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or "request" not in value:
        raise ContractError("projection: expected object with request")
    expected = project(value["request"])
    if canonical_json(value, MAX_PROJECTION_BYTES) != canonical_json(expected, MAX_PROJECTION_BYTES):
        raise ContractError("projection differs from complete deterministic reassembly")
    return value


def decode_projection(raw: bytes) -> dict[str, object]:
    value = decode_canonical(raw, MAX_PROJECTION_BYTES, "ownership projection")
    return validate_projection(value)
