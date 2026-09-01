"""Typed opaque Platform ID validation."""

from .codec import ContractError
from .constants import (ENTITY_PREFIXES, IDENTIFIER_INVALID,
                        PLATFORM_PREFIXES)

_CROCKFORD = set("0123456789abcdefghjkmnpqrstvwxyz")


def validate_platform_id(value: object) -> str:
    """Return one validated typed opaque Platform ID."""
    if type(value) is not str or len(value) != 30 or value[3] != "_":
        raise ContractError(
            "platform ID must have a three-byte prefix and 26-byte suffix",
            IDENTIFIER_INVALID)
    if value[:3] not in PLATFORM_PREFIXES:
        raise ContractError(
            f"platform ID prefix {value[:3]!r} is unsupported", IDENTIFIER_INVALID)
    suffix = value[4:]
    if suffix[0] not in "01234567" or any(char not in _CROCKFORD for char in suffix[1:]):
        raise ContractError(
            "platform ID suffix is not a 128-bit Crockford value", IDENTIFIER_INVALID)
    return value


def validate_typed_id(value: object, prefix: str, label: str) -> str:
    """Return one Platform ID from the required type namespace."""
    identifier = validate_platform_id(value)
    if not identifier.startswith(prefix + "_"):
        raise ContractError(
            f"{label} must use {prefix}_ namespace", IDENTIFIER_INVALID)
    return identifier


def validate_entity_id(entity_type: object, value: object, label: str) -> str:
    """Return one entity ID whose prefix agrees with entity type."""
    if type(entity_type) is not str or entity_type not in ENTITY_PREFIXES:
        raise ContractError(f"{label}.entity_type is unsupported")
    return validate_typed_id(value, ENTITY_PREFIXES[entity_type], f"{label}.entity_id")


def same_message_suffix(message_id: str, specialized_id: str) -> bool:
    """Whether common and specialized IDs identify one wire message."""
    return message_id[4:] == specialized_id[4:]
