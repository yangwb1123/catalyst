"""Exact-source request construction and validation."""

from __future__ import annotations

import base64
import binascii
import hashlib

from .codec import ContractError, canonical_json, decode_canonical
from .constants import (
    API_REQUEST, CANONICAL, CATALOG_NAME, MAPPING_NAME, MAX_CATALOG_BYTES,
    MAX_MAPPING_BYTES, MAX_REQUEST_BYTES, REQUEST_DOMAIN,
)
from .shapes import digest, exact_object, fixed, integer, string
from .sources import Ownership, parse_sources

REQUEST_FIELDS = {
    "api_version", "canonicalization", "catalog_source", "kind",
    "mapping_source", "request_sha256",
}
SOURCE_FIELDS = {
    "content_base64", "content_bytes", "content_encoding", "content_sha256",
    "document_name", "media_type", "source_role",
}


def _hash(domain: bytes, value: object) -> str:
    return hashlib.sha256(domain + canonical_json(value)).hexdigest()


def _source(raw: bytes, name: str, role: str, maximum: int) -> dict[str, object]:
    if not isinstance(raw, bytes) or not 1 <= len(raw) <= maximum:
        raise ContractError(f"{role} source byte length must be 1..{maximum}")
    return {
        "content_base64": base64.b64encode(raw).decode("ascii"),
        "content_bytes": len(raw),
        "content_encoding": "base64-rfc4648-canonical",
        "content_sha256": hashlib.sha256(raw).hexdigest(),
        "document_name": name,
        "media_type": "application/yaml",
        "source_role": role,
    }


def _seal(request: dict[str, object]) -> dict[str, object]:
    request["request_sha256"] = ""
    request["request_sha256"] = _hash(REQUEST_DOMAIN, request)
    canonical_json(request, MAX_REQUEST_BYTES)
    return request


def build_request(catalog_raw: bytes, mapping_raw: bytes) -> dict[str, object]:
    if not isinstance(catalog_raw, bytes) or not 1 <= len(catalog_raw) <= MAX_CATALOG_BYTES:
        raise ContractError(f"capability_catalog source byte length must be 1..{MAX_CATALOG_BYTES}")
    if not isinstance(mapping_raw, bytes) or not 1 <= len(mapping_raw) <= MAX_MAPPING_BYTES:
        raise ContractError(f"capability_skill_map source byte length must be 1..{MAX_MAPPING_BYTES}")
    parse_sources(catalog_raw, mapping_raw)
    return _seal({
        "api_version": API_REQUEST,
        "canonicalization": CANONICAL,
        "catalog_source": _source(catalog_raw, CATALOG_NAME, "capability_catalog",
                                  MAX_CATALOG_BYTES),
        "kind": "PlanningCapabilityOwnershipProjectionRequest",
        "mapping_source": _source(mapping_raw, MAPPING_NAME, "capability_skill_map",
                                  MAX_MAPPING_BYTES),
        "request_sha256": "",
    })


def _decode_source(value: object, name: str, role: str,
                   maximum: int, label: str) -> bytes:
    source = exact_object(value, SOURCE_FIELDS, label)
    fixed(source["content_encoding"], "base64-rfc4648-canonical",
          f"{label}.content_encoding")
    fixed(source["document_name"], name, f"{label}.document_name")
    fixed(source["media_type"], "application/yaml", f"{label}.media_type")
    fixed(source["source_role"], role, f"{label}.source_role")
    encoded_maximum = 4 * ((maximum + 2) // 3)
    encoded = string(source["content_base64"], f"{label}.content_base64", encoded_maximum)
    try:
        raw = base64.b64decode(encoded, validate=True)
    except (binascii.Error, ValueError) as error:
        raise ContractError(f"{label}.content_base64: invalid RFC 4648 base64") from error
    if base64.b64encode(raw).decode("ascii") != encoded:
        raise ContractError(f"{label}.content_base64: noncanonical base64")
    expected_bytes = integer(source["content_bytes"], 1, maximum, f"{label}.content_bytes")
    if len(raw) != expected_bytes:
        raise ContractError(f"{label}.content_bytes: mismatch")
    if hashlib.sha256(raw).hexdigest() != digest(source["content_sha256"],
                                                f"{label}.content_sha256"):
        raise ContractError(f"{label}.content_sha256: mismatch")
    return raw


def validate_request(value: object) -> tuple[dict[str, object], Ownership]:
    request = exact_object(value, REQUEST_FIELDS, "request")
    fixed(request["api_version"], API_REQUEST, "request.api_version")
    fixed(request["canonicalization"], CANONICAL, "request.canonicalization")
    fixed(request["kind"], "PlanningCapabilityOwnershipProjectionRequest", "request.kind")
    catalog = _decode_source(request["catalog_source"], CATALOG_NAME,
                             "capability_catalog", MAX_CATALOG_BYTES, "request.catalog_source")
    mapping = _decode_source(request["mapping_source"], MAPPING_NAME,
                             "capability_skill_map", MAX_MAPPING_BYTES, "request.mapping_source")
    stored = digest(request["request_sha256"], "request.request_sha256")
    preimage = dict(request)
    preimage["request_sha256"] = ""
    if _hash(REQUEST_DOMAIN, preimage) != stored:
        raise ContractError("request.request_sha256: digest mismatch")
    canonical_json(request, MAX_REQUEST_BYTES)
    return request, parse_sources(catalog, mapping)


def decode_request(raw: bytes) -> dict[str, object]:
    value = decode_canonical(raw, MAX_REQUEST_BYTES, "ownership projection request")
    return validate_request(value)[0]
