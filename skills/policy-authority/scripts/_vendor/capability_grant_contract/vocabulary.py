"""Frozen 21-effect vocabulary validation."""

from __future__ import annotations

from typing import Any

from .canonical import ContractError, bounded_canonical_json, bounded_digest
from .constants import (CANONICALIZATION, EFFECTS, EFFECT_SPECS, VOCABULARY_API,
                        MAX_VOCABULARY_BYTES, VOCABULARY_DOMAIN, VOCABULARY_KIND,
                        VOCABULARY_SHA256)
from .shape import require_keys, sha256


def _expected_definition(effect_id: str) -> dict[str, Any]:
    allowed, required, restriction, profile = EFFECT_SPECS[effect_id]
    return {
        "allowed_scope_kinds": list(allowed),
        "effect_id": effect_id,
        "production_restriction": restriction,
        "required_scope_kinds": list(required),
        "scope_profile": profile,
    }


def vocabulary_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_VOCABULARY_BYTES, "effect vocabulary")
    payload = dict(value)
    payload["vocabulary_sha256"] = ""
    return bounded_digest(VOCABULARY_DOMAIN, payload, MAX_VOCABULARY_BYTES,
                          "effect vocabulary")


def validate_vocabulary(value: Any) -> dict[str, Any]:
    keys = {"api_version", "canonicalization", "effects", "kind", "vocabulary_sha256"}
    node = require_keys(value, "effect vocabulary", keys)
    if node["api_version"] != VOCABULARY_API or node["kind"] != VOCABULARY_KIND:
        raise ContractError("effect vocabulary envelope is not v1")
    if node["canonicalization"] != CANONICALIZATION:
        raise ContractError("effect vocabulary canonicalization is unsupported")
    bounded_canonical_json(node, MAX_VOCABULARY_BYTES, "effect vocabulary")
    effects = node["effects"]
    if not isinstance(effects, list) or len(effects) != len(EFFECTS):
        raise ContractError("effect vocabulary must contain the frozen 21 definitions")
    for index, (definition, effect_id) in enumerate(zip(effects, EFFECTS)):
        if definition != _expected_definition(effect_id):
            raise ContractError(f"effect vocabulary definition {index} drifted from v1")
    sha256(node["vocabulary_sha256"], "vocabulary_sha256")
    actual = vocabulary_sha256(node)
    if actual != VOCABULARY_SHA256 or node["vocabulary_sha256"] != VOCABULARY_SHA256:
        raise ContractError("effect vocabulary is not the frozen v1 vocabulary")
    return node
