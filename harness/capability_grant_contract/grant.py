"""CapabilityGrant envelope validation without authority interpretation."""

from __future__ import annotations

from typing import Any

from .canonical import ContractError, bounded_canonical_json, bounded_digest
from .constants import (CANONICALIZATION, EFFECT_SPECS, GRANT_API, GRANT_DOMAIN, GRANT_KIND,
                        MAX_GRANT_BYTES, MAX_TTL_MS, VOCABULARY_SHA256)
from .scope import validate_scope
from .shape import (array, boolean, enum, integer, require_keys, sha256, sorted_unique_nodes,
                    sorted_unique_strings, text, validate_base64url, validate_bindings,
                    validate_budget, validate_capability, validate_principal,
                    validate_task_binding)

GRANT_FIELDS = {
    "api_version", "approval_refs", "authority_proof", "bindings", "budget",
    "canonicalization", "capability", "effect_vocabulary_sha256", "grant_id",
    "grant_sha256", "issuance_phase", "kind", "scope", "separation_of_duty",
    "subject", "task_binding", "usage_policy", "validity",
}
DISTINCTIONS = (
    "approver_not_issuer", "approver_not_requester", "approver_not_subject",
    "issuer_not_requester", "issuer_not_subject",
)


def _validate_issuer(value: Any) -> None:
    keys = {"authority_class", "authority_domain", "principal_id", "principal_type"}
    node = require_keys(value, "authority_proof.issuer", keys)
    authority_class = enum(node["authority_class"], "authority_proof.issuer.authority_class",
                           ("external_operator", "forgeos_kernel"))
    text(node["authority_domain"], "authority_proof.issuer.authority_domain")
    text(node["principal_id"], "authority_proof.issuer.principal_id")
    principal_type = enum(node["principal_type"], "authority_proof.issuer.principal_type",
                          ("agent", "human", "operator", "service"))
    if authority_class == "forgeos_kernel" and principal_type != "service":
        raise ContractError("a declared forgeos_kernel issuer must have service principal_type")
    if authority_class == "external_operator" and principal_type not in ("human", "operator"):
        raise ContractError("a declared external_operator issuer must be human or operator")


def _validate_authority_proof(value: Any) -> None:
    keys = {"issuer", "key_id", "proof_base64url", "proof_profile_id",
            "proof_profile_sha256", "trust_domain", "trust_epoch"}
    node = require_keys(value, "authority_proof", keys)
    _validate_issuer(node["issuer"])
    text(node["key_id"], "authority_proof.key_id")
    validate_base64url(node["proof_base64url"], "authority_proof.proof_base64url")
    text(node["proof_profile_id"], "authority_proof.proof_profile_id")
    sha256(node["proof_profile_sha256"], "authority_proof.proof_profile_sha256")
    text(node["trust_domain"], "authority_proof.trust_domain")
    integer(node["trust_epoch"], "authority_proof.trust_epoch", 0, 2**63 - 1)


def _validate_approvals(value: Any) -> None:
    approvals = array(value, "approval_refs", 0, 32)
    keys = {"approval_id", "approval_sha256", "authority_domain"}
    for index, approval in enumerate(approvals):
        node = require_keys(approval, f"approval_refs[{index}]", keys)
        text(node["approval_id"], f"approval_refs[{index}].approval_id")
        sha256(node["approval_sha256"], f"approval_refs[{index}].approval_sha256")
        text(node["authority_domain"], f"approval_refs[{index}].authority_domain")
    sorted_unique_nodes(approvals, "approval_refs")


def _identity(value: dict[str, Any]) -> tuple[Any, ...]:
    return value["authority_domain"], value["principal_id"], value["principal_type"]


def _validate_separation(value: Any, grant: dict[str, Any]) -> None:
    node = require_keys(value, "separation_of_duty", {"requester", "required_distinctions"})
    validate_principal(node["requester"], "separation_of_duty.requester")
    distinctions = sorted_unique_strings(node["required_distinctions"],
                                          "required_distinctions", DISTINCTIONS)
    issuer = grant["authority_proof"]["issuer"]
    if "issuer_not_requester" in distinctions and _identity(issuer) == _identity(node["requester"]):
        raise ContractError("issuer_not_requester is contradicted by declared identities")
    if "issuer_not_subject" in distinctions and _identity(issuer) == _identity(grant["subject"]):
        raise ContractError("issuer_not_subject is contradicted by declared identities")


