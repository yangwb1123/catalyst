"""Acyclic identity construction for ADR-0068 objects."""

from __future__ import annotations

import hashlib
from copy import deepcopy

from .codec import ContractError, canonical_json
from .constants import (
    ASSESSMENT_DOMAIN, CONTENT_SET_DOMAIN, CONTRACT_DOMAIN, ENTRY_DOMAIN,
    MAX_ASSESSMENT_BYTES, MAX_CONTENT_SET_BYTES, MAX_CONTRACT_BYTES,
    MAX_ENTRY_BYTES, MAX_REGISTRY_BYTES, MAX_REQUEST_BYTES, REGISTRY_DOMAIN,
    REQUEST_DOMAIN,
)

_SPECS = {
    "content_set": (CONTENT_SET_DOMAIN, ("set_sha256",), MAX_CONTENT_SET_BYTES, None),
    "contract": (CONTRACT_DOMAIN,
                 ("capability_contract_id", "capability_contract_sha256"),
                 MAX_CONTRACT_BYTES, "capability-contract-"),
    "entry": (ENTRY_DOMAIN, ("entry_id", "entry_sha256"),
              MAX_ENTRY_BYTES, "capability-registry-entry-"),
    "registry": (REGISTRY_DOMAIN, ("registry_id", "registry_sha256"),
                 MAX_REGISTRY_BYTES, "capability-registry-"),
    "request": (REQUEST_DOMAIN, ("request_sha256",), MAX_REQUEST_BYTES, None),
    "assessment": (ASSESSMENT_DOMAIN, ("assessment_sha256",),
                   MAX_ASSESSMENT_BYTES, None),
}


def object_digest(kind: str, value: dict[str, object]) -> str:
    try:
        domain, fields, maximum, _ = _SPECS[kind]
    except KeyError as error:
        raise ContractError(f"unsupported digest kind {kind!r}") from error
    blanked = deepcopy(value)
    for field in fields:
        if field not in blanked:
            raise ContractError(f"{kind} lacks identity field {field}")
        blanked[field] = ""
    return hashlib.sha256(domain + canonical_json(blanked, max_bytes=maximum)).hexdigest()


def seal(kind: str, value: dict[str, object]) -> dict[str, object]:
    result = deepcopy(value)
    digest = object_digest(kind, result)
    _, fields, _, prefix = _SPECS[kind]
    digest_field = fields[-1]
    result[digest_field] = digest
    if prefix is not None:
        result[fields[0]] = prefix + digest
    return result


def require_digest(kind: str, value: dict[str, object]) -> None:
    expected = object_digest(kind, value)
    digest_field = _SPECS[kind][1][-1]
    if value.get(digest_field) != expected:
        raise ContractError(f"{kind}.{digest_field}: digest mismatch")
    prefix = _SPECS[kind][3]
    if prefix is not None and value.get(_SPECS[kind][1][0]) != prefix + expected:
        raise ContractError(f"{kind}: derived ID mismatch")
