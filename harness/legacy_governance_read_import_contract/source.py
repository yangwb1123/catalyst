"""Validation of supplied request bytes and source framing."""

from __future__ import annotations

import base64
import re
from typing import Any

from .canonical import (
    ContractError, canonical_json, decode_canonical, require_exact_fields, self_digest,
    sha256_bytes, validate_text,
)
from .constants import (
    ADR_KIND, CANONICALIZATION, MAX_ADR_BYTES, MAX_ADR_SOURCES,
    MAX_BINDING_BYTES, MAX_MEMORY_BYTES, MAX_RAW_BYTES, MAX_REQUEST_BYTES,
    MAX_SOURCE_REF_BYTES, MEMORY_KIND, REQUEST_API, REQUEST_DOMAIN, REQUEST_KIND,
)

HEX_RE = re.compile(r"[0-9a-f]{64}")
IDENTIFIER_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:/@+-]{0,159}")
BASE64URL_RE = re.compile(r"[A-Za-z0-9_-]*")
REQUEST_FIELDS = {
    "api_version", "binding", "canonicalization", "kind", "request_sha256", "sources",
}
BINDING_FIELDS = {"project_id", "source_revision", "source_tree_sha256"}
SOURCE_FIELDS = {
    "byte_count", "content_base64url", "content_sha256", "source_kind", "source_ref",
}


def encode_base64url(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii")


def decode_base64url(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or BASE64URL_RE.fullmatch(value) is None:
        raise ContractError(f"{label} must be unpadded RFC 4648 base64url")
    try:
        raw = base64.b64decode(value + "=" * (-len(value) % 4), altchars=b"-_", validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise ContractError(f"{label} is invalid base64url") from error
    if encode_base64url(raw) != value:
        raise ContractError(f"{label} is not canonical unpadded base64url")
    return raw


def require_digest(value: Any, label: str) -> str:
    if not isinstance(value, str) or HEX_RE.fullmatch(value) is None:
        raise ContractError(f"{label} must be lowercase SHA-256 hex")
    return value


def validate_binding(value: Any) -> dict[str, Any]:
    binding = require_exact_fields(value, BINDING_FIELDS, "binding")
    for field in ("project_id", "source_revision"):
        text = validate_text(binding[field], f"binding.{field}", MAX_BINDING_BYTES)
        if IDENTIFIER_RE.fullmatch(text) is None:
            raise ContractError(f"binding.{field} is not a closed ASCII identifier")
    require_digest(binding["source_tree_sha256"], "binding.source_tree_sha256")
    return binding


def _framing(raw: bytes, source_kind: str, label: str) -> None:
    maximum = MAX_MEMORY_BYTES if source_kind == MEMORY_KIND else MAX_ADR_BYTES
    if not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    if b"\r" in raw or not raw.endswith(b"\n"):
        raise ContractError(f"{label} must use LF-only framing and end in LF")
    try:
        raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ContractError(f"{label} must be strict UTF-8") from error


def _source(value: Any, index: int) -> tuple[dict[str, Any], bytes]:
    label = f"sources[{index}]"
    source = require_exact_fields(value, SOURCE_FIELDS, label)
    kind = source["source_kind"]
    if not isinstance(kind, str) or kind not in {MEMORY_KIND, ADR_KIND}:
        raise ContractError(f"{label}.source_kind is unsupported")
    validate_text(source["source_ref"], f"{label}.source_ref", MAX_SOURCE_REF_BYTES)
    raw = decode_base64url(source["content_base64url"], f"{label}.content_base64url")
    if isinstance(source["byte_count"], bool) or source["byte_count"] != len(raw):
        raise ContractError(f"{label}.byte_count does not match supplied bytes")
    digest = require_digest(source["content_sha256"], f"{label}.content_sha256")
    if sha256_bytes(raw) != digest:
        raise ContractError(f"{label}.content_sha256 does not match supplied bytes")
    _framing(raw, kind, label)
    return source, raw


def decode_request(raw: bytes) -> tuple[dict[str, Any], list[bytes]]:
    if not isinstance(raw, bytes) or not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("legacy import request wire must end in exactly one LF")
    value = decode_canonical(raw[:-1], MAX_REQUEST_BYTES - 1, "legacy import request")
    request = require_exact_fields(value, REQUEST_FIELDS, "request")
    if request["api_version"] != REQUEST_API or request["kind"] != REQUEST_KIND:
        raise ContractError("request api_version or kind is not frozen v1")
    if request["canonicalization"] != CANONICALIZATION:
        raise ContractError("request canonicalization is not frozen v1")
    validate_binding(request["binding"])
    sources = request["sources"]
    if not isinstance(sources, list) or not 1 <= len(sources) <= MAX_ADR_SOURCES + 1:
        raise ContractError("request sources cardinality is outside 1..257")
    decoded = [_source(item, index) for index, item in enumerate(sources)]
    keys = [(item[0]["source_kind"], item[0]["source_ref"]) for item in decoded]
    if keys != sorted(keys) or len(set(keys)) != len(keys):
        raise ContractError("request sources must be unique and ordered by kind then ref")
    if sum(key[0] == MEMORY_KIND for key in keys) > 1:
        raise ContractError("request may contain at most one memory JSONL source")
    if sum(key[0] == ADR_KIND for key in keys) > MAX_ADR_SOURCES:
        raise ContractError("request ADR source count exceeds its bound")
    raw_sources = [item[1] for item in decoded]
    if sum(map(len, raw_sources)) > MAX_RAW_BYTES:
        raise ContractError("request aggregate supplied source bytes exceed the bound")
    require_digest(request["request_sha256"], "request.request_sha256")
    expected = self_digest(REQUEST_DOMAIN, request, "request_sha256",
                           MAX_REQUEST_BYTES, "request")
    if request["request_sha256"] != expected:
        raise ContractError("request_sha256 does not match the canonical request")
    return request, raw_sources


def source_descriptors(request: dict[str, Any]) -> list[dict[str, Any]]:
    return [{key: source[key] for key in
             ("byte_count", "content_sha256", "source_kind", "source_ref")}
            for source in request["sources"]]


def make_request(binding: dict[str, str],
                 sources: list[tuple[str, str, bytes]]) -> bytes:
    """Build canonical request bytes from already-supplied source bytes."""
    if not isinstance(sources, list):
        raise ContractError("request sources builder input must be a list")
    entries = []
    for index, source in enumerate(sources):
        if (not isinstance(source, tuple) or len(source) != 3 or
                not isinstance(source[0], str) or not isinstance(source[1], str) or
                not isinstance(source[2], bytes)):
            raise ContractError(f"request source builder item {index} has invalid types")
        source_kind, source_ref, raw = source
        entries.append({
            "byte_count": len(raw), "content_base64url": encode_base64url(raw),
            "content_sha256": sha256_bytes(raw), "source_kind": source_kind,
            "source_ref": source_ref,
        })
    entries.sort(key=lambda item: (item["source_kind"], item["source_ref"]))
    request = {
        "api_version": REQUEST_API,
        "binding": binding,
        "canonicalization": CANONICALIZATION,
        "kind": REQUEST_KIND,
        "request_sha256": "",
        "sources": entries,
    }
    request["request_sha256"] = self_digest(
        REQUEST_DOMAIN, request, "request_sha256", MAX_REQUEST_BYTES, "request")
    encoded = canonical_json(request, MAX_REQUEST_BYTES - 1, "request") + b"\n"
    decode_request(encoded)
    return encoded
