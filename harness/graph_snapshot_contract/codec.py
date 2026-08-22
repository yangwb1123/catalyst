"""Bounded canonical JSON, Base64URL, and digest primitives for ADR-0065."""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import unicodedata

from governance_contract import ContractError

from .constants import (
    BASE64URL_RE, MAX_ARRAY_ITEMS, MAX_CROSSWALKS, MAX_DEPTH, MAX_EDGE_UNION,
    MAX_ENVELOPE_BYTES, MAX_FIELDS, MAX_GRAPH_BASE64URL_BYTES, MAX_GRAPH_BYTES,
    MAX_IDENTIFIER_BYTES, MAX_I64, MAX_LOCATORS_PER_RECORD, MAX_NODES,
    MAX_PATH_BYTES, MAX_PATH_SCALARS, MAX_UNRESOLVED_NODES, MIN_I64,
    MAX_TEST_SOURCE_EDGE_UNION, MAX_TEST_SOURCE_NODES, SNAPSHOT_API,
    TEST_SOURCE_PROFILE_ID,
)


def forbidden_scalar(character: str) -> bool:
    code = ord(character)
    return (unicodedata.category(character) == "Cc" or
            0xD800 <= code <= 0xDFFF or
            code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


def _valid_key(key: object) -> bool:
    return (isinstance(key, str) and bool(key) and "a" <= key[0] <= "z" and
            all(char == "_" or "a" <= char <= "z" or "0" <= char <= "9"
                for char in key))


def _array_limit(path: tuple[str, ...], root_snapshot: bool,
                 profile_id: object,
                 overrides: dict[tuple[str, ...], int] | None) -> int:
    if overrides is not None and path in overrides:
        return overrides[path]
    normalized = ("snapshot",) + path if root_snapshot else path
    if normalized == ("snapshot", "edges"):
        return (MAX_TEST_SOURCE_EDGE_UNION
                if profile_id == TEST_SOURCE_PROFILE_ID else MAX_EDGE_UNION)
    exact = {
        ("snapshot", "adr_0062_node_crosswalk"): MAX_CROSSWALKS,
        ("snapshot", "extractors"): 1,
        ("snapshot", "nodes"): (MAX_TEST_SOURCE_NODES
                                  if profile_id == TEST_SOURCE_PROFILE_ID
                                  else MAX_NODES),
        ("snapshot", "sources"): 1,
        ("snapshot", "system_unknown_reason_codes"): 20,
        ("snapshot", "unresolved_nodes"): MAX_UNRESOLVED_NODES,
        ("snapshot", "coverage", "surfaces"): 11,
        ("snapshot", "freshness", "reason_codes"): 20,
    }
    if normalized in exact:
        return exact[normalized]
    if (len(normalized) >= 3 and normalized[0] == "snapshot" and
            normalized[1] in {
                "edges", "nodes", "unresolved_edges", "unresolved_nodes"}):
        return {
            "category_axes": 7,
            "claim_record_ids": 0,
            "evidence_record_ids": 0,
            "extractor_sha256s": 1,
            "owner_node_ids": 0,
            "qualified_name_components": 3,
            "candidate_qualified_name_components": 2,
            "source_ids": 1,
            "source_locators": MAX_LOCATORS_PER_RECORD,
            "target_node_ids": MAX_LOCATORS_PER_RECORD,
        }.get(normalized[-1], MAX_ARRAY_ITEMS)
    if (len(normalized) >= 4 and
            normalized[:3] == ("snapshot", "coverage", "surfaces") and
            normalized[-1] == "reason_codes"):
        return 20
    return MAX_ARRAY_ITEMS


def _walk_string(value: str, path: tuple[str, ...]) -> None:
    try:
        encoded = value.encode("utf-8")
    except UnicodeError as error:
        raise ContractError(f"ADR-0065 string is not valid UTF-8: {error}") from error
    field = path[-1] if path else ""
    if path in {("graph_observation_base64url",),
                ("request", "graph_observation_base64url")}:
        limit, scalar_limit = MAX_GRAPH_BASE64URL_BYTES, None
    elif path == ("expected", "canonical_envelope_json"):
        limit, scalar_limit = MAX_ENVELOPE_BYTES, None
    elif path == ("input", "canonical_graph_observation_json"):
        limit, scalar_limit = MAX_GRAPH_BYTES, None
    elif field in {"observer_run_id", "project_id", "run_id"}:
        limit, scalar_limit = MAX_IDENTIFIER_BYTES, MAX_PATH_SCALARS
    else:
        limit, scalar_limit = MAX_PATH_BYTES, MAX_PATH_SCALARS
    if len(encoded) > limit or scalar_limit is not None and len(value) > scalar_limit:
        raise ContractError(f"ADR-0065 string at {'.'.join(path)} exceeds its bound")
    if any(forbidden_scalar(character) for character in value):
        raise ContractError("ADR-0065 string contains forbidden Unicode scalar")


def _walk(value: object, depth: int = 1,
          path: tuple[str, ...] = (), root_snapshot: bool = False,
          discriminator_only: bool = False, profile_id: object = None,
          array_limit_overrides: dict[tuple[str, ...], int] | None = None) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"ADR-0065 JSON depth exceeds {MAX_DEPTH}")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not MIN_I64 <= value <= MAX_I64:
            raise ContractError("ADR-0065 integer is outside signed int64")
        return
    if isinstance(value, str):
        if discriminator_only:
            encoded = value.encode("utf-8")
            if (len(encoded) > MAX_GRAPH_BASE64URL_BYTES or
                    any(forbidden_scalar(character) for character in value)):
                raise ContractError("ADR-0065 discriminator string exceeds safe bound")
        else:
            _walk_string(value, path)
        return
    if isinstance(value, list):
        limit = (MAX_TEST_SOURCE_EDGE_UNION if discriminator_only else
                 _array_limit(path, root_snapshot, profile_id,
                              array_limit_overrides))
        if len(value) > limit:
            raise ContractError(f"ADR-0065 array at {'.'.join(path)} exceeds {limit} items")
        for item in value:
            _walk(item, depth + 1, path, root_snapshot, discriminator_only,
                  profile_id, array_limit_overrides)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported ADR-0065 JSON value {type(value).__name__}")
    if len(value) > MAX_FIELDS:
        raise ContractError(f"ADR-0065 object exceeds {MAX_FIELDS} fields")
    for key, item in value.items():
        if not _valid_key(key):
            raise ContractError(f"ADR-0065 key {key!r} is not ASCII snake_case")
        _walk_string(key, path + (key,))
        _walk(item, depth + 1, path + (key,), root_snapshot,
              discriminator_only, profile_id, array_limit_overrides)