def _validate_usage_policy(value: Any, budget: dict[str, Any]) -> None:
    keys = {"atomic_reservation_required", "concurrent_use", "consumption_mode", "replay",
            "uncertain_effect", "usage_ledger_required"}
    node = require_keys(value, "usage_policy", keys)
    if boolean(node["atomic_reservation_required"], "atomic_reservation_required") is not True:
        raise ContractError("atomic_reservation_required must be true")
    if boolean(node["usage_ledger_required"], "usage_ledger_required") is not True:
        raise ContractError("usage_ledger_required must be true")
    enum(node["concurrent_use"], "concurrent_use", ("forbidden",))
    mode = enum(node["consumption_mode"], "consumption_mode", ("bounded_calls", "single_use"))
    enum(node["replay"], "replay", ("receipt_only_no_reexecute",))
    enum(node["uncertain_effect"], "uncertain_effect", ("quarantine",))
    if mode == "single_use" and budget["max_calls"] != 1:
        raise ContractError("single_use requires max_calls=1")


def _validate_validity(value: Any) -> None:
    keys = {"expires_at_unix_ms", "issued_at_unix_ms", "not_before_unix_ms", "transferable"}
    node = require_keys(value, "validity", keys)
    issued = integer(node["issued_at_unix_ms"], "issued_at_unix_ms", 0, 2**63 - 1)
    not_before = integer(node["not_before_unix_ms"], "not_before_unix_ms", 0, 2**63 - 1)
    expires = integer(node["expires_at_unix_ms"], "expires_at_unix_ms", 0, 2**63 - 1)
    if not issued <= not_before < expires or expires - issued > MAX_TTL_MS:
        raise ContractError("validity must be ordered within the 24-hour maximum window")
    if boolean(node["transferable"], "validity.transferable") is not False:
        raise ContractError("CapabilityGrant must be non-transferable")


def grant_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_GRANT_BYTES, "CapabilityGrant")
    payload = {key: child for key, child in value.items()}
    payload["authority_proof"] = dict(value["authority_proof"])
    payload["authority_proof"]["proof_base64url"] = ""
    payload["grant_id"] = ""
    payload["grant_sha256"] = ""
    return bounded_digest(GRANT_DOMAIN, payload, MAX_GRANT_BYTES, "CapabilityGrant")


def _validate_identity_and_digest(node: dict[str, Any]) -> None:
    text(node["grant_id"], "grant_id", 256)
    sha256(node["grant_sha256"], "grant_sha256")
    actual = grant_sha256(node)
    if node["grant_sha256"] != actual or node["grant_id"] != f"capability-grant-{actual}":
        raise ContractError("CapabilityGrant identity or self digest does not match")


def validate_grant(value: Any) -> dict[str, Any]:
    node = require_keys(value, "CapabilityGrant", GRANT_FIELDS)
    if node["api_version"] != GRANT_API or node["canonicalization"] != CANONICALIZATION:
        raise ContractError("CapabilityGrant API or canonicalization is unsupported")
    if node["kind"] != GRANT_KIND:
        raise ContractError("kind must be CapabilityGrant; aliases are rejected")
    if node["effect_vocabulary_sha256"] != VOCABULARY_SHA256:
        raise ContractError("CapabilityGrant does not bind the frozen effect vocabulary")
    bounded_canonical_json(node, MAX_GRANT_BYTES, "CapabilityGrant")
    _validate_approvals(node["approval_refs"])
    _validate_authority_proof(node["authority_proof"])
    validate_bindings(node["bindings"])
    validate_budget(node["budget"])
    validate_capability(node["capability"])
    enum(node["issuance_phase"], "issuance_phase", ("bootstrap_planning", "plan_finalization"))
    validate_scope(node["scope"])
    validate_principal(node["subject"], "subject")
    validate_task_binding(node["task_binding"])
    _validate_separation(node["separation_of_duty"], node)
    _validate_usage_policy(node["usage_policy"], node["budget"])
    _validate_validity(node["validity"])
    _validate_phase_and_restriction(node)
    _validate_identity_and_digest(node)
    return node


def _validate_phase_and_restriction(node: dict[str, Any]) -> None:
    if node["issuance_phase"] == "plan_finalization":
        if any(node["bindings"][field] is None for field in ("impact_sha256", "plan_sha256",
                                                               "risk_sha256")):
            raise ContractError("plan_finalization requires impact, plan, and risk bindings")
    effect_id = node["scope"]["effect_id"]
    restriction = EFFECT_SPECS[effect_id][2]
    issuer_class = node["authority_proof"]["issuer"]["authority_class"]
    production = any(resource.get("environment_class") == "production"
                     for clause in node["scope"]["allow"]
                     for resource in clause["resources"])
    if restriction == "external_operator_only" and production:
        if issuer_class != "external_operator" or not node["approval_refs"]:
            raise ContractError("production apply/execute requires external operator and approval")
