"""Signed revocation snapshot shape and complete in-ledger chain relations."""

from __future__ import annotations

import re
from typing import Any

from .authority import key_by_id, key_for_usage
from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, MAX_POLICY_VALIDITY_MS,
                        MAX_REVOCATION_BYTES, MAX_REVOCATION_SNAPSHOTS, PROFILE_ID,
                        REVOCATION_API, REVOCATION_DOMAIN)
from .shape import (array, integer, require_keys, sha256,
                    sorted_unique_strings, validate_signature)

REVOCATION_FIELDS = {
    "api_version", "canonicalization", "effective_at_unix_ms", "expires_at_unix_ms",
    "kind", "prior_revocation_sha256", "profile_id", "revocation_sequence",
    "revocation_sha256", "revoked_approval_ids", "revoked_key_ids", "signature",
    "trust_epoch", "trust_root_sha256",
}


def revocation_sha256(value: dict[str, Any]) -> str:
    return self_digest(REVOCATION_DOMAIN, value, ("revocation_sha256",),
                       MAX_REVOCATION_BYTES,
                       "ArchitectureDecisionApprovalRevocationSnapshot", signed=True)


def validate_revocation(value: Any, profile_hash: str,
                        root: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionApprovalRevocationSnapshot"
    node = require_keys(value, label, REVOCATION_FIELDS)
    bounded_canonical_json(node, MAX_REVOCATION_BYTES, label)
    expected = (REVOCATION_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    sequence = integer(node["revocation_sequence"], "revocation.revocation_sequence",
                       1, 2**63 - 1)
    _validate_time(node)
    _validate_revoked_values(node, root)
    _validate_authority(node, profile_hash, root)
    if sequence == 1 and node["prior_revocation_sha256"] is not None:
        raise ContractError("first revocation snapshot must have null prior digest")
    if sequence > 1:
        sha256(node["prior_revocation_sha256"], "revocation.prior_revocation_sha256")
    sha256(node["revocation_sha256"], "revocation.revocation_sha256")
    if node["revocation_sha256"] != revocation_sha256(node):
        raise ContractError("revocation snapshot self digest does not match")
    return node


def _validate_time(node: dict[str, Any]) -> None:
    effective = integer(node["effective_at_unix_ms"], "revocation.effective_at_unix_ms",
                        0, 2**63 - 1)
    expires = integer(node["expires_at_unix_ms"], "revocation.expires_at_unix_ms",
                      0, 2**63 - 1)
    if not effective < expires or expires - effective > MAX_POLICY_VALIDITY_MS:
        raise ContractError("revocation snapshot validity must be ordered within 24 hours")


def _validate_revoked_values(node: dict[str, Any], root: dict[str, Any]) -> None:
    approvals = sorted_unique_strings(node["revoked_approval_ids"],
                                      "revocation.revoked_approval_ids", 0, 256)
    for approval_id in approvals:
        if re.fullmatch(r"approval-record-[0-9a-f]{64}", approval_id) is None:
            raise ContractError("revoked approval ID is malformed")
    key_ids = sorted_unique_strings(node["revoked_key_ids"],
                                    "revocation.revoked_key_ids", 0, 20)
    for key_id in key_ids:
        key_by_id(root, key_id)


def _validate_authority(node: dict[str, Any], profile_hash: str,
                        root: dict[str, Any]) -> None:
    if (sha256(node["trust_root_sha256"], "revocation.trust_root_sha256") !=
            root["root_sha256"] or node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("revocation snapshot does not bind the trust root")
    integer(node["trust_epoch"], "revocation.trust_epoch", 1, 2**63 - 1)
    signature = validate_signature(node["signature"], "revocation.signature", profile_hash)
    expected = key_for_usage(root, "approval_revocation_sign")["key_id"]
    if signature["key_id"] != expected:
        raise ContractError("revocation signature uses the wrong root key usage")
    if expected in node["revoked_key_ids"]:
        raise ContractError("revocation snapshot cannot revoke its own signing key")


def validate_revocation_chain(value: Any, profile_hash: str,
                              root: dict[str, Any]) -> list[dict[str, Any]]:
    snapshots = array(value, "ledger.revocation_snapshots", 1,
                      MAX_REVOCATION_SNAPSHOTS)
    prior = None
    prior_approvals: set[str] = set()
    prior_keys: set[str] = set()
    prior_effective = -1
    for index, snapshot in enumerate(snapshots):
        node = validate_revocation(snapshot, profile_hash, root)
        if node["revocation_sequence"] != index + 1:
            raise ContractError("revocation sequence must start at one and be contiguous")
        if node["prior_revocation_sha256"] != prior:
            raise ContractError("revocation prior digest chain is not contiguous")
        if node["effective_at_unix_ms"] < prior_effective:
            raise ContractError("revocation effective times must be nondecreasing")
        approvals, keys = set(node["revoked_approval_ids"]), set(node["revoked_key_ids"])
        if not prior_approvals <= approvals or not prior_keys <= keys:
            raise ContractError("revocation sets must be monotonic within one root epoch")
        prior, prior_approvals, prior_keys = node["revocation_sha256"], approvals, keys
        prior_effective = node["effective_at_unix_ms"]
    return snapshots
