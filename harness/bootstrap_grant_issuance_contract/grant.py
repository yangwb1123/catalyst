"""ADR-0056 CapabilityGrant overlay for the single bootstrap issuance profile."""

from __future__ import annotations

from typing import Any

from capability_grant_contract.grant import validate_grant as validate_adr0056_grant

from .authority import key_for_usage
from .canonical import ContractError, bounded_canonical_json, envelope_digest
from .constants import GRANT_ENVELOPE_DOMAIN, MAX_GRANT_BYTES, MAX_TTL_MS
from .shape import fixed_base64url, principal_identity


def grant_envelope_sha256(value: dict[str, Any]) -> str:
    return envelope_digest(GRANT_ENVELOPE_DOMAIN, value, MAX_GRANT_BYTES,
                           "complete CapabilityGrant envelope")


def validate_bootstrap_grant(value: Any, profile_hash: str,
                             root: dict[str, Any]) -> dict[str, Any]:
    node = validate_adr0056_grant(value)
    bounded_canonical_json(node, MAX_GRANT_BYTES, "bootstrap CapabilityGrant")
    if node["issuance_phase"] != "bootstrap_planning":
        raise ContractError("bootstrap Grant issuance_phase must be bootstrap_planning")
    if node["approval_refs"] != []:
        raise ContractError("bootstrap Grant approval_refs must be empty")
    bindings = node["bindings"]
    if any(bindings[field] is not None for field in ("impact_sha256", "plan_sha256",
                                                     "risk_sha256")):
        raise ContractError("bootstrap Grant impact, plan, and risk bindings must be null")
    _validate_grant_proof(node, profile_hash, root)
    _validate_usage_and_separation(node, root)
    return node


def _validate_grant_proof(node: dict[str, Any], profile_hash: str,
                          root: dict[str, Any]) -> None:
    proof = node["authority_proof"]
    issue_key = key_for_usage(root, "grant_issue")
    if proof["key_id"] != issue_key["key_id"]:
        raise ContractError("Grant proof does not use grant_issue key")
    if proof["proof_profile_id"] != "forgeos.ed25519-domain-sha256/v1":
        raise ContractError("Grant proof profile ID drifted")
    if proof["proof_profile_sha256"] != profile_hash:
        raise ContractError("Grant proof does not bind signature profile")
    if proof["trust_domain"] != root["trust_domain"] or proof["trust_epoch"] != root["trust_epoch"]:
        raise ContractError("Grant proof does not bind GovernanceTrustRoot")
    issuer = proof["issuer"]
    if issuer["authority_class"] != "forgeos_kernel":
        raise ContractError("bootstrap Grant issuer must be forgeos_kernel")
    expected = dict(issue_key["principal"])
    expected["authority_class"] = "forgeos_kernel"
    if issuer != expected:
        raise ContractError("Grant issuer differs from grant_issue principal")
    fixed_base64url(proof["proof_base64url"], "Grant proof_base64url", 64, 86)


def _validate_usage_and_separation(node: dict[str, Any], root: dict[str, Any]) -> None:
    expected_usage = {
        "atomic_reservation_required": True,
        "concurrent_use": "forbidden",
        "consumption_mode": "single_use",
        "replay": "receipt_only_no_reexecute",
        "uncertain_effect": "quarantine",
        "usage_ledger_required": True,
    }
    if node["usage_policy"] != expected_usage:
        raise ContractError("bootstrap Grant usage policy drifted")
    separation = node["separation_of_duty"]
    request_key = key_for_usage(root, "request_auth")
    expected_distinctions = ["issuer_not_requester", "issuer_not_subject"]
    if (separation["requester"] != request_key["principal"] or
            separation["required_distinctions"] != expected_distinctions):
        raise ContractError("bootstrap Grant separation-of-duty declarations drifted")
    if principal_identity(node["subject"]) == principal_identity(
            key_for_usage(root, "grant_issue")["principal"]):
        raise ContractError("grant_issue principal must differ from Grant subject")


def validate_grant_relations(grant: dict[str, Any], policy: dict[str, Any],
                             request: dict[str, Any], receipt: dict[str, Any]) -> None:
    if receipt["decision"] == "denied":
        raise ContractError("denied issuance cannot contain a Grant")
    exact = ("budget", "capability", "scope", "subject", "task_binding")
    if any(grant[field] != request[field] for field in exact):
        raise ContractError("issued Grant differs from exact Request fields")
    bindings = grant["bindings"]
    expected = {
        "context_sha256": request["bindings"]["context_sha256"],
        "grant_request_sha256": request["request_sha256"],
        "impact_sha256": None,
        "plan_sha256": None,
        "policy_sha256": policy["policy_sha256"],
        "risk_sha256": None,
        "source_revision": request["bindings"]["source_revision"],
        "source_tree_sha256": request["bindings"]["source_tree_sha256"],
    }
    if bindings != expected:
        raise ContractError("issued Grant bindings differ from Policy or Request")
    _validate_grant_validity(grant, policy, request, receipt)
    if (receipt["grant_id"] != grant["grant_id"] or
            receipt["grant_sha256"] != grant["grant_sha256"] or
            receipt["grant_envelope_sha256"] != grant_envelope_sha256(grant)):
        raise ContractError("receipt does not bind the complete issued Grant envelope")


def _validate_grant_validity(grant: dict[str, Any], policy: dict[str, Any],
                             request: dict[str, Any], receipt: dict[str, Any]) -> None:
    validity = grant["validity"]
    stored = receipt["stored_at_unix_ms"]
    if (validity["issued_at_unix_ms"] != stored or
            validity["not_before_unix_ms"] != stored or
            validity["expires_at_unix_ms"] != stored + request["requested_ttl_ms"] or
            validity["transferable"] is not False):
        raise ContractError("Grant validity does not equal durable decision time plus requested TTL")
    if validity["expires_at_unix_ms"] - validity["issued_at_unix_ms"] > MAX_TTL_MS:
        raise ContractError("Grant validity exceeds one hour")
    if validity["expires_at_unix_ms"] > policy["validity"]["expires_at_unix_ms"]:
        raise ContractError("Grant validity exceeds signed Policy validity")
