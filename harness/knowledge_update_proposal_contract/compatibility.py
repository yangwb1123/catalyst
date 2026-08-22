"""Pure Grant, reassembled Context, and artifact declaration comparisons."""

from __future__ import annotations

from typing import Any

import capability_grant_contract
import context_package_contract
from capability_grant_contract.scope import scope_relation

from .canonical import ContractError, bounded_canonical_json
from .constants import CONTEXT_RESULT, GRANT_RESULT, MAX_PROPOSAL_BYTES
from .proposal import validate_proposal
from .shape import sorted_nodes


def project_capability_grant_ref(grant: dict[str, Any]) -> dict[str, str]:
    capability_grant_contract.grant.validate_grant(grant)
    issuer = grant["authority_proof"]["issuer"]
    return {"authority_domain": issuer["authority_domain"], "grant_id": grant["grant_id"],
            "grant_sha256": grant["grant_sha256"]}


def project_artifact_resources(proposal: dict[str, Any]) -> list[dict[str, Any]]:
    """Project declared artifacts into typed resources without validating an artifact."""
    validate_proposal(proposal)
    projected = [{**artifact, "scope_kind": "artifact"}
                 for artifact in proposal["bindings"]["artifacts"]]
    sorted_nodes(projected, "projected artifact resources")
    return projected


def _same(left: Any, right: Any, field: str) -> str:
    return f"same_declared_{field}" if left == right else f"{field}_mismatch"


def _result(relations: dict[str, str], result: str,
            reasons: list[str]) -> dict[str, Any]:
    return {"reason_codes": reasons, "relations": relations, "result": result}


def assess_declared_grant_compatibility(grant: dict[str, Any],
                                        proposal: dict[str, Any]) -> dict[str, Any]:
    """Compare strict caller-provided declarations without authenticating the Grant."""
    bounded_canonical_json(proposal, MAX_PROPOSAL_BYTES, "KnowledgeUpdateProposal")
    capability_grant_contract.grant.validate_grant(grant)
    validate_proposal(proposal)
    common = ("context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
              "risk_sha256", "source_revision", "source_tree_sha256")
    proposal_bindings = {field: proposal["bindings"][field] for field in common}
    grant_bindings = {field: grant["bindings"][field] for field in common}
    submitted, validity = proposal["submitted_at_unix_ms"], grant["validity"]
    action = {"effect_id": "knowledge.propose",
              "resources": [proposal["knowledge_scope"]]}
    relations = {
        "bindings": _same(proposal_bindings, grant_bindings, "bindings"),
        "grant_ref": _same(proposal["capability_grant_ref"],
                           project_capability_grant_ref(grant), "grant_ref"),
        "proposer": _same(proposal["proposer"], grant["subject"], "proposer"),
        "task_binding": _same(proposal["task_binding"], grant["task_binding"],
                              "task_binding"),
        "effect": ("same_declared_effect" if grant["scope"]["effect_id"] ==
                   "knowledge.propose" else "effect_mismatch"),
        "scope": scope_relation(grant["scope"], action),
        "declared_time": ("same_declared_time" if
                     validity["not_before_unix_ms"] <= submitted <
                     validity["expires_at_unix_ms"] else "declared_time_mismatch"),
    }
    reasons = []
    for field, relation in relations.items():
        if relation.startswith("same_declared_") or relation == "covered_by_declaration":
            continue
        if field == "scope" and relations["effect"] == "effect_mismatch":
            continue
        reasons.append({"denied_by_declaration": "deny_matched",
                        "outside_declared_scope": "scope_not_covered"}.get(
                            relation, relation))
    return _result(relations, GRANT_RESULT, sorted(set(reasons)))


def assess_reassembled_context_compatibility(
        context_request: dict[str, Any], context_package: dict[str, Any],
        proposal: dict[str, Any], counter: context_package_contract.TokenCounter) -> dict[str, Any]:
    """Compare only after the caller's ContextPackage has been exactly reassembled."""
    bounded_canonical_json(proposal, MAX_PROPOSAL_BYTES, "KnowledgeUpdateProposal")
    context_package_contract.validate_package(context_request, context_package, counter)
    validate_proposal(proposal)
    source = context_package["source_binding"]
    task = context_package["task_binding"]
    proposal_task = proposal["task_binding"]
    shared = ("change_id", "node_id", "project_id", "role", "run_id", "task_id")
    expires = context_package["freshness"]["expires_at_unix_ms"]
    submitted = proposal["submitted_at_unix_ms"]
    temporal = context_package["freshness"]["evaluated_at_unix_ms"] <= submitted
    if expires is not None:
        temporal = temporal and submitted < expires
    relations = {
        "context": _same(proposal["bindings"]["context_sha256"],
                         context_package["context_sha256"], "context"),
        "policy": _same(proposal["bindings"]["policy_sha256"],
                        source["policy_sha256"], "policy"),
        "source": _same((proposal["bindings"]["source_revision"],
                         proposal["bindings"]["source_tree_sha256"]),
                        (source["source_revision"], source["source_tree_sha256"]), "source"),
        "task_binding": _same(tuple(proposal_task[field] for field in shared),
                              tuple(task[field] for field in shared), "task_binding"),
        "freshness": ("inside_declared_freshness" if temporal
                      else "outside_declared_freshness"),
    }
    reasons = []
    for relation in relations.values():
        if relation.startswith("same_declared_") or relation == "inside_declared_freshness":
            continue
        reasons.append("freshness_mismatch" if relation == "outside_declared_freshness"
                       else relation)
    return _result(relations, CONTEXT_RESULT, sorted(set(reasons)))
