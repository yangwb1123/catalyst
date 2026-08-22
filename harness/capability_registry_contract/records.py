"""Capability contract and registry-entry record validation."""

from __future__ import annotations

from capability_grant_contract.constants import EFFECTS

from .codec import ContractError, canonical_json
from .constants import CANONICALIZATION, CONTRACT_API, CONTRACT_FIELDS, ENTRY_API, ENTRY_FIELDS
from .constants import MAX_NARRATIVE_BYTES
from .digests import require_digest
from .shapes import (
    array, enum, exact_object, hash_value, identifier, repo_path, sorted_unique,
    string, string_set, validate_content_ref, validate_content_set,
    validate_failure, validate_gate, validate_permission, validate_predicate_set,
    unique_ref_targets, validate_rollback, validate_rule, validate_signal, version,
)

DOMAINS = {"device", "execution", "governance", "planning", "reasoning", "verification"}


def _structured_set(node: dict[str, object], field: str, identity: str,
                    validator, low: int = 1, high: int = 64) -> list[object]:
    values = array(node[field], f"contract.{field}", low, high)
    for index, value in enumerate(values):
        validator(value, f"contract.{field}[{index}]")
    sorted_unique(values, f"contract.{field}",
                  lambda item: string(item[identity], identity).encode())
    return values


def _validate_proof(value: object, label: str) -> dict[str, object]:
    fields = {"description", "obligation_id", "verification_refs"}
    node = exact_object(value, fields, label)
    string(node["description"], f"{label}.description", maximum=MAX_NARRATIVE_BYTES)
    identifier(node["obligation_id"], f"{label}.obligation_id")
    refs = array(node["verification_refs"], f"{label}.verification_refs", 1, 64)
    for index, ref in enumerate(refs):
        validate_content_ref(ref, f"{label}.verification_refs[{index}]")
    sorted_unique(refs, f"{label}.verification_refs", canonical_json)
    unique_ref_targets(refs, f"{label}.verification_refs")
    return node


def _validate_schemas(node: dict[str, object], field: str) -> None:
    values = array(node[field], f"contract.{field}", 1, 64)
    for index, value in enumerate(values):
        validate_content_ref(value, f"contract.{field}[{index}]", schema_only=True)
    sorted_unique(values, f"contract.{field}", canonical_json)
    unique_ref_targets(values, f"contract.{field}")


def _validate_permissions(node: dict[str, object]) -> None:
    values = _structured_set(node, "permission_requirements", "requirement_id",
                             validate_permission, low=0)
    effects = node["effects"]
    for permission in values:
        if permission["effect_id"] not in effects:
            raise ContractError("permission requirement references an undeclared effect")
    if not effects and values:
        raise ContractError("permission requirements must be empty when effects are empty")


def validate_contract(value: object) -> dict[str, object]:
    node = exact_object(value, CONTRACT_FIELDS, "contract")
    if (node["api_version"] != CONTRACT_API or node["kind"] != "CapabilityContract" or
            node["canonicalization"] != CANONICALIZATION):
        raise ContractError("contract: unsupported envelope")
    if node["capability_id"] != "local-go-package-impact-prescan":
        raise ContractError("contract.capability_id: v1 singleton identity drifted")
    if version(node["capability_version"], "contract.capability_version") != "1":
        raise ContractError("contract.capability_version: v1 singleton version drifted")
    enum(node["domain"], DOMAINS, "contract.domain")
    enum(node["risk_floor"], {"L0", "L1", "L2", "L3", "L4"}, "contract.risk_floor")
    _validate_schemas(node, "input_schemas")
    _validate_schemas(node, "output_schemas")
    validate_predicate_set(node["trigger"], "contract.trigger")
    validate_predicate_set(node["not_applicable"], "contract.not_applicable")
    string_set(node["preconditions"], "contract.preconditions", 1)
    string_set(node["postconditions"], "contract.postconditions", 1)
    string_set(node["effects"], "contract.effects", choices=set(EFFECTS))
    _structured_set(node, "proof_obligations", "obligation_id", _validate_proof)
    _structured_set(node, "failure_modes", "failure_id", validate_failure)
    _structured_set(node, "observability", "signal_id", validate_signal)
    _structured_set(node, "rules", "rule_id", validate_rule)
    _structured_set(node, "quality_gates", "gate_id", validate_gate)
    _validate_permissions(node)
    validate_rollback(node["rollback_or_compensation"], "contract.rollback_or_compensation")
    require_digest("contract", node)
    return node


