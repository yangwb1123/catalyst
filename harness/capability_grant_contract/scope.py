"""Single-effect typed scope validation and declared coverage relations."""

from __future__ import annotations

from typing import Any

from .canonical import ContractError, canonical_json
from .constants import EFFECTS, EFFECT_SPECS
from .shape import (array, enum, require_keys, sorted_unique_nodes, validate_resources,
                    validate_usage)


def _kinds(resources: list[dict[str, Any]]) -> set[str]:
    return {resource["scope_kind"] for resource in resources}


def _validate_kinds(effect_id: str, resources: list[dict[str, Any]], label: str) -> None:
    allowed, required, _, profile = EFFECT_SPECS[effect_id]
    actual = _kinds(resources)
    if not actual <= set(allowed):
        raise ContractError(f"{label} has a scope kind outside effect {effect_id!r}")
    if not set(required) <= actual:
        raise ContractError(f"{label} lacks a required scope kind for effect {effect_id!r}")
    _validate_profile(profile, resources, label)


def _validate_profile(profile: str, resources: list[dict[str, Any]], label: str) -> None:
    counts = {kind: sum(item["scope_kind"] == kind for item in resources)
              for kind in _kinds(resources)}
    if profile == "artifact_environment" and counts != {"artifact": 1, "environment": 1}:
        raise ContractError(f"{label} requires exactly one artifact and one environment")
    exact_one = {"approval_object", "knowledge_object", "policy_object", "command",
                 "network_origin", "secret_ref", "target", "target_query"}
    if profile in exact_one and len(resources) != 1:
        raise ContractError(f"{label} requires exactly one resource")
    _validate_repo_profile(profile, counts, resources, label)
    expected_object = {"approval_object": "approval", "knowledge_object": "knowledge",
                       "policy_object": "policy"}.get(profile)
    if expected_object and resources[0]["object_kind"] != expected_object:
        raise ContractError(f"{label} governance object_kind does not match {profile}")


def _validate_repo_profile(profile: str, counts: dict[str, int], resources: list[dict[str, Any]],
                           label: str) -> None:
    paths, environments = counts.get("repo_path", 0), counts.get("environment", 0)
    if profile in ("repo_read", "repo_write_exact") and not (1 <= paths <= 32 and len(resources) == paths):
        raise ContractError(f"{label} requires 1..32 repository paths")
    if profile == "environment_repo_emit" and not (
            environments == 1 and 1 <= paths <= 31 and len(resources) == paths + 1):
        raise ContractError(f"{label} requires one environment and 1..31 repository paths")
    if profile == "repo_emit_optional_environment" and not (
            environments <= 1 and 1 <= paths <= 32 - environments and
            len(resources) == paths + environments):
        raise ContractError(f"{label} requires 1..32 total paths/environment resources")


def validate_scope(value: Any) -> dict[str, Any]:
    node = require_keys(value, "scope", {"allow", "deny", "effect_id"})
    effect_id = enum(node["effect_id"], "scope.effect_id", EFFECTS)
    clauses = array(node["allow"], "scope.allow", 1, 64)
    total = 0
    for index, clause in enumerate(clauses):
        clause = require_keys(clause, f"scope.allow[{index}]", {"resources"})
        resources = validate_resources(clause["resources"], f"scope.allow[{index}].resources",
                                       maximum=32)
        total += len(resources)
        _validate_kinds(effect_id, resources, f"scope.allow[{index}]")
        _require_exact_emit_paths(effect_id, resources, f"scope.allow[{index}]")
    sorted_unique_nodes(clauses, "scope.allow")
    deny = validate_resources(node["deny"], "scope.deny", minimum=0, maximum=64)
    allowed = set(EFFECT_SPECS[effect_id][0])
    if not _kinds(deny) <= allowed:
        raise ContractError("scope.deny has a scope kind outside its effect")
    if total + len(deny) > 256:
        raise ContractError("scope contains more than 256 resources")
    return node


def validate_requested_action(value: Any) -> dict[str, Any]:
    node = require_keys(value, "requested_action", {"effect_id", "resources", "usage"})
    effect_id = enum(node["effect_id"], "requested_action.effect_id", EFFECTS)
    resources = validate_resources(node["resources"], "requested_action.resources", maximum=32)
    _validate_kinds(effect_id, resources, "requested_action.resources")
    if any(resource.get("match") != "exact" for resource in resources
           if resource["scope_kind"] == "repo_path"):
        raise ContractError("requested repository paths must use exact matching")
    validate_usage(node["usage"], "requested_action.usage")
    if effect_id == "process.exec" and resources[0]["timeout_ms"] != node["usage"]["timeout_ms"]:
        raise ContractError(
            "process.exec command timeout_ms must equal requested_action.usage.timeout_ms")
    return node


def _require_exact_emit_paths(effect_id: str, resources: list[dict[str, Any]], label: str) -> None:
    if effect_id not in ("migration.generate", "release.plan", "repo.write"):
        return
    if any(resource.get("match") != "exact" for resource in resources
           if resource["scope_kind"] == "repo_path"):
        raise ContractError(f"{label} repository emit/write paths must use exact matching")


def _repo_covers(declared: dict[str, Any], requested: dict[str, Any]) -> bool:
    if declared["match"] == "exact":
        return requested["match"] == "exact" and declared["path"] == requested["path"]
    root = declared["path"]
    return root == "." or requested["path"] == root or requested["path"].startswith(root + "/")


def resource_covers(declared: dict[str, Any], requested: dict[str, Any]) -> bool:
    if declared["scope_kind"] != requested["scope_kind"]:
        return False
    if declared["scope_kind"] == "repo_path":
        return _repo_covers(declared, requested)
    return canonical_json(declared) == canonical_json(requested)


def scope_relation(scope: dict[str, Any], action: dict[str, Any]) -> str:
    if scope["effect_id"] != action["effect_id"]:
        return "outside_declared_scope"
    if any(resource_covers(denied, requested) for denied in scope["deny"]
           for requested in action["resources"]):
        return "denied_by_declaration"
    for clause in scope["allow"]:
        if _clause_covers(scope["effect_id"], clause["resources"], action["resources"]):
            return "covered_by_declaration"
    return "outside_declared_scope"


def _clause_covers(effect_id: str, declared: list[dict[str, Any]],
                   requested: list[dict[str, Any]]) -> bool:
    if effect_id == "migration.generate":
        declared_environment = any(item["scope_kind"] == "environment" for item in declared)
        requested_environment = any(item["scope_kind"] == "environment" for item in requested)
        if declared_environment != requested_environment:
            return False
    return all(any(resource_covers(candidate, resource) for candidate in declared)
               for resource in requested)
