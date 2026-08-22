"""Closed source and wire shape primitives for ADR-0069."""

from __future__ import annotations

import re

from .codec import ContractError
from .constants import MAX_ADAPTER_BYTES, MAX_IDENTIFIER_BYTES

HASH_RE = re.compile(r"[a-f0-9]{64}")
IDENTIFIER_RE = re.compile(r"[a-z0-9][a-z0-9._:/-]*")
SKILL_RE = re.compile(r"[a-z0-9][a-z0-9._-]*")
NODE_RE = re.compile(r"[0-9]{2}")
ADAPTER_RE = re.compile(r"\.agent/skills/[a-z0-9][a-z0-9._-]*\.md")


def exact_object(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ContractError(f"{label}: expected object")
    if set(value) != fields:
        raise ContractError(f"{label}: unexpected or missing fields")
    return value


def array(value: object, low: int, high: int, label: str) -> list[object]:
    if not isinstance(value, list) or not low <= len(value) <= high:
        raise ContractError(f"{label}: expected {low}..{high} array items")
    return value


def string(value: object, label: str, maximum: int = 16_384) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > maximum:
        raise ContractError(f"{label}: expected bounded nonempty string")
    return value


def integer(value: object, low: int, high: int, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{label}: expected integer in {low}..{high}")
    return value


def fixed(value: object, expected: object, label: str) -> None:
    if type(value) is not type(expected) or value != expected:
        raise ContractError(f"{label}: expected fixed value {expected!r}")


def identifier(value: object, label: str) -> str:
    text = string(value, label, MAX_IDENTIFIER_BYTES)
    if IDENTIFIER_RE.fullmatch(text) is None:
        raise ContractError(f"{label}: invalid identifier")
    return text


def node_id(value: object, label: str) -> str:
    text = string(value, label, 2)
    if NODE_RE.fullmatch(text) is None:
        raise ContractError(f"{label}: invalid two-digit node ID")
    return text


def skill_name(value: object, label: str) -> str:
    text = string(value, label, MAX_IDENTIFIER_BYTES)
    if SKILL_RE.fullmatch(text) is None or len(f".agent/skills/{text}.md".encode()) > MAX_ADAPTER_BYTES:
        raise ContractError(f"{label}: invalid bounded Skill package name")
    return text


def digest(value: object, label: str) -> str:
    text = string(value, label, 64)
    if HASH_RE.fullmatch(text) is None:
        raise ContractError(f"{label}: expected lowercase SHA-256")
    return text


def adapter_ref(value: object, label: str) -> str:
    text = string(value, label, MAX_ADAPTER_BYTES)
    if ADAPTER_RE.fullmatch(text) is None:
        raise ContractError(f"{label}: invalid logical adapter reference")
    return text


def sorted_unique(values: list[str], label: str) -> None:
    encoded = [value.encode("utf-8") for value in values]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise ContractError(f"{label}: must already be raw-UTF-8 sorted unique")