def _root_profile(value: object) -> object:
    if not isinstance(value, dict):
        return None
    if value.get("api_version") == SNAPSHOT_API:
        return value.get("profile_id")
    snapshot = value.get("snapshot")
    return snapshot.get("profile_id") if isinstance(snapshot, dict) else None


def canonical_json(
        value: object, *, max_bytes: int = MAX_ENVELOPE_BYTES,
        array_limit_overrides: dict[tuple[str, ...], int] | None = None) -> bytes:
    try:
        root_snapshot = (isinstance(value, dict) and
                         value.get("api_version") == SNAPSHOT_API)
        _walk(value, root_snapshot=root_snapshot, profile_id=_root_profile(value),
              array_limit_overrides=array_limit_overrides)
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":")).encode("utf-8")
        if len(raw) > max_bytes:
            raise ContractError(f"ADR-0065 canonical JSON exceeds {max_bytes} bytes")
        return raw
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"ADR-0065 canonical JSON failed: {error}") from error


def _reject_number(value: str) -> None:
    raise ContractError(f"floating or non-finite ADR-0065 number {value!r} is forbidden")


def _parse_int(value: str) -> int:
    digits = value[1:] if value.startswith("-") else value
    if len(digits) > 19:
        raise ContractError("ADR-0065 integer is outside signed int64")
    parsed = int(value)
    if not MIN_I64 <= parsed <= MAX_I64:
        raise ContractError("ADR-0065 integer is outside signed int64")
    return parsed


