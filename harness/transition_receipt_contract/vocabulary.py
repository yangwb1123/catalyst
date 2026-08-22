"""Exact authored Transition state vocabulary and its content identity."""

from __future__ import annotations

import copy
from typing import Any

from .canonical import ContractError, bounded_canonical_json, bounded_digest, decode_canonical
from .constants import (CANONICALIZATION, EDGE_ITEMS, MAX_VOCABULARY_BYTES,
                        REWORK_TARGETS, STATES, TERMINAL_STATES, VOCABULARY_API,
                        VOCABULARY_DOMAIN, VOCABULARY_KIND)
from .shape import require_keys, sha256

VOCABULARY_FIELDS = {
    "api_version", "canonicalization", "edges", "kind", "rework_targets", "states",
    "terminal_states", "vocabulary_sha256",
}


def _unsealed_vocabulary() -> dict[str, Any]:
    return {
        "api_version": VOCABULARY_API,
        "canonicalization": CANONICALIZATION,
        "edges": [{"allowed_to_states": list(allowed), "from_state": source}
                  for source, allowed in EDGE_ITEMS],
        "kind": VOCABULARY_KIND,
        "rework_targets": list(REWORK_TARGETS),
        "states": list(STATES),
        "terminal_states": list(TERMINAL_STATES),
        "vocabulary_sha256": "",
    }


def vocabulary_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_VOCABULARY_BYTES, "Transition state vocabulary")
    payload = copy.deepcopy(value)
    payload["vocabulary_sha256"] = ""
    return bounded_digest(VOCABULARY_DOMAIN, payload, MAX_VOCABULARY_BYTES,
                          "Transition state vocabulary digest preimage")


def transition_vocabulary() -> dict[str, Any]:
    value = _unsealed_vocabulary()
    value["vocabulary_sha256"] = vocabulary_sha256(value)
    return value


def validate_vocabulary(value: Any) -> dict[str, Any]:
    node = require_keys(value, "Transition state vocabulary", VOCABULARY_FIELDS)
    bounded_canonical_json(node, MAX_VOCABULARY_BYTES, "Transition state vocabulary")
    sha256(node["vocabulary_sha256"], "vocabulary_sha256")
    expected = transition_vocabulary()
    if node != expected:
        raise ContractError("Transition state vocabulary differs from the exact authored graph")
    if vocabulary_sha256(node) != node["vocabulary_sha256"]:
        raise ContractError("Transition state vocabulary self digest does not match")
    return node


def decode_vocabulary(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_VOCABULARY_BYTES, "Transition state vocabulary")
    return validate_vocabulary(value)

