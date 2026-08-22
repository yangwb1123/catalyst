"""One signed-shaped state image containing the complete ledger and exact view."""

from __future__ import annotations

from typing import Any

from .authority import lifecycle_key
from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, MAX_STATE_BYTES, PROFILE_ID, STATE_API,
                        STATE_DOMAIN, STATE_KEY_USAGE)
from .ledger import validate_ledger, validate_materialized_view
from .shape import integer, require_keys, sha256, validate_signature

STATE_FIELDS = {
    "api_version", "canonicalization", "kind", "ledger", "materialized_view",
    "profile_id", "signature", "state_sha256", "trust_epoch", "trust_root_sha256",
}


def state_sha256(value: dict[str, Any]) -> str:
    return self_digest(STATE_DOMAIN, value, ("state_sha256",), MAX_STATE_BYTES,
                       "ArchitectureDecisionLifecycleState", signed=True)


def validate_state(value: Any, profile_hash: str, lifecycle_root: dict[str, Any],
                   approval_root: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    label = "ArchitectureDecisionLifecycleState"
    node = require_keys(value, label, STATE_FIELDS)
    bounded_canonical_json(node, MAX_STATE_BYTES, label)
    _validate_envelope(node, profile_hash, lifecycle_root)
    ledger, rebuilt = validate_ledger(node["ledger"], profile_hash,
                                      lifecycle_root, approval_root)
    validate_materialized_view(node["materialized_view"], ledger, rebuilt)
    expected_hash = state_sha256(node)
    if sha256(node["state_sha256"], "state.state_sha256") != expected_hash:
        raise ContractError("lifecycle state image self digest does not match")
    return node, rebuilt


def _validate_envelope(node: dict[str, Any], profile_hash: str,
                       root: dict[str, Any]) -> None:
    expected = (STATE_API, CANONICALIZATION,
                "ArchitectureDecisionLifecycleState", PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError("lifecycle state image envelope drifted from v1")
    if (sha256(node["trust_root_sha256"], "state.trust_root_sha256") !=
            root["root_sha256"] or node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("lifecycle state image does not bind lifecycle root")
    integer(node["trust_epoch"], "state.trust_epoch", 1)
    key = lifecycle_key(root, STATE_KEY_USAGE)
    validate_signature(node["signature"], "state.signature", profile_hash,
                       key["key_id"])


__all__ = ["state_sha256", "validate_state"]
