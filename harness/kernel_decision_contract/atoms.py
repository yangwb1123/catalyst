"""CognitiveAtom v2 strict validation, sealing, and decoding."""

from __future__ import annotations

import copy
import hashlib

from kernel_operational_contract.shape import bare_hash, bindings, exact_object, task_binding

from .codec import ContractError, canonical_json, decode_canonical_json
from .constants import *  # noqa: F403 - frozen contract vocabulary
from .shape import (atom_scope, attestations, declared_authority, proposition,
                    source, validity)


def _identity(atom: dict[str, object], allow_blank: bool) -> None:
    if allow_blank and atom["atom_id"] == atom["atom_sha256"] == "":
        return
    digest = bare_hash(atom["atom_sha256"], "atom_sha256")
    if atom["atom_id"] != f"{ATOM_PREFIX}{digest}":  # noqa: F405
        raise ContractError("atom_id must bind atom_sha256")


def _confidence(atom: dict[str, object]) -> None:
    value = atom["confidence_micros"]
    requires = atom["atom_type"] in {"assumption", "hypothesis", "inference"}
    valid = (isinstance(value, int) and not isinstance(value, bool) and
             0 <= value <= MAX_CONFIDENCE_MICROS)  # noqa: F405
    if requires != valid or (not requires and value is not None):
        raise ContractError("confidence_micros presence does not match atom_type")


def _hardness(atom: dict[str, object]) -> None:
    atom_type = atom["atom_type"]
    hardness = atom["declared_hardness"]
    authority_kind = atom["declared_authority"]["authority_kind"]
    legacy = atom["source"]["source_kind"] == "cognitive_atom_v1"
    if legacy:
        if hardness != "none" or authority_kind != "none":
            raise ContractError("cognitive_atom_v1 source preserves none hardness/authority")
        return
    if not isinstance(hardness, str) or hardness not in HARDNESS_BY_TYPE[atom_type]:  # noqa: F405
        raise ContractError("declared_hardness is not admitted by atom_type")
    if hardness == "none" and authority_kind != "none":
        raise ContractError("none hardness requires none authority")
    if hardness in {"contract", "invariant"} and authority_kind != "contract_artifact":
        raise ContractError("contract/invariant hardness requires a contract artifact")
    if hardness == "required" and authority_kind == "none":
        raise ContractError("required hardness requires a declared authority reference")
    if (hardness == "required" and atom_type == "decision" and
            authority_kind not in {"approval_record", "architecture_decision"}):
        raise ContractError("required Decision requires an ADR or Approval reference")


def _epistemic_state(atom: dict[str, object]) -> None:
    if atom["source"]["source_kind"] == "cognitive_atom_v1":
        allowed = LEGACY_STATES[atom["atom_type"]]  # noqa: F405
        if (not isinstance(atom["epistemic_state"], str) or
                atom["epistemic_state"] not in allowed):
            raise ContractError("legacy epistemic_state is outside ADR-0047 shadow states")
    elif atom["epistemic_state"] != "declared":
        raise ContractError("non-legacy CognitiveAtom epistemic_state must be declared")


def validate_cognitive_atom_shape(value: object, *,
                                  allow_blank: bool = False) -> dict[str, object]:
    """Validate exact fields and authority-neutral cognitive relations."""
    atom = exact_object(value, ATOM_FIELDS, "CognitiveAtom")  # noqa: F405
    constants = {"api_version": ATOM_API, "canonicalization": CANONICALIZATION,  # noqa: F405
                 "effective_hardness": "none", "kind": ATOM_KIND}  # noqa: F405
    for field, expected in constants.items():
        if atom[field] != expected:
            raise ContractError(f"{field} must be {expected!r}")
    if atom["instruction_allowed"] is not False:
        raise ContractError("instruction_allowed must be False")
    _identity(atom, allow_blank)
    if not isinstance(atom["atom_type"], str) or atom["atom_type"] not in ATOM_TYPES:  # noqa: F405
        raise ContractError("atom_type is unsupported")
    attestations(atom["attestations"])
    bindings(atom["bindings"])
    task = task_binding(atom["task_binding"])
    proposition_value = proposition(atom["proposition"])
    atom_scope(atom["scope"], task, proposition_value)
    declared_authority(atom["declared_authority"])
    source(atom["source"], atom["atom_type"])
    validity(atom["validity"])
    _confidence(atom)
    _hardness(atom)
    _epistemic_state(atom)
    return atom


def _blanked(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != ATOM_FIELDS:  # noqa: F405
        raise ContractError("CognitiveAtom digest input must have exact fields")
    result = copy.deepcopy(value)
    result["atom_id"], result["atom_sha256"] = "", ""
    return result


def cognitive_atom_digest(value: object) -> str:
    blank = _blanked(value)
    validate_cognitive_atom_shape(blank, allow_blank=True)
    raw = canonical_json(blank)
    if len(raw) > MAX_ATOM_BYTES:  # noqa: F405
        raise ContractError(f"CognitiveAtom blank preimage exceeds {MAX_ATOM_BYTES} bytes")  # noqa: F405
    return hashlib.sha256(ATOM_DOMAIN + raw).hexdigest()  # noqa: F405


def validate_cognitive_atom(value: object) -> dict[str, object]:
    atom = validate_cognitive_atom_shape(value)
    if len(canonical_json(atom)) > MAX_ATOM_BYTES:  # noqa: F405
        raise ContractError(f"CognitiveAtom exceeds {MAX_ATOM_BYTES} bytes")  # noqa: F405
    if atom["atom_sha256"] != cognitive_atom_digest(atom):
        raise ContractError("atom_sha256 does not match canonical preimage")
    return atom


def seal_cognitive_atom(value: object) -> dict[str, object]:
    atom = copy.deepcopy(value)
    if not isinstance(atom, dict) or atom.get("atom_id") != "" or atom.get("atom_sha256") != "":
        raise ContractError("sealing CognitiveAtom requires blank identity fields")
    digest = cognitive_atom_digest(atom)
    atom["atom_id"], atom["atom_sha256"] = f"{ATOM_PREFIX}{digest}", digest  # noqa: F405
    return validate_cognitive_atom(atom)


def decode_cognitive_atom(raw: bytes) -> dict[str, object]:
    return validate_cognitive_atom(decode_canonical_json(raw, MAX_ATOM_BYTES))  # noqa: F405
