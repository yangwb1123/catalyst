"""Public pure Platform Core Envelope v1 operations."""

from __future__ import annotations

import os
import stat
from pathlib import Path
from typing import Any, Callable

from . import identity
from .artifact import validate_artifact_ref
from .codec import (ContractError, canonical_json, decode_canonical,
                    decode_json, exact_object, observation_digest,
                    validate_document_shape, validate_i64)
from .constants import (ARTIFACT_DIGEST_DOMAIN, COMMAND_DIGEST_DOMAIN,
                        COMMAND_FIELDS, COMMAND_TYPE_PROFILE, DOCUMENT_INVALID,
                        EVENT_DIGEST_DOMAIN, EVENT_FIELDS, EVENT_TYPE_PROFILE,
                        FIXTURE_API_VERSION, FIXTURE_PATH,
                        MAX_ARTIFACT_REF_BYTES, MAX_ENVELOPE_BYTES,
                        MAX_INPUT_PATH_BYTES, MAX_INPUT_PATH_COMPONENTS,
                        MAX_UNIX_MILLISECONDS, REFERENCE_MISMATCH,
                        RELATION_MISMATCH)
from .envelope import (validate_common_references,
                       validate_common_relations, validate_common_values,
                       validate_idempotency_key)
from .references import (scope_contains, validate_entity_ref,
                         validate_record_ref)


def decode_artifact_ref(raw: bytes) -> dict[str, Any]:
    """Decode and validate one exact canonical ArtifactRef."""
    value = _decode_document(raw, MAX_ARTIFACT_REF_BYTES)
    return validate_artifact_ref(value)


def canonical_artifact_ref(value: Any) -> bytes:
    """Validate and canonically encode one ArtifactRef."""
    return _validated_canonical(
        value, MAX_ARTIFACT_REF_BYTES, validate_artifact_ref)


def artifact_ref_sha256(value: Any) -> str:
    """Return the ArtifactRef conformance observation digest."""
    return observation_digest(ARTIFACT_DIGEST_DOMAIN, canonical_artifact_ref(value))


def decode_command_envelope(raw: bytes) -> dict[str, Any]:
    """Decode and validate one exact canonical CommandEnvelope."""
    return validate_command_envelope(_decode_document(raw, MAX_ENVELOPE_BYTES))


def canonical_command_envelope(value: Any) -> bytes:
    """Validate and canonically encode one CommandEnvelope."""
    return _validated_canonical(
        value, MAX_ENVELOPE_BYTES, validate_command_envelope)


def command_envelope_sha256(value: Any) -> str:
    """Return the CommandEnvelope conformance observation digest."""
    return observation_digest(COMMAND_DIGEST_DOMAIN, canonical_command_envelope(value))


def decode_event_envelope(raw: bytes) -> dict[str, Any]:
    """Decode and validate one exact canonical EventEnvelope."""
    return validate_event_envelope(_decode_document(raw, MAX_ENVELOPE_BYTES))


def canonical_event_envelope(value: Any) -> bytes:
    """Validate and canonically encode one EventEnvelope."""
    return _validated_canonical(
        value, MAX_ENVELOPE_BYTES, validate_event_envelope)


def event_envelope_sha256(value: Any) -> str:
    """Return the EventEnvelope conformance observation digest."""
    return observation_digest(EVENT_DIGEST_DOMAIN, canonical_event_envelope(value))


def _validated_canonical(
        value: Any, maximum: int,
        validator: Callable[[Any], dict[str, Any]]) -> bytes:
    """Validate a detached snapshot and return the exact bytes it represents."""
    canonical = canonical_json(value, maximum)
    snapshot = decode_canonical(canonical, maximum)
    validator(snapshot)
    return canonical


def _decode_document(raw: bytes, maximum: int) -> dict[str, Any]:
    try:
        return decode_canonical(raw, maximum)
    except ContractError as error:
        raise error.with_code(DOCUMENT_INVALID) from error


def validate_command_envelope(value: Any) -> dict[str, Any]:
    """Return one valid command declaration without authorizing it."""
    validate_document_shape(value, COMMAND_TYPE_PROFILE, "CommandEnvelope")
    node = exact_object(value, COMMAND_FIELDS, "CommandEnvelope")
    validate_common_values(node)
    _validate_command_values(node)
    validate_common_references(node)
    _validate_command_references(node)
    validate_common_relations(node)
    _validate_command_relations(node)
    canonical_json(node, MAX_ENVELOPE_BYTES)
    return node


