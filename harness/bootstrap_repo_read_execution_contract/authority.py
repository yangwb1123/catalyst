"""Independent externally pinned execution-root structural contract."""

from __future__ import annotations

from typing import Any

from bootstrap_grant_issuance_contract.authority import validate_signature_profile

from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, KEY_USAGES, MAX_ROOT_BYTES, PROFILE_ID,
                        ROOT_API, ROOT_DOMAIN)
from .shape import (fixed_base64url, integer, principal_identity, require_keys, sha256,
                    text, validate_principal)

FIELDS = {
    "api_version", "canonicalization", "issuance_trust_epoch",
    "issuance_trust_root_sha256", "keys", "kind", "profile_id", "root_sha256",
    "signature_profile_sha256", "trust_domain", "trust_epoch",
}
KEY_FIELDS = {"key_id", "principal", "public_key_base64url", "usage"}


def root_sha256(value: dict[str, Any]) -> str:
    return self_digest(ROOT_DOMAIN, value, "root_sha256", MAX_ROOT_BYTES,
                       "BootstrapRepoReadExecutionTrustRoot")


def validate_execution_root(value: Any, profile_hash: str,
                            issuance_root: dict[str, Any]) -> dict[str, Any]:
    node = require_keys(value, "BootstrapRepoReadExecutionTrustRoot", FIELDS)
    bounded_canonical_json(node, MAX_ROOT_BYTES, "BootstrapRepoReadExecutionTrustRoot")
    expected = (ROOT_API, CANONICALIZATION, "BootstrapRepoReadExecutionTrustRoot", PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"], node["profile_id"])
    if actual != expected:
        raise ContractError("execution trust root envelope drifted from v1")
    _validate_root_identity(node, profile_hash, issuance_root)
    keys = _validate_keys(node["keys"])
    issuance_public = {key["public_key_base64url"] for key in issuance_root["keys"]}
    if any(key["public_key_base64url"] in issuance_public for key in keys):
        raise ContractError("execution and issuance trust roots must not reuse public keys")
    sha256(node["root_sha256"], "execution_root.root_sha256")
    if node["root_sha256"] != root_sha256(node):
        raise ContractError("execution trust root self digest does not match")
    return node


def _validate_root_identity(node: dict[str, Any], profile_hash: str,
                            issuance_root: dict[str, Any]) -> None:
    text(node["trust_domain"], "execution_root.trust_domain")
    integer(node["trust_epoch"], "execution_root.trust_epoch", 1, 2**63 - 1)
    integer(node["issuance_trust_epoch"], "execution_root.issuance_trust_epoch",
            1, 2**63 - 1)
    sha256(node["issuance_trust_root_sha256"], "issuance_trust_root_sha256")
    sha256(node["signature_profile_sha256"], "signature_profile_sha256")
    if node["signature_profile_sha256"] != profile_hash:
        raise ContractError("execution trust root does not bind the signature profile")
    if (node["issuance_trust_root_sha256"] != issuance_root["root_sha256"] or
            node["issuance_trust_epoch"] != issuance_root["trust_epoch"]):
        raise ContractError("execution trust root does not bind the pinned issuance root")


def _validate_keys(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list) or len(value) != 3:
        raise ContractError("execution trust root must contain exactly three keys")
    keys = [_validate_key(item, index, usage)
            for index, (item, usage) in enumerate(zip(value, KEY_USAGES))]
    identities = ([key["key_id"] for key in keys],
                  [principal_identity(key["principal"]) for key in keys],
                  [key["public_key_base64url"] for key in keys])
    if any(len(set(values)) != 3 for values in identities):
        raise ContractError("execution key IDs, principals, and public keys must be distinct")
    return keys


def _validate_key(value: Any, index: int, usage: str) -> dict[str, Any]:
    label = f"execution_root.keys[{index}]"
    node = require_keys(value, label, KEY_FIELDS)
    text(node["key_id"], f"{label}.key_id")
    principal = validate_principal(node["principal"], f"{label}.principal")
    if node["usage"] != usage:
        raise ContractError("execution root keys must use the frozen usage order")
    expected_type = "agent" if usage == "execution_request_auth" else "service"
    if principal["principal_type"] != expected_type:
        raise ContractError(f"{usage} principal must be {expected_type}")
    fixed_base64url(node["public_key_base64url"], f"{label}.public_key_base64url", 32, 43)
    return node


def key_for_usage(root: dict[str, Any], usage: str) -> dict[str, Any]:
    return root["keys"][KEY_USAGES.index(usage)]


__all__ = [
    "key_for_usage", "root_sha256", "validate_execution_root",
    "validate_signature_profile",
]
