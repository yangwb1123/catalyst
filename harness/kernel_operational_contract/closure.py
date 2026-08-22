"""Nonsemantic KernelOperationalReferenceClosure sealing and validation."""

from __future__ import annotations

import copy
import hashlib

from .codec import ContractError, canonical_json, decode_canonical_json
from .constants import (ARTIFACT_RECEIPT_FIELDS, CANONICALIZATION, CLOSURE_API,
                        CLOSURE_DOMAIN, CLOSURE_FIELDS, CLOSURE_KIND, CLOSURE_PREFIX,
                        MAX_ARTIFACT_RECEIPTS, MAX_ARTIFACTS, MAX_CLOSURE_BYTES,
                        MAX_EVENTS, MAX_EXECUTION_RECEIPTS, MAX_INVOCATIONS,
                        SUCCESS_MARKER)
from .graph import validate_reference_graph
from .records import (validate_artifact_receipt, validate_capability_invocation,
                      validate_execution_receipt, validate_interaction_event)
from .shape import attestations, bare_hash, exact_object, optional_artifact_list


def _record_array(value: object, label: str, minimum: int, maximum: int,
                  validator) -> list[dict[str, object]]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} cardinality must be {minimum}..{maximum}")
    return [validator(item) for item in value]


def _receipt_order(records: list[dict[str, object]], field: str, label: str) -> None:
    identities = [item[field].encode("utf-8") for item in records]
    if identities != sorted(identities) or len(identities) != len(set(identities)):
        raise ContractError(f"{label} must be strictly identity-sorted and unique")


def _identity(closure: dict[str, object], allow_blank: bool) -> None:
    if allow_blank and closure["closure_id"] == closure["closure_sha256"] == "":
        return
    digest = bare_hash(closure["closure_sha256"], "closure_sha256")
    if closure["closure_id"] != f"{CLOSURE_PREFIX}{digest}":
        raise ContractError("closure_id must bind closure_sha256")


def validate_closure_shape(value: object, *, allow_blank: bool = False) -> dict[str, object]:
    """Validate exact closure members, ordering, and the complete supplied DAG."""
    closure = exact_object(value, CLOSURE_FIELDS, "KernelOperationalReferenceClosure")
    constants = {"api_version": CLOSURE_API, "canonicalization": CANONICALIZATION,
                 "kind": CLOSURE_KIND, "result": SUCCESS_MARKER}
    for field, expected in constants.items():
        if closure[field] != expected:
            raise ContractError(f"{field} must be {expected!r}")
    _identity(closure, allow_blank)
    attestations(closure["attestations"])
    optional_artifact_list(closure["artifacts"], "artifacts", MAX_ARTIFACTS)
    _record_array(closure["artifact_receipts"], "artifact_receipts", 0,
                  MAX_ARTIFACT_RECEIPTS, validate_artifact_receipt)
    _record_array(closure["capability_invocations"], "capability_invocations", 1,
                  MAX_INVOCATIONS, validate_capability_invocation)
    _record_array(closure["interaction_events"], "interaction_events", 0,
                  MAX_EVENTS, validate_interaction_event)
    _record_array(closure["execution_receipts"], "execution_receipts", 1,
                  MAX_EXECUTION_RECEIPTS, validate_execution_receipt)
    _receipt_order(closure["artifact_receipts"], "artifact_receipt_id",
                   "artifact_receipts")
    validate_reference_graph(closure)
    return closure


def _blanked(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != CLOSURE_FIELDS:
        raise ContractError("closure digest input must have exact top-level fields")
    result = copy.deepcopy(value)
    result["closure_id"], result["closure_sha256"] = "", ""
    return result


def closure_digest(value: object) -> str:
    """Compute the closure self-digest over its blank-identity canonical form."""
    blank = _blanked(value)
    validate_closure_shape(blank, allow_blank=True)
    raw = canonical_json(blank)
    if len(raw) > MAX_CLOSURE_BYTES:
        raise ContractError(f"closure blank preimage exceeds {MAX_CLOSURE_BYTES} bytes")
    return hashlib.sha256(CLOSURE_DOMAIN + raw).hexdigest()


def validate_closure(value: object) -> dict[str, object]:
    """Validate one sealed closure and all in-memory semantic document ceilings."""
    closure = validate_closure_shape(value)
    if len(canonical_json(closure)) > MAX_CLOSURE_BYTES:
        raise ContractError(f"closure exceeds {MAX_CLOSURE_BYTES} bytes")
    if closure["closure_sha256"] != closure_digest(closure):
        raise ContractError("closure_sha256 does not match the canonical preimage")
    return closure


def seal_closure(value: object) -> dict[str, object]:
    """Seal a blank-identity closure without mutating the caller."""
    closure = copy.deepcopy(value)
    if not isinstance(closure, dict):
        raise ContractError("closure must be an object")
    if closure.get("closure_id") != "" or closure.get("closure_sha256") != "":
        raise ContractError("sealing requires blank closure identity fields")
    digest = closure_digest(closure)
    closure["closure_id"], closure["closure_sha256"] = f"{CLOSURE_PREFIX}{digest}", digest
    return validate_closure(closure)


def decode_closure(raw: bytes) -> dict[str, object]:
    """Decode one exact canonical closure document with no trailing LF."""
    return validate_closure(decode_canonical_json(raw, MAX_CLOSURE_BYTES))