def validate_event_envelope(value: Any) -> dict[str, Any]:
    """Return one valid event declaration without appending it."""
    validate_document_shape(value, EVENT_TYPE_PROFILE, "EventEnvelope")
    node = exact_object(value, EVENT_FIELDS, "EventEnvelope")
    validate_common_values(node)
    _validate_event_values(node)
    validate_common_references(node)
    _validate_event_references(node)
    validate_common_relations(node)
    _validate_event_relations(node)
    canonical_json(node, MAX_ENVELOPE_BYTES)
    return node


def _validate_command_values(node: dict[str, Any]) -> None:
    identity.validate_typed_id(node["command_id"], "cmd", "command_id")
    validate_entity_ref(node["target_ref"], "target_ref")
    _unix_ms(node["issued_at_unix_ms"], "issued_at_unix_ms")
    deadline = node["deadline_unix_ms"]
    if deadline is not None:
        _unix_ms(deadline, "deadline_unix_ms")
    version = node["expected_version"]
    if version is not None:
        validate_i64(version, "expected_version", 0)
    validate_idempotency_key(node["idempotency_key"])
    if node["authorization_ref"] is not None:
        validate_record_ref(node["authorization_ref"], "authorization_ref")


def _validate_command_references(node: dict[str, Any]) -> None:
    if not scope_contains(node["scope_ref"], node["target_ref"]):
        raise ContractError("target_ref is not represented by scope_ref",
                            REFERENCE_MISMATCH)


def _validate_command_relations(node: dict[str, Any]) -> None:
    if not identity.same_message_suffix(node["message_id"], node["command_id"]):
        raise ContractError("message_id and command_id suffixes must match",
                            RELATION_MISMATCH)
    deadline = node["deadline_unix_ms"]
    if deadline is not None and deadline < node["issued_at_unix_ms"]:
        raise ContractError("deadline_unix_ms must not precede issued_at_unix_ms",
                            RELATION_MISMATCH)
    artifact = node["payload_artifact_ref"]
    if artifact is not None and artifact["created_at_unix_ms"] > node["issued_at_unix_ms"]:
        raise ContractError("command payload artifact cannot be created after issue time",
                            RELATION_MISMATCH)


def _validate_event_values(node: dict[str, Any]) -> None:
    identity.validate_typed_id(node["event_id"], "evt", "event_id")
    validate_entity_ref(node["aggregate_ref"], "aggregate_ref")
    _unix_ms(node["occurred_at_unix_ms"], "occurred_at_unix_ms")
    validate_i64(node["aggregate_version"], "aggregate_version", 1)
    validate_i64(node["sequence"], "sequence", 1)
    if node["source_component"] not in {
            "app_server", "control_plane", "harness", "legacy_importer", "runtime"}:
        raise ContractError("source_component is unsupported")
    if node["source_snapshot_ref"] is not None:
        validate_entity_ref(node["source_snapshot_ref"], "source_snapshot_ref")


def _validate_event_references(node: dict[str, Any]) -> None:
    if not scope_contains(node["scope_ref"], node["aggregate_ref"]):
        raise ContractError("aggregate_ref is not represented by scope_ref",
                            REFERENCE_MISMATCH)
    _validate_event_snapshot(node)


def _validate_event_relations(node: dict[str, Any]) -> None:
    if not identity.same_message_suffix(node["message_id"], node["event_id"]):
        raise ContractError("message_id and event_id suffixes must match",
                            RELATION_MISMATCH)
    artifact = node["payload_artifact_ref"]
    if artifact is not None and artifact["created_at_unix_ms"] > node["occurred_at_unix_ms"]:
        raise ContractError("event payload artifact cannot be created after occurrence time",
                            RELATION_MISMATCH)


def _validate_event_snapshot(node: dict[str, Any]) -> None:
    scoped, source = node["scope_ref"]["project_snapshot_id"], node["source_snapshot_ref"]
    if scoped is None and source is None:
        return
    if scoped is None or source is None:
        raise ContractError("source_snapshot_ref must match scoped project_snapshot_id",
                            REFERENCE_MISMATCH)
    if source["entity_type"] != "project_snapshot" or source["entity_id"] != scoped:
        raise ContractError("source_snapshot_ref must match scoped project_snapshot_id",
                            REFERENCE_MISMATCH)


