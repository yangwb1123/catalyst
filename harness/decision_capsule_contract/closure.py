"""Outer StructuralReplayClosure sealing and validation."""

from __future__ import annotations

import hashlib

from kernel_operational_contract.shape import bare_hash

from .branch import (_derive_with_validated_capsule, _local_preflight as
                     _validate_branch_local,
                     _validate_comparison, _validate_with_validated_capsule)
from .capsule import (_local_preflight as _validate_capsule_local,
                      validate_decision_capsule)
from .codec import (ContractError, bounded_deepcopy, canonical_json,
                    decode_canonical_json, exact_object_shell,
                    require_constants, require_identity_strings,
                    validate_json_tree)
from .constants import *  # noqa: F403 - frozen vocabulary is intentionally central
from .shape import attestations, reflection_refs


def _identity(closure: dict[str, object], allow_blank: bool) -> None:
    if allow_blank and closure["closure_id"] == closure["closure_sha256"] == "":
        return
    digest = bare_hash(closure["closure_sha256"], "closure_sha256")
    if closure["closure_id"] != f"{CLOSURE_PREFIX}{digest}":  # noqa: F405
        raise ContractError("closure_id must bind closure_sha256")


def _local_preflight(value: object, *, allow_blank: bool = False,
                     ignore_identity: bool = False) -> dict[str, object]:
    closure = exact_object_shell(  # noqa: F405
        value, CLOSURE_FIELDS, "StructuralReplayClosure")
    constants = {"api_version": CLOSURE_API, "canonicalization": CANONICALIZATION,  # noqa: F405
                 "kind": CLOSURE_KIND, "result": SUCCESS_MARKER}  # noqa: F405
    require_constants(closure, constants)
    require_identity_strings(closure, "closure_id", "closure_sha256")
    if not ignore_identity:
        _identity(closure, allow_blank)
    validate_json_tree(closure["attestations"], MAX_CLOSURE_BYTES)  # noqa: F405
    attestations(closure["attestations"])
    validate_json_tree(  # noqa: F405
        closure["reflection_report_artifact_refs"], MAX_CLOSURE_BYTES)
    reflection_refs(closure["reflection_report_artifact_refs"])
    _validate_branch_local(closure["evaluation_branch"])
    _validate_capsule_local(closure["decision_capsule"])
    validate_json_tree(closure, MAX_CLOSURE_BYTES)  # noqa: F405
    return closure


def _shape(value: object, *, allow_blank: bool = False,
           ignore_identity: bool = False) -> dict[str, object]:
    closure = _local_preflight(
        value, allow_blank=allow_blank, ignore_identity=ignore_identity)
    _validate_comparison(
        closure["evaluation_branch"], closure["decision_capsule"])
    capsule = validate_decision_capsule(closure["decision_capsule"])
    _validate_with_validated_capsule(closure["evaluation_branch"], capsule)
    return closure


def _blanked(value: object) -> dict[str, object]:
    exact_object_shell(value, CLOSURE_FIELDS, "closure digest input")  # noqa: F405
    validate_json_tree(value, MAX_CLOSURE_BYTES)  # noqa: F405
    result = bounded_deepcopy(value, MAX_CLOSURE_BYTES)  # noqa: F405
    result["closure_id"], result["closure_sha256"] = "", ""
    return result


def structural_replay_closure_digest(value: object) -> str:
    closure = _shape(value, ignore_identity=True)
    blank = _blanked(closure)
    raw = canonical_json(blank)
    if len(raw) > MAX_CLOSURE_BYTES:  # noqa: F405
        raise ContractError("outer closure blank preimage exceeds its byte ceiling")
    return hashlib.sha256(CLOSURE_DOMAIN + raw).hexdigest()  # noqa: F405


def validate_structural_replay_closure(value: object) -> dict[str, object]:
    closure = _shape(value)
    blank = _blanked(closure)
    raw = canonical_json(blank)
    if len(canonical_json(closure)) > MAX_CLOSURE_BYTES:  # noqa: F405
        raise ContractError("outer closure exceeds its byte ceiling")
    digest = hashlib.sha256(CLOSURE_DOMAIN + raw).hexdigest()  # noqa: F405
    if closure["closure_sha256"] != digest:
        raise ContractError("closure_sha256 does not match canonical preimage")
    return closure


def seal_structural_replay_closure(value: object) -> dict[str, object]:
    if (type(value) is not dict or value.get("closure_id") != "" or
            value.get("closure_sha256") != ""):
        raise ContractError("sealing outer closure requires blank own identity")
    shaped = _shape(value, allow_blank=True)
    closure = bounded_deepcopy(shaped, MAX_CLOSURE_BYTES)  # noqa: F405
    raw = canonical_json(closure)
    if len(raw) > MAX_CLOSURE_BYTES:  # noqa: F405
        raise ContractError("outer closure blank preimage exceeds its byte ceiling")
    digest = hashlib.sha256(CLOSURE_DOMAIN + raw).hexdigest()  # noqa: F405
    closure["closure_id"] = f"{CLOSURE_PREFIX}{digest}"  # noqa: F405
    closure["closure_sha256"] = digest
    if len(canonical_json(closure)) > MAX_CLOSURE_BYTES:  # noqa: F405
        raise ContractError("outer closure exceeds its sealed byte ceiling")
    _identity(closure, False)
    return closure


def derive_structural_replay_closure(
        capsule: object, reflection_report_artifact_refs: object) -> dict[str, object]:
    validate_json_tree(
        reflection_report_artifact_refs, MAX_CLOSURE_BYTES)  # noqa: F405
    refs = reflection_refs(reflection_report_artifact_refs)
    _validate_capsule_local(capsule)
    validated = validate_decision_capsule(capsule)
    candidate = {
        "api_version": CLOSURE_API, "attestations": {  # noqa: F405
            field: False for field in ATTESTATION_FIELDS},  # noqa: F405
        "canonicalization": CANONICALIZATION, "closure_id": "",  # noqa: F405
        "closure_sha256": "", "decision_capsule": bounded_deepcopy(
            validated, MAX_CAPSULE_BYTES),  # noqa: F405
        "evaluation_branch": _derive_with_validated_capsule(validated),
        "kind": CLOSURE_KIND,  # noqa: F405
        "reflection_report_artifact_refs": bounded_deepcopy(
            refs, MAX_CLOSURE_BYTES),  # noqa: F405
        "result": SUCCESS_MARKER,  # noqa: F405
    }
    raw = canonical_json(candidate)
    if len(raw) > MAX_CLOSURE_BYTES:  # noqa: F405
        raise ContractError("outer closure blank preimage exceeds its byte ceiling")
    digest = hashlib.sha256(CLOSURE_DOMAIN + raw).hexdigest()  # noqa: F405
    candidate["closure_id"] = f"{CLOSURE_PREFIX}{digest}"  # noqa: F405
    candidate["closure_sha256"] = digest
    if len(canonical_json(candidate)) > MAX_CLOSURE_BYTES:  # noqa: F405
        raise ContractError("outer closure exceeds its sealed byte ceiling")
    _identity(candidate, False)
    return candidate


def decode_structural_replay_closure(raw: bytes) -> dict[str, object]:
    value = decode_canonical_json(raw, MAX_CLOSURE_BYTES)  # noqa: F405
    return validate_structural_replay_closure(value)
