"""KernelDecisionReferenceClosure v1 sealing and validation."""

from __future__ import annotations

import copy
import hashlib

from kernel_operational_contract import validate_closure as validate_operational_closure
from kernel_operational_contract.shape import bare_hash, exact_object

from .atoms import validate_cognitive_atom
from .codec import ContractError, canonical_json, decode_canonical_json
from .constants import *  # noqa: F403 - frozen vocabulary is intentionally central
from .graph import validate_reference_graph
from .shape import attestations
from .transaction import validate_decision_transaction


def _identity(closure: dict[str, object], allow_blank: bool) -> None:
    if allow_blank and closure["closure_id"] == closure["closure_sha256"] == "":
        return
    digest = bare_hash(closure["closure_sha256"], "closure_sha256")
    if closure["closure_id"] != f"{CLOSURE_PREFIX}{digest}":  # noqa: F405
        raise ContractError("closure_id must bind closure_sha256")


def _atoms(value: object) -> list[dict[str, object]]:
    if not isinstance(value, list) or not 1 <= len(value) <= MAX_ATOMS:  # noqa: F405
        raise ContractError(f"cognitive_atoms cardinality must be 1..{MAX_ATOMS}")  # noqa: F405
    atoms = [validate_cognitive_atom(item) for item in value]
    identities = [item["atom_id"].encode("utf-8") for item in atoms]
    if identities != sorted(identities) or len(identities) != len(set(identities)):
        raise ContractError("cognitive_atoms must be strictly atom-id sorted and unique")
    if len(canonical_json(atoms)) > MAX_ATOM_SET_BYTES:  # noqa: F405
        raise ContractError("cognitive_atoms exceeds the aggregate byte ceiling")
    return atoms


def validate_closure_shape(value: object, *, allow_blank: bool = False) -> dict[str, object]:
    closure = exact_object(value, CLOSURE_FIELDS, "KernelDecisionReferenceClosure")  # noqa: F405
    constants = {"api_version": CLOSURE_API, "canonicalization": CANONICALIZATION,  # noqa: F405
                 "kind": CLOSURE_KIND, "result": SUCCESS_MARKER}  # noqa: F405
    for field, expected in constants.items():
        if closure[field] != expected:
            raise ContractError(f"{field} must be {expected!r}")
    _identity(closure, allow_blank)
    attestations(closure["attestations"])
    atoms = _atoms(closure["cognitive_atoms"])
    transaction = validate_decision_transaction(closure["decision_transaction"])
    try:
        operational = validate_operational_closure(closure["operational_closure"])
    except ValueError as error:
        raise ContractError(f"operational_closure: {error}") from error
    validate_reference_graph(atoms, transaction, operational)
    return closure


def _blanked(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != CLOSURE_FIELDS:  # noqa: F405
        raise ContractError("closure digest input must have exact fields")
    result = copy.deepcopy(value)
    result["closure_id"], result["closure_sha256"] = "", ""
    return result


def closure_digest(value: object) -> str:
    blank = _blanked(value)
    validate_closure_shape(blank, allow_blank=True)
    raw = canonical_json(blank)
    if len(raw) > MAX_CLOSURE_BYTES:  # noqa: F405
        raise ContractError("closure blank preimage exceeds its byte ceiling")
    return hashlib.sha256(CLOSURE_DOMAIN + raw).hexdigest()  # noqa: F405


def validate_closure(value: object) -> dict[str, object]:
    closure = validate_closure_shape(value)
    if len(canonical_json(closure)) > MAX_CLOSURE_BYTES:  # noqa: F405
        raise ContractError("closure exceeds its byte ceiling")
    if closure["closure_sha256"] != closure_digest(closure):
        raise ContractError("closure_sha256 does not match canonical preimage")
    return closure


def seal_closure(value: object) -> dict[str, object]:
    closure = copy.deepcopy(value)
    if (not isinstance(closure, dict) or closure.get("closure_id") != "" or
            closure.get("closure_sha256") != ""):
        raise ContractError("sealing closure requires blank identity fields")
    digest = closure_digest(closure)
    closure["closure_id"], closure["closure_sha256"] = f"{CLOSURE_PREFIX}{digest}", digest  # noqa: F405
    return validate_closure(closure)


def decode_closure(raw: bytes) -> dict[str, object]:
    return validate_closure(decode_canonical_json(raw, MAX_CLOSURE_BYTES))  # noqa: F405