def _unix_ms(value: Any, label: str) -> int:
    return validate_i64(value, label, 1, MAX_UNIX_MILLISECONDS)


_FIXTURE_FIELDS = {
    "api_version", "artifact_ref", "command_envelope", "event_envelope", "expected",
}
_EXPECTED_FIELDS = {
    "artifact_ref_sha256", "command_envelope_sha256", "event_envelope_sha256",
}


def load_golden(repo_root: Path) -> dict[str, Any]:
    """Load and fully validate the shared repository golden."""
    path = repo_root / FIXTURE_PATH
    value = decode_json(read_bounded_file(path, 64 * 1024), 64 * 1024)
    fixture = exact_object(value, _FIXTURE_FIELDS, "golden fixture")
    if fixture["api_version"] != FIXTURE_API_VERSION:
        raise ContractError("golden fixture api_version is unsupported")
    validate_artifact_ref(fixture["artifact_ref"])
    validate_command_envelope(fixture["command_envelope"])
    validate_event_envelope(fixture["event_envelope"])
    expected = exact_object(fixture["expected"], _EXPECTED_FIELDS, "golden expected")
    _check_digest(expected, "artifact_ref_sha256",
                  artifact_ref_sha256(fixture["artifact_ref"]))
    _check_digest(expected, "command_envelope_sha256",
                  command_envelope_sha256(fixture["command_envelope"]))
    _check_digest(expected, "event_envelope_sha256",
                  event_envelope_sha256(fixture["event_envelope"]))
    return fixture


def read_bounded_file(path: Path, maximum: int = MAX_ENVELOPE_BYTES) -> bytes:
    """Read one stable regular file through a retained no-follow path chain."""
    chain: list[tuple[int, tuple[int, int, int, int]]] = []
    leaf_descriptor: int | None = None
    try:
        chain, names, leaf = _open_directory_chain(path)
        raw, leaf_descriptor, leaf_identity = _read_stable_leaf(
            chain[-1][0], leaf, path, maximum)
        _revalidate_directory_chain(chain, names, path)
        _revalidate_leaf(chain[-1][0], leaf, leaf_descriptor, leaf_identity, path)
        return raw
    except ContractError:
        raise
    except (OSError, RuntimeError, ValueError) as error:
        raise ContractError(f"{path}: unsafe or unavailable input path: {error}") from error
    finally:
        if leaf_descriptor is not None:
            os.close(leaf_descriptor)
        for descriptor, _identity_value in reversed(chain):
            os.close(descriptor)


def _open_directory_chain(
        path: Path) -> tuple[list[tuple[int, tuple[int, int, int, int]]], list[str], str]:
    anchor, components = _path_components(path)
    if not components:
        raise ContractError(f"{path}: expected a file path")
    flags = os.O_RDONLY | _required_open_flags("O_DIRECTORY", "O_NOFOLLOW", "O_CLOEXEC")
    descriptor = os.open(anchor, flags)
    try:
        root_identity = _directory_identity(os.fstat(descriptor))
    except BaseException:
        os.close(descriptor)
        raise
    chain = [(descriptor, root_identity)]
    names: list[str] = []
    try:
        for component in components[:-1]:
            child = os.open(component, flags, dir_fd=chain[-1][0])
            try:
                opened = os.fstat(child)
            except BaseException:
                os.close(child)
                raise
            if not stat.S_ISDIR(opened.st_mode):
                os.close(child)
                raise ContractError(f"{path}: ancestor is not a directory")
            chain.append((child, _directory_identity(opened)))
            names.append(component)
            descriptor = child
    except BaseException:
        for opened, _identity_value in reversed(chain):
            os.close(opened)
        raise
    return chain, names, components[-1]


def _path_components(path: Path) -> tuple[str, list[str]]:
    raw = os.fspath(path)
    if type(raw) is not str or not raw:
        raise ContractError("input path must be nonempty text")
    try:
        path_bytes = os.fsencode(raw)
    except UnicodeError as error:
        raise ContractError("input path is not representable by the filesystem") from error
    if len(path_bytes) > MAX_INPUT_PATH_BYTES:
        raise ContractError(f"input path exceeds {MAX_INPUT_PATH_BYTES} bytes")
    candidate = Path(raw)
    parts = list(candidate.parts)
    anchor = parts.pop(0) if candidate.is_absolute() else "."
    if len(parts) > MAX_INPUT_PATH_COMPONENTS:
        raise ContractError(
            f"input path exceeds {MAX_INPUT_PATH_COMPONENTS} components")
    if any(component in {"", ".", ".."} for component in parts):
        raise ContractError(f"{path}: dot path components are forbidden")
    return anchor, parts


