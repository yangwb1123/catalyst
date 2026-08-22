"""Top-level Registry and declared-resolution request validation."""

from __future__ import annotations

from .codec import ContractError, decode_canonical
from .constants import (
    CANONICALIZATION, EFFECT_VOCABULARY_SHA256, MAX_REGISTRY_BYTES,
    MAX_REQUEST_BYTES, REGISTRY_API, REGISTRY_FIELDS, REQUEST_API, FROZEN_SET_PINS,
    FROZEN_REGISTRY_SHA256,
    REQUEST_FIELDS,
)
from .digests import require_digest
from .records import validate_contract, validate_entry
from .shapes import enum, exact_object, hash_value, identifier, version


def _frozen_entry_relations(entry: dict[str, object]) -> None:
    contract = entry["contract"]
    if contract["domain"] != "reasoning" or contract["risk_floor"] != "L1":
        raise ContractError("entry contract domain/risk floor drifted from singleton v1")
    if contract["effects"] or contract["permission_requirements"]:
        raise ContractError("singleton v1 contract must remain effect- and permission-free")
    set_hashes = {item["set_sha256"] for item in entry["content_sets"]}
    for content_set in entry["content_sets"]:
        selection = content_set["selection"]
        key = selection["root"] if selection["root"] is not None else "explicit"
        pin = FROZEN_SET_PINS.get(key)
        actual = (len(content_set["files"]),
                  sum(item["content_bytes"] for item in content_set["files"]),
                  content_set["set_sha256"])
        if pin is None or actual != pin:
            raise ContractError("entry content-set count/bytes/digest drifted from v1")
        if key == "explicit" and (selection["mode"] != "explicit_files" or
                                  selection["suffixes"]):
            raise ContractError("entry explicit content-set selection drifted")
        if key != "explicit" and (selection["mode"] !=
                                  "all_regular_files_recursive_with_suffixes" or
                                  selection["suffixes"] != [".go" if key.startswith(
                                      "forge-core/") else ".py"]):
            raise ContractError("entry recursive content-set selection drifted")
    implementation_ids = [item["implementation_id"] for item in entry["implementations"]]
    test_ids = [item["test_id"] for item in entry["tests"]]
    if implementation_ids != ["go", "python"]:
        raise ContractError("entry implementations must be the frozen Go/Python pair")
    if test_ids != ["go-contract-suite", "python-contract-suite"]:
        raise ContractError("entry tests must be the frozen Go/Python pair")
    by_root = {item["selection"]["root"]: item["set_sha256"]
               for item in entry["content_sets"]}
    expected = {
        ("implementation", "go"): by_root.get("forge-core/internal/goimpactprescan"),
        ("implementation", "python"): by_root.get(
            "harness/local_go_package_impact_prescan_contract"),
        ("test", "go-contract-suite"): by_root.get("forge-core/internal/goimpactprescan"),
        ("test", "python-contract-suite"): by_root.get(None),
    }
    for item in entry["implementations"]:
        if item["source_set_sha256"] != expected[("implementation", item["implementation_id"])]:
            raise ContractError("entry implementation is wired to the wrong content set")
    for item in entry["tests"]:
        if item["source_set_sha256"] != expected[("test", item["test_id"])]:
            raise ContractError("entry test is wired to the wrong content set")


def validate_registry(value: object) -> dict[str, object]:
    node = exact_object(value, REGISTRY_FIELDS, "registry")
    fixed = {
        "api_version": REGISTRY_API,
        "canonicalization": CANONICALIZATION,
        "coverage_mode": "explicit_entries_only_not_global_inventory",
        "effect_vocabulary_sha256": EFFECT_VOCABULARY_SHA256,
        "kind": "CapabilityRegistry",
        "registry_mode": "authority_neutral_read_only_contract_catalog",
        "status": "staged",
    }
    for field, expected in fixed.items():
        if node[field] != expected:
            raise ContractError(f"registry.{field}: frozen value drifted")
    entries = node["entries"]
    if not isinstance(entries, list) or len(entries) != 1:
        raise ContractError("registry.entries: v1 requires exactly one entry")
    validate_entry(entries[0])
    _frozen_entry_relations(entries[0])
    require_digest("registry", node)
    if node["registry_sha256"] != FROZEN_REGISTRY_SHA256:
        raise ContractError("registry differs from the exact frozen singleton v1 profile")
    return node


def decode_registry(raw: bytes) -> dict[str, object]:
    value = decode_canonical(raw, max_bytes=MAX_REGISTRY_BYTES, label="Capability Registry v1")
    return validate_registry(value)


def _validate_reference(value: object) -> dict[str, object]:
    fields = {"capability_contract_sha256", "capability_id", "capability_version", "origin"}
    node = exact_object(value, fields, "request.expected_reference")
    hash_value(node["capability_contract_sha256"],
               "request.expected_reference.capability_contract_sha256")
    identifier(node["capability_id"], "request.expected_reference.capability_id")
    version(node["capability_version"], "request.expected_reference.capability_version")
    enum(node["origin"], {"current_registry", "external_declared", "external_legacy"},
         "request.expected_reference.origin")
    return node


def validate_request(value: object) -> dict[str, object]:
    node = exact_object(value, REQUEST_FIELDS, "request")
    if (node["api_version"] != REQUEST_API or
            node["kind"] != "CapabilityRegistryDeclaredResolutionRequest" or
            node["canonicalization"] != CANONICALIZATION):
        raise ContractError("request: unsupported envelope")
    reference = _validate_reference(node["expected_reference"])
    expected = node["expected_contract"]
    if expected is not None:
        contract = validate_contract(expected)
        projection = (contract["capability_id"], contract["capability_version"],
                      contract["capability_contract_sha256"])
        declared = (reference["capability_id"], reference["capability_version"],
                    reference["capability_contract_sha256"])
        if projection != declared:
            raise ContractError("request expected contract/reference projection mismatch")
    hash_value(node["registry_sha256"], "request.registry_sha256")
    require_digest("request", node)
    return node


def decode_request(raw: bytes) -> dict[str, object]:
    value = decode_canonical(raw, max_bytes=MAX_REQUEST_BYTES,
                             label="Capability Registry resolution request")
    return validate_request(value)
