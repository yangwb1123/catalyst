"""Exact proposal policy, authenticated-role declarations, and SoD relations."""

from __future__ import annotations

from typing import Any

from .authority import key_by_id, key_for_usage
from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (
    CANONICALIZATION,
    GATE_ID,
    MAX_POLICY_BYTES,
    MAX_POLICY_VALIDITY_MS,
    MAX_REQUEST_VALIDITY_MS,
    POLICY_API,
    POLICY_DOMAIN,
    PROFILE_ID,
    REQUIRED_DISTINCTIONS,
)
from .proposal import validate_proposal_binding
from .shape import (enum, integer, principal_identity, require_keys, sha256, stable_id,
                    stable_ref, sorted_unique_nodes, sorted_unique_strings, text,
                    validate_principal, validate_signature)

POLICY_FIELDS = {
    "api_version", "approval_record_profile", "canonicalization", "disposition",
    "eligible_approver_key_ids", "kind", "max_request_validity_ms", "policy_id",
    "policy_sha256", "profile_id", "proposal_binding", "required_distinctions",
    "roles", "signature", "threshold", "trust_epoch", "trust_root_sha256",
    "validity", "veto_on_reject",
}
ROLES_FIELDS = {"approver_bindings", "implementers", "owner_bindings", "requester"}
APPROVER_BINDING_FIELDS = {"approver_ref", "key_id"}
OWNER_BINDING_FIELDS = {"owner_ref", "principal"}
APPROVAL_PROFILE_FIELDS = {
    "change_id", "context_sha256", "environment_class", "environment_id", "gate_id",
    "impact_sha256", "materiality_level", "plan_sha256", "project_id", "risk_sha256",
    "source_revision", "source_tree_sha256", "subject",
}


def policy_sha256(value: dict[str, Any]) -> str:
    return self_digest(POLICY_DOMAIN, value, ("policy_sha256",), MAX_POLICY_BYTES,
                       "ArchitectureDecisionApprovalPolicy", signed=True)