def _read_stable_leaf(
        parent: int, leaf: str, path: Path,
        maximum: int) -> tuple[bytes, int, tuple[int, int, int, int, int, int, int]]:
    before = os.stat(leaf, dir_fd=parent, follow_symlinks=False)
    if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or
            before.st_size < 0 or before.st_size > maximum):
        raise ContractError(
            f"{path}: expected one stable single-link regular file <= {maximum} bytes")
    pin_flags = _required_open_flags("O_PATH", "O_NOFOLLOW", "O_CLOEXEC")
    pinned = os.open(leaf, pin_flags, dir_fd=parent)
    descriptor: int | None = None
    try:
        pinned_state = os.fstat(pinned)
        if _file_identity(before) != _file_identity(pinned_state):
            raise ContractError(f"{path}: file changed before readable open")
        read_flags = os.O_RDONLY | _required_open_flags("O_NONBLOCK", "O_CLOEXEC")
        descriptor = os.open(f"/proc/self/fd/{pinned}", read_flags)
        opened = os.fstat(descriptor)
        if _file_identity(pinned_state) != _file_identity(opened):
            raise ContractError(f"{path}: pinned file identity changed before read")
        raw = _read_exact(descriptor, opened.st_size)
        if _file_identity(opened) != _file_identity(os.fstat(descriptor)):
            raise ContractError(f"{path}: file changed during read")
        return raw, descriptor, _file_identity(opened)
    except BaseException:
        if descriptor is not None:
            os.close(descriptor)
        raise
    finally:
        os.close(pinned)


def _revalidate_leaf(
        parent: int, leaf: str, descriptor: int,
        expected: tuple[int, int, int, int, int, int, int], path: Path) -> None:
    if _file_identity(os.fstat(descriptor)) != expected:
        raise ContractError(f"{path}: file changed during read")
    after = os.stat(leaf, dir_fd=parent, follow_symlinks=False)
    if _file_identity(after) != expected:
        raise ContractError(f"{path}: file identity changed during read")


def _revalidate_directory_chain(
        chain: list[tuple[int, tuple[int, int, int, int]]], names: list[str], path: Path) -> None:
    flags = os.O_RDONLY | _required_open_flags("O_DIRECTORY", "O_NOFOLLOW", "O_CLOEXEC")
    for descriptor, expected in chain:
        if _directory_identity(os.fstat(descriptor)) != expected:
            raise ContractError(f"{path}: ancestor changed during read")
    for index, name in enumerate(names, start=1):
        reopened = os.open(name, flags, dir_fd=chain[index - 1][0])
        try:
            if _directory_identity(os.fstat(reopened)) != chain[index][1]:
                raise ContractError(f"{path}: ancestor identity changed during read")
        finally:
            os.close(reopened)


def _required_open_flags(*names: str) -> int:
    flags = 0
    for name in names:
        value = getattr(os, name, None)
        if value is None:
            raise ContractError(f"explicit contract reads require {name} support")
        flags |= value
    return flags


def _read_exact(descriptor: int, size: int) -> bytes:
    chunks, remaining = [], size
    while remaining:
        chunk = os.read(descriptor, min(remaining, 64 * 1024))
        if not chunk:
            raise ContractError("contract file ended before its observed size")
        chunks.append(chunk)
        remaining -= len(chunk)
    if os.read(descriptor, 1):
        raise ContractError("contract file grew during bounded read")
    return b"".join(chunks)


def _file_identity(value: os.stat_result) -> tuple[int, int, int, int, int, int, int]:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_nlink,
            value.st_size, value.st_mtime_ns, value.st_ctime_ns)


def _directory_identity(value: os.stat_result) -> tuple[int, int, int, int]:
    return value.st_dev, value.st_ino, value.st_mode, value.st_nlink


def _check_digest(expected: dict[str, Any], field: str, actual: str) -> None:
    if expected[field] != actual:
        raise ContractError(f"golden {field} pin drifted")
