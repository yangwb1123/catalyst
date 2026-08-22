"""Signature profile and externally pinned GovernanceTrustRoot shapes."""

from __future__ import annotations

from typing import Any

from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, CONTRACT_PROFILE_ID, KEY_USAGES,
                        MAX_PROFILE_BYTES, MAX_ROOT_BYTES, PROFILE_API, PROFILE_DOMAIN,
                        ROOT_API, ROOT_DOMAIN, SIGNATURE_PROFILE_ID)
from .shape import (fixed_base64url, integer, principal_identity, require_keys, sha256,
                    text, validate_principal)

PROFILE_FIELDS = {
    "algorithm", "api_version", "canonicalization", "digest_algorithm", "kind",
    "message_preimage", "profile_id", "profile_sha256", "public_key_encoding",
    "signature_encoding",
}
ROOT_FIELDS = {
    "api_version", "canonicalization", "keys", "kind", "profile_id", "root_sha256",
    "signature_profile_sha256", "trust_domain", "trust_epoch",
}
PROFILE_CONSTANTS = {
    "algorithm": "Ed25519",
    "api_version": PROFILE_API,
    "canonicalization": CANONICALIZATION,
    "digest_algorithm": "SHA-256",
    "kind": "SignatureProfile",
    "message_preimage": "domain_separator_utf8_nul_then_raw_32_byte_sha256_digest",
    "profile_id": SIGNATURE_PROFILE_ID,
    "public_key_encoding": "base64url_unpadded_32_bytes",
    "signature_encoding": "base64url_unpadded_64_bytes",
}


def profile_sha256(value: dict[str, Any]) -> str:
    return self_digest(PROFILE_DOMAIN, value, "profile_sha256", MAX_PROFILE_BYTES,
                       "signature profile")


def validate_signature_profile(value: Any) -> dict[str, Any]:
    node = require_keys(value, "signature profile", PROFILE_FIELDS)
    bounded_canonical_json(node, MAX_PROFILE_BYTES, "signature profile")
    for field, expected in PROFILE_CONSTANTS.items():
        if node[field] != expected:
            raise ContractError(f"signature profile {field} drifted from v1")
    sha256(node["profile_sha256"], "signature profile.profile_sha256")
    if node["profile_sha256"] != profile_sha256(node):
        raise ContractError("signature profile self digest does not match")
    return node


def root_sha256(value: dict[str, Any]) -> str:
    return self_digest(ROOT_DOMAIN, value, "root_sha256", MAX_ROOT_BYTES,
                       "GovernanceTrustRoot")


def validate_trust_root(value: Any, profile_hash: str) -> dict[str, Any]:
    node = require_keys(value, "GovernanceTrustRoot", ROOT_FIELDS)
    bounded_canonical_json(node, MAX_ROOT_BYTES, "GovernanceTrustRoot")
    expected = (ROOT_API, CANONICALIZATION, "GovernanceTrustRoot", CONTRACT_PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"], node["profile_id"])
    if actual != expected:
        raise ContractError("GovernanceTrustRoot envelope drifted from v1")
    text(node["trust_domain"], "GovernanceTrustRoot.trust_domain")
    integer(node["trust_epoch"], "GovernanceTrustRoot.trust_epoch", 1, 2**63 - 1)
    if sha256(node["signature_profile_sha256"], "signature_profile_sha256") != profile_hash:
        raise ContractError("GovernanceTrustRoot does not bind the signature profile")
    _validate_root_keys(node["keys"])
    sha256(node["root_sha256"], "GovernanceTrustRoot.root_sha256")
    if node["root_sha256"] != root_sha256(node):
        raise ContractError("GovernanceTrustRoot self digest does not match")
    return node


def _validate_root_keys(value: Any) -> None:
    if not isinstance(value, list) or len(value) != 3:
        raise ContractError("GovernanceTrustRoot must contain exactly three keys")
    for index, (key, usage) in enumerate(zip(value, KEY_USAGES)):
        _validate_root_key(key, index, usage)
    key_ids = [key["key_id"] for key in value]
    principals = [principal_identity(key["principal"]) for key in value]
    public_keys = [key["public_key_base64url"] for key in value]
    if len(set(key_ids)) != 3 or len(set(principals)) != 3 or len(set(public_keys)) != 3:
        raise ContractError("root key IDs, principals, and public keys must be pairwise distinct")


def _validate_root_key(value: Any, index: int, usage: str) -> None:
    label = f"GovernanceTrustRoot.keys[{index}]"
    node = require_keys(value, label, {"key_id", "principal", "public_key_base64url", "usage"})
    text(node["key_id"], f"{label}.key_id")
    principal = validate_principal(node["principal"], f"{label}.principal")
    if node["usage"] != usage:
        raise ContractError("GovernanceTrustRoot keys must use frozen usage order")
    required_type = "agent" if usage == "request_auth" else "service"
    if principal["principal_type"] != required_type:
        raise ContractError(f"{usage} key principal must be {required_type}")
    fixed_base64url(node["public_key_base64url"], f"{label}.public_key_base64url", 32, 43)


def key_for_usage(root: dict[str, Any], usage: str) -> dict[str, Any]:
    return root["keys"][KEY_USAGES.index(usage)]