def _validate_adapter(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"adapter_id", "adapter_kind", "entrypoint"}, label)
    identifier(node["adapter_id"], f"{label}.adapter_id")
    enum(node["adapter_kind"], {"command_line", "library_api"}, f"{label}.adapter_kind")
    string(node["entrypoint"], f"{label}.entrypoint", maximum=MAX_NARRATIVE_BYTES)
    return node


def _validate_implementation(value: object, label: str) -> dict[str, object]:
    fields = {"adapters", "implementation_id", "language", "runtime_profile", "source_set_sha256"}
    node = exact_object(value, fields, label)
    identifier(node["implementation_id"], f"{label}.implementation_id")
    enum(node["language"], {"go", "python"}, f"{label}.language")
    identifier(node["runtime_profile"], f"{label}.runtime_profile")
    hash_value(node["source_set_sha256"], f"{label}.source_set_sha256")
    adapters = array(node["adapters"], f"{label}.adapters", 1, 16)
    for index, adapter in enumerate(adapters):
        _validate_adapter(adapter, f"{label}.adapters[{index}]")
    sorted_unique(adapters, f"{label}.adapters",
                  lambda item: item["adapter_id"].encode())
    return node


def _validate_test(value: object, label: str) -> dict[str, object]:
    fields = {"covers_gate_ids", "entrypoint", "fixture_refs", "source_set_sha256",
              "test_id", "test_kinds"}
    node = exact_object(value, fields, label)
    identifier(node["test_id"], f"{label}.test_id")
    string(node["entrypoint"], f"{label}.entrypoint", maximum=MAX_NARRATIVE_BYTES)
    hash_value(node["source_set_sha256"], f"{label}.source_set_sha256")
    string_set(node["covers_gate_ids"], f"{label}.covers_gate_ids", 1)
    kinds = array(node["test_kinds"], f"{label}.test_kinds", 1, 8)
    allowed = {"adversarial", "bounds", "cross_language_golden", "unit"}
    for index, kind in enumerate(kinds):
        enum(kind, allowed, f"{label}.test_kinds[{index}]")
    sorted_unique(kinds, f"{label}.test_kinds", lambda item: item.encode())
    refs = array(node["fixture_refs"], f"{label}.fixture_refs", 1, 64)
    for index, ref in enumerate(refs):
        validate_content_ref(ref, f"{label}.fixture_refs[{index}]")
    sorted_unique(refs, f"{label}.fixture_refs", canonical_json)
    unique_ref_targets(refs, f"{label}.fixture_refs")
    return node


def _validate_owner(value: object) -> dict[str, object]:
    node = exact_object(value, {"module", "team"}, "entry.owner")
    repo_path(node["module"], "entry.owner.module")
    identifier(node["team"], "entry.owner.team")
    return node


def validate_entry(value: object) -> dict[str, object]:
    node = exact_object(value, ENTRY_FIELDS, "entry")
    if (node["api_version"] != ENTRY_API or node["kind"] != "CapabilityRegistryEntry" or
            node["canonicalization"] != CANONICALIZATION):
        raise ContractError("entry: unsupported envelope")
    if node["catalog_binding"] is not None:
        raise ContractError("entry.catalog_binding: singleton foundation capability requires null")
    validate_contract(node["contract"])
    sets = array(node["content_sets"], "entry.content_sets", 3, 3)
    for index, content_set in enumerate(sets):
        validate_content_set(content_set, f"entry.content_sets[{index}]")
    sorted_unique(sets, "entry.content_sets", canonical_json)
    implementations = array(node["implementations"], "entry.implementations", 2, 8)
    for index, implementation in enumerate(implementations):
        _validate_implementation(implementation, f"entry.implementations[{index}]")
    sorted_unique(implementations, "entry.implementations",
                  lambda item: item["implementation_id"].encode())
    tests = array(node["tests"], "entry.tests", 2, 8)
    for index, test in enumerate(tests):
        _validate_test(test, f"entry.tests[{index}]")
    sorted_unique(tests, "entry.tests", lambda item: item["test_id"].encode())
    _validate_owner(node["owner"])
    require_digest("entry", node)
    return node
