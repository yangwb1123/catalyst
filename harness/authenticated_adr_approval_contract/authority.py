"""Signature profile and externally pinned independent trust-root shapes."""

from __future__ import annotations

from typing import Any

from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (
    CANONICALIZATION,
    KEY_USAGES,
    MAX_PROFILE_BYTES,
    MAX_ROOT_BYTES,
    PROFILE_ID,
    SIGNATURE_PROFILE_API,
    SIGNATURE_PROFILE_DOMAIN,
    SIGNATURE_PROFILE_ID,
    TRUST_ROOT_API,
    TRUST_ROOT_DOMAIN,
)
from .shape import (fixed_base64url, integer, principal_identity, require_keys,
                    sha256, sorted_unique_nodes, text, validate_principal)

SIGNATURE_PROFILE_FIELDS = {
    "algorithm", "api_version", "canonicalization", "digest_algorithm", "kind",
    "message_preimage", "profile_id", "profile_sha256", "public_key_encoding",
    "signature_encoding",
}
TRUST_ROOT_FIELDS = {
    "api_version", "canonicalization", "keys", "kind", "profile_id", "root_sha256",
    "signature_profile_sha256", "trust_domain", "trust_epoch",
}
KEY_FIELDS = {"key_id", "principal", "public_key_base64url", "usage"}
PROFILE_CONSTANTS = {
    "algorithm": "Ed25519",
    "api_version": SIGNATURE_PROFILE_API,
    "canonicalization": CANONICALIZATION,
    "digest_algorithm": "SHA-256",
    "kind": "SignatureProfile",
    "message_preimage": "domain_separator_utf8_nul_then_raw_32_byte_sha256_digest",
    "profile_id": SIGNATURE_PROFILE_ID,
    "public_key_encoding": "base64url_unpadded_32_bytes",
    "signature_encoding": "base64url_unpadded_64_bytes",
}


def signature_profile_sha256(value: dict[str, Any]) -> str:
    return self_digest(SIGNATURE_PROFILE_DOMAIN, value, ("profile_sha256",),
                       MAX_PROFILE_BYTES, "SignatureProfile")


def validate_signature_profile(value: Any) -> dict[str, Any]:
    node = require_keys(value, "SignatureProfile", SIGNATURE_PROFILE_FIELDS)
    bounded_canonical_json(node, MAX_PROFILE_BYTES, "SignatureProfile")
    for field, expected in PROFILE_CONSTANTS.items():
        if node[field] != expected:
            raise ContractError(f"SignatureProfile.{field} drifted from v1")
    sha256(node["profile_sha256"], "SignatureProfile.profile_sha256")
    if node["profile_sha256"] != signature_profile_sha256(node):
        raise ContractError("SignatureProfile self digest does not match")
    return node


def trust_root_sha256(value: dict[str, Any]) -> str:
    return self_digest(TRUST_ROOT_DOMAIN, value, ("root_sha256",), MAX_ROOT_BYTES,
                       "ArchitectureDecisionApprovalTrustRoot")


def validate_trust_root(value: Any, profile_hash: str) -> dict[str, Any]:
    label = "ArchitectureDecisionApprovalTrustRoot"
    node = require_keys(value, label, TRUST_ROOT_FIELDS)
    bounded_canonical_json(node, MAX_ROOT_BYTES, label)
    expected = (TRUST_ROOT_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    text(node["trust_domain"], f"{label}.trust_domain")
    integer(node["trust_epoch"], f"{label}.trust_epoch", 1, 2**63 - 1)
    if sha256(node["signature_profile_sha256"], "signature_profile_sha256") != profile_hash:
        raise ContractError(f"{label} does not bind the SignatureProfile")
    _validate_keys(node["keys"])
    sha256(node["root_sha256"], f"{label}.root_sha256")
    if node["root_sha256"] != trust_root_sha256(node):
        raise ContractError(f"{label} self digest does not match")
    return node


def _validate_keys(value: Any) -> None:
    keys = sorted_unique_nodes(value, "trust_root.keys", 6, 20)
    for index, key in enumerate(keys):
        _validate_key(key, f"trust_root.keys[{index}]")
    usages = [key["usage"] for key in keys]
    expected_counts = {
        "approval_authorization_state_sign": 1,
        "approval_policy_sign": 1,
        "approval_request_auth": 1,
        "approval_revocation_sign": 1,
    }
    if any(usages.count(usage) != count for usage, count in expected_counts.items()):
        raise ContractError("trust root service/request key usage counts drifted from v1")
    if not 2 <= usages.count("architecture_approval_sign") <= 16:
        raise ContractError("trust root requires 2..16 architecture approval keys")
    identities = [principal_identity(key["principal"]) for key in keys]
    if (len({key["key_id"] for key in keys}) != len(keys) or
            len(set(identities)) != len(keys) or
            len({key["public_key_base64url"] for key in keys}) != len(keys)):
        raise ContractError("root key IDs, principals, and public keys must be pairwise distinct")


def _validate_key(value: Any, label: str) -> None:
    node = require_keys(value, label, KEY_FIELDS)
    text(node["key_id"], f"{label}.key_id")
    if node["usage"] not in KEY_USAGES:
        raise ContractError(f"{label}.usage is unsupported")
    allowed = (("human", "operator") if node["usage"] == "architecture_approval_sign"
               else ("agent", "human", "operator", "service")
               if node["usage"] == "approval_request_auth" else ("service",))
    validate_principal(node["principal"], f"{label}.principal", allowed)
    fixed_base64url(node["public_key_base64url"], f"{label}.public_key_base64url", 32)


def keys_for_usage(root: dict[str, Any], usage: str) -> list[dict[str, Any]]:
    if usage not in KEY_USAGES:
        raise ContractError(f"unsupported trust-root key usage {usage!r}")
    return [key for key in root["keys"] if key["usage"] == usage]


def key_for_usage(root: dict[str, Any], usage: str) -> dict[str, Any]:
    matches = keys_for_usage(root, usage)
    if len(matches) != 1:
        raise ContractError(f"trust root does not contain one {usage} key")
    return matches[0]


def key_by_id(root: dict[str, Any], key_id: str) -> dict[str, Any]:
    matches = [key for key in root["keys"] if key["key_id"] == key_id]
    if len(matches) != 1:
        raise ContractError(f"key {key_id!r} is not unique in trust root")
    return matches[0]
