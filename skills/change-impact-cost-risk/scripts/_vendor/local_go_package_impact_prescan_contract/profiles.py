"""Closed lexical input profiles for ADR-0062."""

from __future__ import annotations

from governance_contract import ContractError
from local_command_observation_producer.profiles import safe_repo_path

from .constants import (MAX_CHANGED_PATHS, MAX_PATH_BYTES, MAX_PATH_SCALARS,
                        MAX_RUN_ID_BYTES, RUN_ID_RE)


def utf8_key(value: str) -> bytes:
    return value.encode("utf-8")


def validate_repo_path(value: object, label: str) -> str:
    if not isinstance(value, str) or not safe_repo_path(value):
        raise ContractError(f"{label}: expected canonical repository-relative path")
    try:
        encoded = value.encode("utf-8")
    except UnicodeError as error:
        raise ContractError(f"{label}: expected valid UTF-8: {error}") from error
    if len(value) > MAX_PATH_SCALARS or len(encoded) > MAX_PATH_BYTES:
        raise ContractError(f"{label}: path length limit exceeded")
    return value


def validate_changed_paths(value: object) -> list[str]:
    if not isinstance(value, list) or not 1 <= len(value) <= MAX_CHANGED_PATHS:
        raise ContractError("request.changed_paths: expected 1..256 paths")
    paths = [validate_repo_path(item, f"request.changed_paths[{index}]")
             for index, item in enumerate(value)]
    if paths != sorted(set(paths), key=utf8_key):
        raise ContractError(
            "request.changed_paths: must be strictly UTF-8-byte sorted and unique")
    return paths


def validate_run_id(value: object, label: str = "request.run_id") -> str:
    if (not isinstance(value, str) or len(value.encode("utf-8")) > MAX_RUN_ID_BYTES or
            RUN_ID_RE.fullmatch(value) is None):
        raise ContractError(f"{label}: invalid bounded lowercase identifier")
    return value


def path_is_within(path: str, directory: str) -> bool:
    return directory == "." or path == directory or path.startswith(directory + "/")


def path_directory(path: str) -> str:
    prefix, separator, _ = path.rpartition("/")
    return prefix if separator else "."
