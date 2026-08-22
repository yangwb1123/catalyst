"""Proof-shaped lifecycle authority roots; no signature is verified here."""

from __future__ import annotations

from typing import Any

from authenticated_adr_approval_contract.authority import (
    key_for_usage as approval_key_for_usage,
    validate_signature_profile,
    validate_trust_root as validate_approval_trust_root,
)
from authenticated_adr_approval_contract.shape import (
    fixed_base64url,
    principal_identity,
    validate_principal,
)

from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (
    CANONICALIZATION,
    KEY_USAGES,
    MAX_ROOT_BYTES,
    PROFILE_ID,
    REQUEST_KEY_USAGE,
    STATE_KEY_USAGE,
    TRUST_ROOT_API,
    TRUST_ROOT_DOMAIN,
)
from .shape import integer, require_keys, sha256, text

ROOT_FIELDS = {
    "api_version", "canonicalization", "keys", "kind", "profile_id",
    "root_sha256", "signature_profile_sha256", "trust_domain", "trust_epoch",
}
KEY_FIELDS = {"key_id", "principal", "public_key_base64url", "usage"}


def trust_root_sha256(value: dict[str, Any]) -> str:
    return self_digest(TRUST_ROOT_DOMAIN, value, ("root_sha256",),
                       MAX_ROOT_BYTES, "ArchitectureDecisionLifecycleTrustRoot")


def validate_lifecycle_trust_root(value: Any, profile_hash: str) -> dict[str, Any]:
    label = "ArchitectureDecisionLifecycleTrustRoot"
    node = require_keys(value, label, ROOT_FIELDS)
    bounded_canonical_json(node, MAX_ROOT_BYTES, label)
    expected = (TRUST_ROOT_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    text(node["trust_domain"], f"{label}.trust_domain")
    integer(node["trust_epoch"], f"{label}.trust_epoch", 1)
    if sha256(node["signature_profile_sha256"],
              f"{label}.signature_profile_sha256") != profile_hash:
        raise ContractError(f"{label} does not bind the SignatureProfile")
    _validate_keys(node["keys"], node["trust_domain"])
    if sha256(node["root_sha256"], f"{label}.root_sha256") != trust_root_sha256(node):
        raise ContractError(f"{label} self digest does not match")
    return node


def _validate_keys(value: Any, trust_domain: str) -> None:
    if not isinstance(value, list) or len(value) != 2:
        raise ContractError("lifecycle trust root requires exactly two keys")
    encoded = [bounded_canonical_json(item, MAX_ROOT_BYTES, "lifecycle root key")
               for item in value]
    if encoded != sorted(set(encoded)):
        raise ContractError("lifecycle root keys must be canonical-byte sorted and unique")
    keys = [_validate_key(item, index) for index, item in enumerate(value)]
    if sorted(key["usage"] for key in keys) != sorted(KEY_USAGES):
        raise ContractError("lifecycle trust root requires one key for each exact usage")
    if (len({key["key_id"] for key in keys}) != 2 or
            len({key["public_key_base64url"] for key in keys}) != 2 or
            len({principal_identity(key["principal"]) for key in keys}) != 2):
        raise ContractError("lifecycle key IDs, public keys, and principals must differ")
    if any(key["principal"]["authority_domain"] != trust_domain for key in keys):
        raise ContractError("lifecycle key principals must use the root trust domain")


def validate_independent_roots(lifecycle_root: dict[str, Any],
                               approval_root: dict[str, Any]) -> None:
    if (lifecycle_root["root_sha256"] == approval_root["root_sha256"] or
            lifecycle_root["trust_domain"] == approval_root["trust_domain"]):
        raise ContractError("approval and lifecycle roots/domains must be independent")
    for field in ("key_id", "public_key_base64url"):
        left = {key[field] for key in lifecycle_root["keys"]}
        right = {key[field] for key in approval_root["keys"]}
        if left & right:
            raise ContractError(f"approval and lifecycle roots reuse {field}")
    left_principals = {principal_identity(key["principal"])
                       for key in lifecycle_root["keys"]}
    right_principals = {principal_identity(key["principal"])
                        for key in approval_root["keys"]}
    if left_principals & right_principals:
        raise ContractError("approval and lifecycle roots reuse a principal")


def _validate_key(value: Any, index: int) -> dict[str, Any]:
    label = f"lifecycle_trust_root.keys[{index}]"
    node = require_keys(value, label, KEY_FIELDS)
    text(node["key_id"], f"{label}.key_id")
    if node["usage"] not in KEY_USAGES:
        raise ContractError(f"{label}.usage is unsupported")
    allowed = (("agent", "human", "operator", "service")
               if node["usage"] == REQUEST_KEY_USAGE else ("service",))
    validate_principal(node["principal"], f"{label}.principal", allowed)
    fixed_base64url(node["public_key_base64url"],
                    f"{label}.public_key_base64url", 32)
    return node


def lifecycle_key(root: dict[str, Any], usage: str) -> dict[str, Any]:
    if usage not in KEY_USAGES:
        raise ContractError(f"unsupported lifecycle key usage {usage!r}")
    matches = [key for key in root["keys"] if key["usage"] == usage]
    if len(matches) != 1:
        raise ContractError(f"lifecycle trust root lacks exactly one {usage} key")
    return matches[0]


__all__ = [
    "approval_key_for_usage", "lifecycle_key", "trust_root_sha256",
    "validate_independent_roots",
    "validate_approval_trust_root", "validate_lifecycle_trust_root",
    "validate_signature_profile",
]
