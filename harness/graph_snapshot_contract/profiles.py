"""Closed identifiers, repository paths, ordering, and relative-name profiles."""

from __future__ import annotations

from governance_contract import ContractError
from local_command_observation_producer.profiles import safe_repo_path

from .constants import (
    HASH_RE, IDENTIFIER_RE, MAX_IDENTIFIER_BYTES, MAX_PATH_BYTES, MAX_PATH_SCALARS,
)


def utf8_key(value: str) -> bytes:
    return value.encode("utf-8")


def validate_identifier(value: object, label: str) -> str:
    if (not isinstance(value, str) or not value or
            len(value.encode("utf-8")) > MAX_IDENTIFIER_BYTES or
            IDENTIFIER_RE.fullmatch(value) is None):
        raise ContractError(f"{label}: invalid bounded lowercase identifier")
    return value


def validate_hash(value: object, label: str) -> str:
    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:
        raise ContractError(f"{label}: expected lowercase SHA-256")
    return value


def validate_repo_path(value: object, label: str) -> str:
    if not isinstance(value, str) or not safe_repo_path(value):
        raise ContractError(f"{label}: expected canonical repository-relative path")
    if len(value) > MAX_PATH_SCALARS or len(value.encode("utf-8")) > MAX_PATH_BYTES:
        raise ContractError(f"{label}: repository path limit exceeded")
    return value


def relative_name(value: str, module_directory: str, label: str) -> str:
    if value == module_directory:
        return "."
    if module_directory == ".":
        return value
    prefix = module_directory + "/"
    if not value.startswith(prefix):
        raise ContractError(f"{label}: value is outside selected module boundary")
    return value[len(prefix):]


def locator_key(locator: dict[str, object]):
    digest = locator["content_sha256"]
    digest_key = (0, b"") if digest is None else (1, utf8_key(digest))
    return utf8_key(locator["role"]), utf8_key(locator["path"]), digest_key


def sorted_unique(values: list[dict[str, object]], key, label: str):
    ordered = sorted(values, key=key)
    keys = [key(value) for value in ordered]
    if len(keys) != len(set(keys)):
        raise ContractError(f"{label}: derived set contains duplicate sort keys")
    return ordered
