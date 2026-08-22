"""Closed planning source shapes and complete unique ownership extraction."""

from __future__ import annotations

from dataclasses import dataclass

from .codec import ContractError
from .constants import MAX_CATALOG_BYTES, MAX_MAPPING_BYTES
from .shapes import array, exact_object, fixed, identifier, integer, node_id, skill_name, string
from .yaml_subset import parse_yaml

CATALOG_FIELDS = {
    "api_version", "authority_semantics", "canonical_vocabulary",
    "control_plane_joins", "decision_ref", "executable", "extension_decision_refs",
    "gates", "kind", "nodes", "risk_levels", "runtime_note", "status",
    "universal_node_contract",
}
NODE_FIELDS = {
    "activities", "authority", "capabilities", "entry_criteria", "escalation",
    "exit_criteria", "forbidden", "handoff", "id", "inputs", "memory_updates",
    "name", "outputs", "owner_lens", "purpose", "quality_gates", "rules",
}
MAPPING_FIELDS = {
    "api_version", "executable", "kind", "mapping_rules", "packages",
    "skill_specification", "source_catalog", "status",
}
PACKAGE_FIELDS = {"implementation_wave", "includes", "skill"}
NODE_LIST_FIELDS = NODE_FIELDS - {"id", "name", "owner_lens", "purpose"}


@dataclass(frozen=True)
class Ownership:
    nodes: dict[str, tuple[str, ...]]
    owners: dict[str, tuple[str, int]]
    occurrences: int
    package_count: int


def _string_list(value: object, label: str, identifiers: bool = False) -> list[str]:
    values = array(value, 1, 512, label)
    result = []
    for index, item in enumerate(values):
        checker = identifier if identifiers else string
        result.append(checker(item, f"{label}[{index}]"))
    return result


def _shape_ignored(value: object, expected: type, label: str) -> None:
    if not isinstance(value, expected):
        raise ContractError(f"{label}: unexpected source shape")


def validate_catalog(value: object) -> tuple[dict[str, tuple[str, ...]], int]:
    catalog = exact_object(value, CATALOG_FIELDS, "catalog")
    fixed(catalog["api_version"], "forgeos.design/v1", "catalog.api_version")
    fixed(catalog["kind"], "AIEngineeringCapabilityCatalog", "catalog.kind")
    fixed(catalog["status"], "planning_only", "catalog.status")
    fixed(catalog["executable"], False, "catalog.executable")
    for field in ("canonical_vocabulary", "control_plane_joins", "authority_semantics",
                  "risk_levels", "universal_node_contract", "gates"):
        _shape_ignored(catalog[field], dict, f"catalog.{field}")
    _string_list(catalog["extension_decision_refs"], "catalog.extension_decision_refs")
    string(catalog["decision_ref"], "catalog.decision_ref")
    string(catalog["runtime_note"], "catalog.runtime_note")
    nodes, occurrences = {}, 0
    for index, raw in enumerate(array(catalog["nodes"], 1, 64, "catalog.nodes")):
        node = _validate_node(raw, index)
        ident, capabilities = node["id"], tuple(node["capabilities"])
        if ident in nodes:
            raise ContractError(f"catalog.nodes[{index}].id: duplicate")
        nodes[ident] = capabilities
        occurrences += len(capabilities)
    unique_count = len({item for capabilities in nodes.values() for item in capabilities})
    if unique_count > 512 or occurrences > 4096:
        raise ContractError("catalog capability coverage exceeds bounds")
    return nodes, occurrences


def _validate_node(value: object, index: int) -> dict[str, object]:
    label = f"catalog.nodes[{index}]"
    node = exact_object(value, NODE_FIELDS, label)
    node_id(node["id"], f"{label}.id")
    for field in ("name", "owner_lens", "purpose"):
        string(node[field], f"{label}.{field}")
    for field in NODE_LIST_FIELDS:
        values = array(node[field], 1, 512, f"{label}.{field}")
        if field == "capabilities":
            checked = _string_list(values, f"{label}.{field}", True)
            if len(checked) != len(set(checked)):
                raise ContractError(f"{label}.capabilities: duplicate")
    return node


def validate_mapping(value: object) -> tuple[dict[str, tuple[str, int]], int]:
    mapping = exact_object(value, MAPPING_FIELDS, "mapping")
    fixed(mapping["api_version"], "forgeos.design/v1", "mapping.api_version")
    fixed(mapping["kind"], "CapabilitySkillOwnershipMap", "mapping.kind")
    fixed(mapping["status"], "planning_only", "mapping.status")
    fixed(mapping["executable"], False, "mapping.executable")
    fixed(mapping["source_catalog"], "capability-catalog.v1.yml", "mapping.source_catalog")
    string(mapping["skill_specification"], "mapping.skill_specification")
    _string_list(mapping["mapping_rules"], "mapping.mapping_rules")
    owners, skills = {}, set()
    packages = array(mapping["packages"], 1, 64, "mapping.packages")
    for index, raw in enumerate(packages):
        package = exact_object(raw, PACKAGE_FIELDS, f"mapping.packages[{index}]")
        skill = skill_name(package["skill"], f"mapping.packages[{index}].skill")
        if skill in skills:
            raise ContractError(f"mapping.packages[{index}].skill: duplicate")
        skills.add(skill)
        wave = integer(package["implementation_wave"], 1, 6,
                       f"mapping.packages[{index}].implementation_wave")
        includes = _string_list(package["includes"], f"mapping.packages[{index}].includes", True)
        if len(includes) != len(set(includes)):
            raise ContractError(f"mapping.packages[{index}].includes: duplicate")
        for capability in includes:
            if capability in owners:
                raise ContractError(f"mapping capability {capability!r}: duplicate primary owner")
            owners[capability] = (skill, wave)
            if len(owners) > 512:
                raise ContractError("mapping capability coverage exceeds bounds")
    return owners, len(packages)


def parse_sources(catalog_raw: bytes, mapping_raw: bytes) -> Ownership:
    if not isinstance(catalog_raw, bytes) or not 1 <= len(catalog_raw) <= MAX_CATALOG_BYTES:
        raise ContractError(f"catalog source byte length must be 1..{MAX_CATALOG_BYTES}")
    if not isinstance(mapping_raw, bytes) or not 1 <= len(mapping_raw) <= MAX_MAPPING_BYTES:
        raise ContractError(f"mapping source byte length must be 1..{MAX_MAPPING_BYTES}")
    nodes, occurrences = validate_catalog(parse_yaml(catalog_raw, MAX_CATALOG_BYTES))
    owners, packages = validate_mapping(parse_yaml(mapping_raw, MAX_MAPPING_BYTES))
    catalog_capabilities = {item for values in nodes.values() for item in values}
    if catalog_capabilities != set(owners):
        missing = sorted(catalog_capabilities - set(owners))
        extra = sorted(set(owners) - catalog_capabilities)
        raise ContractError(f"ownership coverage mismatch missing={missing} extra={extra}")
    return Ownership(nodes, owners, occurrences, packages)