def validate_policy(value: Any, profile_hash: str, root: dict[str, Any],
                    proposal_metadata: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionApprovalPolicy"
    node = require_keys(value, label, POLICY_FIELDS)
    bounded_canonical_json(node, MAX_POLICY_BYTES, label)
    expected = (POLICY_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    stable_id(node["policy_id"], "policy.policy_id")
    enum(node["disposition"], "policy.disposition", ("allow", "deny"))
    integer(node["max_request_validity_ms"], "policy.max_request_validity_ms", 1,
            MAX_REQUEST_VALIDITY_MS)
    _validate_validity(node["validity"])
    _validate_proposal_time(node["validity"], proposal_metadata)
    _validate_approval_profile(node["approval_record_profile"])
    _validate_policy_controls(node)
    _validate_roles(node, root, proposal_metadata)
    _validate_authority(node, profile_hash, root)
    binding = validate_proposal_binding(node["proposal_binding"])
    _validate_binding_metadata(binding, proposal_metadata)
    sha256(node["policy_sha256"], "policy.policy_sha256")
    if node["policy_sha256"] != policy_sha256(node):
        raise ContractError("policy self digest does not match")
    return node


def _validate_binding_metadata(binding: dict[str, Any], metadata: dict[str, Any]) -> None:
    fields = ("adr_id", "body_sha256", "document_name", "self_sha256", "status")
    if any(binding[field] != metadata[field] for field in fields):
        raise ContractError("policy ProposalBinding differs from supplied proposal metadata")


def _validate_validity(value: Any) -> None:
    node = require_keys(value, "policy.validity",
                        {"expires_at_unix_ms", "not_before_unix_ms"})
    starts = integer(node["not_before_unix_ms"], "policy.validity.not_before_unix_ms",
                     0, 2**63 - 1)
    expires = integer(node["expires_at_unix_ms"], "policy.validity.expires_at_unix_ms",
                      0, 2**63 - 1)
    if not starts < expires or expires - starts > MAX_POLICY_VALIDITY_MS:
        raise ContractError("policy validity must be ordered within 24 hours")


def _validate_proposal_time(validity: dict[str, Any], metadata: dict[str, Any]) -> None:
    if validity["not_before_unix_ms"] < metadata["proposed_at_unix_ms"]:
        raise ContractError("policy validity begins before the proposal declared time")
    proposal_expiry = metadata["expires_at_unix_ms"]
    if proposal_expiry is not None and validity["expires_at_unix_ms"] > proposal_expiry:
        raise ContractError("policy validity extends beyond proposal declared expiry")


def _validate_approval_profile(value: Any) -> None:
    label = "policy.approval_record_profile"
    node = require_keys(value, label, APPROVAL_PROFILE_FIELDS)
    for field in ("change_id", "environment_id", "project_id", "source_revision"):
        text(node[field], f"{label}.{field}")
    if node["gate_id"] != GATE_ID or node["materiality_level"] != "L4":
        raise ContractError("approval record profile must use the fixed gate at L4")
    enum(node["environment_class"], f"{label}.environment_class",
         ("development", "local", "production", "staging", "test"))
    for field in ("context_sha256", "impact_sha256", "plan_sha256", "risk_sha256",
                  "source_tree_sha256"):
        sha256(node[field], f"{label}.{field}")
    validate_principal(node["subject"], f"{label}.subject", ("service",))


def _validate_policy_controls(node: dict[str, Any]) -> None:
    eligible = sorted_unique_strings(node["eligible_approver_key_ids"],
                                     "policy.eligible_approver_key_ids", 2, 16)
    threshold = integer(node["threshold"], "policy.threshold", 2, 16)
    if threshold > len(eligible):
        raise ContractError("policy threshold exceeds eligible approver count")
    distinctions = sorted_unique_strings(node["required_distinctions"],
                                         "policy.required_distinctions", 5, 5)
    if tuple(distinctions) != REQUIRED_DISTINCTIONS:
        raise ContractError("policy required distinctions drifted from v1")
    if node["veto_on_reject"] is not True:
        raise ContractError("policy veto_on_reject must be true")


def _validate_roles(node: dict[str, Any], root: dict[str, Any],
                    metadata: dict[str, Any]) -> None:
    roles = require_keys(node["roles"], "policy.roles", ROLES_FIELDS)
    approvers = _validate_approver_bindings(roles["approver_bindings"], root,
                                            metadata["approver_refs"])
    owners = _validate_owner_bindings(roles["owner_bindings"], metadata["owner_refs"])
    implementers = sorted_unique_nodes(roles["implementers"], "policy.roles.implementers",
                                       1, 32)
    for index, principal in enumerate(implementers):
        validate_principal(principal, f"policy.roles.implementers[{index}]")
    requester = validate_principal(roles["requester"], "policy.roles.requester")
    eligible = node["eligible_approver_key_ids"]
    if sorted(binding["key_id"] for binding in approvers) != eligible:
        raise ContractError("eligible approver keys differ from approver-ref mappings")
    _validate_role_separation(root, eligible, owners, implementers, requester,
                              node["approval_record_profile"]["subject"])


def _validate_approver_bindings(value: Any, root: dict[str, Any],
                                declared_refs: list[str]) -> list[dict[str, Any]]:
    nodes = sorted_unique_nodes(value, "policy.roles.approver_bindings", 2, 16)
    for index, item in enumerate(nodes):
        node = require_keys(item, f"approver_bindings[{index}]", APPROVER_BINDING_FIELDS)
        stable_ref(node["approver_ref"], f"approver_bindings[{index}].approver_ref")
        key = key_by_id(root, text(node["key_id"], f"approver_bindings[{index}].key_id"))
        if key["usage"] != "architecture_approval_sign":
            raise ContractError("approver binding key has the wrong root usage")
    if [node["approver_ref"] for node in nodes] != declared_refs:
        raise ContractError("policy must map every exact proposal approver_ref")
    if len({node["key_id"] for node in nodes}) != len(nodes):
        raise ContractError("approver bindings must use unique root keys")
    return nodes


def _validate_owner_bindings(value: Any, declared_refs: list[str]) -> list[dict[str, Any]]:
    nodes = sorted_unique_nodes(value, "policy.roles.owner_bindings", 1, 64)
    for index, item in enumerate(nodes):
        node = require_keys(item, f"owner_bindings[{index}]", OWNER_BINDING_FIELDS)
        stable_ref(node["owner_ref"], f"owner_bindings[{index}].owner_ref")
        validate_principal(node["principal"], f"owner_bindings[{index}].principal")
    if [node["owner_ref"] for node in nodes] != declared_refs:
        raise ContractError("policy must map every exact proposal owner_ref")
    identities = [principal_identity(node["principal"]) for node in nodes]
    if len(set(identities)) != len(nodes):
        raise ContractError("owner bindings must map to unique principals")
    return nodes


def _validate_role_separation(root: dict[str, Any], eligible: list[str],
                              owners: list[dict[str, Any]], implementers: list[dict[str, Any]],
                              requester: dict[str, Any], subject: dict[str, Any]) -> None:
    approvers = {principal_identity(key_by_id(root, key_id)["principal"])
                 for key_id in eligible}
    blocked = {principal_identity(requester), principal_identity(subject)}
    blocked.update(principal_identity(item) for item in implementers)
    blocked.update(principal_identity(item["principal"]) for item in owners)
    if approvers & blocked:
        raise ContractError("eligible approvers violate policy role separation")


def _validate_authority(node: dict[str, Any], profile_hash: str,
                        root: dict[str, Any]) -> None:
    if (sha256(node["trust_root_sha256"], "policy.trust_root_sha256") !=
            root["root_sha256"] or node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("policy does not bind the supplied trust root")
    integer(node["trust_epoch"], "policy.trust_epoch", 1, 2**63 - 1)
    signature = validate_signature(node["signature"], "policy.signature", profile_hash)
    if signature["key_id"] != key_for_usage(root, "approval_policy_sign")["key_id"]:
        raise ContractError("policy signature uses the wrong root key usage")