def _pairs_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate ADR-0065 JSON key {key!r}")
        result[key] = value
    return result


def _precheck_depth(text: str) -> None:
    depth, in_string, escaped = 0, False, False
    for character in text:
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
        elif character == '"':
            in_string = True
        elif character in "[{":
            depth += 1
            if depth > MAX_DEPTH:
                raise ContractError(f"ADR-0065 JSON depth exceeds {MAX_DEPTH}")
        elif character in "]}":
            depth -= 1


def decode_canonical(raw: bytes, *, max_bytes: int, label: str) -> object:
    try:
        if not isinstance(raw, bytes):
            raise ContractError(f"{label} input must be bytes")
        if len(raw) > max_bytes:
            raise ContractError(f"{label} exceeds {max_bytes} bytes")
        text = raw.decode("utf-8")
        _precheck_depth(text)
        value = json.loads(text, object_pairs_hook=_pairs_object,
                           parse_int=_parse_int, parse_float=_reject_number,
                           parse_constant=_reject_number)
        root_snapshot = (isinstance(value, dict) and
                         value.get("api_version") == SNAPSHOT_API)
        _walk(value, root_snapshot=root_snapshot, profile_id=_root_profile(value))
        if canonical_json(value, max_bytes=max_bytes) != raw:
            raise ContractError(f"{label} is not exact compact canonical JSON")
        return value
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError,
            json.JSONDecodeError) as error:
        raise ContractError(f"invalid {label} UTF-8 JSON: {error}") from error


def decode_profile_discriminators(raw: bytes, *, max_bytes: int,
                                  label: str) -> object:
    """Decode exact canonical bytes under only broad resource safety limits."""
    try:
        if not isinstance(raw, bytes) or len(raw) > max_bytes:
            raise ContractError(f"{label} exceeds its byte bound")
        text = raw.decode("utf-8")
        _precheck_depth(text)
        value = json.loads(text, object_pairs_hook=_pairs_object,
                           parse_int=_parse_int, parse_float=_reject_number,
                           parse_constant=_reject_number)
        _walk(value, discriminator_only=True)
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True,
                             separators=(",", ":")).encode("utf-8")
        if encoded != raw:
            raise ContractError(f"{label} is not exact compact canonical JSON")
        return value
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError,
            json.JSONDecodeError) as error:
        raise ContractError(f"invalid {label} UTF-8 JSON: {error}") from error


def decode_base64url(value: object) -> bytes:
    if (not isinstance(value, str) or not 3 <= len(value) <= MAX_GRAPH_BASE64URL_BYTES or
            BASE64URL_RE.fullmatch(value) is None):
        raise ContractError("request graph observation is not bounded unpadded Base64URL")
    try:
        encoded = value.encode("ascii")
        padding = b"=" * ((4 - len(encoded) % 4) % 4)
        raw = base64.b64decode(encoded + padding, altchars=b"-_", validate=True)
    except (UnicodeError, ValueError, binascii.Error) as error:
        raise ContractError(f"request graph observation Base64URL is invalid: {error}") from error
    if len(raw) > MAX_GRAPH_BYTES:
        raise ContractError(f"decoded graph observation exceeds {MAX_GRAPH_BYTES} bytes")
    if base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii") != value:
        raise ContractError("request graph observation Base64URL is not canonical")
    return raw


def domain_digest(
        domain: bytes, value: object, *, max_bytes: int,
        array_limit_overrides: dict[tuple[str, ...], int] | None = None) -> str:
    encoded = canonical_json(
        value, max_bytes=max_bytes, array_limit_overrides=array_limit_overrides)
    return hashlib.sha256(domain + encoded).hexdigest()


def self_digest(domain: bytes, value: dict[str, object], field: str,
                *, max_bytes: int) -> str:
    preimage = dict(value)
    preimage[field] = ""
    return domain_digest(domain, preimage, max_bytes=max_bytes)
