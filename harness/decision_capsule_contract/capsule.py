"""DecisionCapsule validation, reseal comparison, and identity."""

from __future__ import annotations

import hashlib

from kernel_decision_contract import (
    canonical_json as decision_canonical_json,
    validate_closure as validate_decision_closure,
)
from kernel_decision_contract.constants import (
    MAX_CLOSURE_BYTES as DECISION_CLOSURE_MAX_BYTES,
)
from kernel_operational_contract.shape import bare_hash

from .codec import (ContractError, bounded_deepcopy, canonical_json,
                    decode_canonical_json, exact_object_shell,
                    require_constants, require_identity_strings,
                    validate_json_tree)
from .constants import *  # noqa: F403 - frozen vocabulary is intentionally central
from .manifest import (_derive_with_validated_closure, _local_preflight as
                       _validate_manifest_local,
                       _validate_projection, _validate_with_validated_closure)
from .shape import attestations


def _reseal_decision_closure(value: object) -> dict[str, object]:
    validate_json_tree(value, DECISION_CLOSURE_MAX_BYTES)
    closure = validate_decision_closure(value)
    candidate = bounded_deepcopy(closure, DECISION_CLOSURE_MAX_BYTES)
    candidate["closure_id"], candidate["closure_sha256"] = "", ""
    preimage = decision_canonical_json(candidate)
    domain = b"forgeos.kernel-decision-reference-closure.v1\0"
    digest = hashlib.sha256(domain + preimage).hexdigest()
    candidate["closure_id"] = f"kernel-decision-reference-closure-{digest}"
    candidate["closure_sha256"] = digest
    if candidate != closure:
        raise ContractError("embedded ADR-0090 closure differs after exact reseal")
    return closure


def _identity(capsule: dict[str, object], allow_blank: bool) -> None:
    if allow_blank and capsule["capsule_id"] == capsule["capsule_sha256"] == "":
        return
    digest = bare_hash(capsule["capsule_sha256"], "capsule_sha256")
    if capsule["capsule_id"] != f"{CAPSULE_PREFIX}{digest}":  # noqa: F405
        raise ContractError("capsule_id must bind capsule_sha256")


def _local_preflight(value: object, *, allow_blank: bool = False,
                     ignore_identity: bool = False) -> dict[str, object]:
    capsule = exact_object_shell(value, CAPSULE_FIELDS, "DecisionCapsule")  # noqa: F405
    constants = {"api_version": CAPSULE_API, "canonicalization": CANONICALIZATION,  # noqa: F405
                 "capsule_mode": CAPSULE_MODE, "kind": CAPSULE_KIND,  # noqa: F405
                 "result": CAPSULE_RESULT}  # noqa: F405
    require_constants(capsule, constants)
    require_identity_strings(capsule, "capsule_id", "capsule_sha256")
    if not ignore_identity:
        _identity(capsule, allow_blank)
    validate_json_tree(capsule["attestations"], MAX_CAPSULE_BYTES)  # noqa: F405
    attestations(capsule["attestations"])
    _validate_manifest_local(capsule["replay_manifest"])
    validate_json_tree(capsule, MAX_CAPSULE_BYTES)  # noqa: F405
    return capsule


def _base_shape(value: object, *, allow_blank: bool = False) -> dict[str, object]:
    return _local_preflight(value, allow_blank=allow_blank)


def _validate_components(value: object, *, allow_blank: bool = False,
                         ignore_identity: bool = False
                         ) -> tuple[dict[str, object], dict[str, object]]:
    capsule = _local_preflight(
        value, allow_blank=allow_blank, ignore_identity=ignore_identity)
    _validate_projection(capsule["replay_manifest"], capsule["decision_closure"])
    closure = _reseal_decision_closure(capsule["decision_closure"])
    _validate_with_validated_closure(capsule["replay_manifest"], closure)
    return capsule, closure


def _blanked(value: object) -> dict[str, object]:
    exact_object_shell(value, CAPSULE_FIELDS, "capsule digest input")  # noqa: F405
    validate_json_tree(value, MAX_CAPSULE_BYTES)  # noqa: F405
    result = bounded_deepcopy(value, MAX_CAPSULE_BYTES)  # noqa: F405
    result["capsule_id"], result["capsule_sha256"] = "", ""
    return result


def decision_capsule_digest(value: object) -> str:
    capsule, _ = _validate_components(value, ignore_identity=True)
    blank = _blanked(capsule)
    raw = canonical_json(blank)
    if len(raw) > MAX_CAPSULE_BYTES:  # noqa: F405
        raise ContractError("capsule blank preimage exceeds its byte ceiling")
    return hashlib.sha256(CAPSULE_DOMAIN + raw).hexdigest()  # noqa: F405


def validate_decision_capsule(value: object) -> dict[str, object]:
    capsule, _ = _validate_components(value)
    blank = _blanked(capsule)
    raw = canonical_json(blank)
    if len(canonical_json(capsule)) > MAX_CAPSULE_BYTES:  # noqa: F405
        raise ContractError("capsule exceeds its byte ceiling")
    digest = hashlib.sha256(CAPSULE_DOMAIN + raw).hexdigest()  # noqa: F405
    if capsule["capsule_sha256"] != digest:
        raise ContractError("capsule_sha256 does not match canonical preimage")
    return capsule


def seal_decision_capsule(value: object) -> dict[str, object]:
    if (type(value) is not dict or value.get("capsule_id") != "" or
            value.get("capsule_sha256") != ""):
        raise ContractError("sealing capsule requires blank own identity")
    shaped, _ = _validate_components(value, allow_blank=True)
    capsule = bounded_deepcopy(shaped, MAX_CAPSULE_BYTES)  # noqa: F405
    raw = canonical_json(capsule)
    if len(raw) > MAX_CAPSULE_BYTES:  # noqa: F405
        raise ContractError("capsule blank preimage exceeds its byte ceiling")
    digest = hashlib.sha256(CAPSULE_DOMAIN + raw).hexdigest()  # noqa: F405
    capsule["capsule_id"] = f"{CAPSULE_PREFIX}{digest}"  # noqa: F405
    capsule["capsule_sha256"] = digest
    if len(canonical_json(capsule)) > MAX_CAPSULE_BYTES:  # noqa: F405
        raise ContractError("capsule exceeds its sealed byte ceiling")
    _identity(capsule, False)
    return capsule


def derive_decision_capsule(decision_closure: object) -> dict[str, object]:
    closure = _reseal_decision_closure(decision_closure)
    candidate = {
        "api_version": CAPSULE_API, "attestations": {  # noqa: F405
            field: False for field in ATTESTATION_FIELDS},  # noqa: F405
        "canonicalization": CANONICALIZATION, "capsule_id": "",  # noqa: F405
        "capsule_mode": CAPSULE_MODE, "capsule_sha256": "",  # noqa: F405
        "decision_closure": bounded_deepcopy(
            closure, DECISION_CLOSURE_MAX_BYTES), "kind": CAPSULE_KIND,  # noqa: F405
        "replay_manifest": _derive_with_validated_closure(closure),
        "result": CAPSULE_RESULT,  # noqa: F405
    }
    _base_shape(candidate, allow_blank=True)
    raw = canonical_json(candidate)
    if len(raw) > MAX_CAPSULE_BYTES:  # noqa: F405
        raise ContractError("capsule blank preimage exceeds its byte ceiling")
    digest = hashlib.sha256(CAPSULE_DOMAIN + raw).hexdigest()  # noqa: F405
    candidate["capsule_id"] = f"{CAPSULE_PREFIX}{digest}"  # noqa: F405
    candidate["capsule_sha256"] = digest
    if len(canonical_json(candidate)) > MAX_CAPSULE_BYTES:  # noqa: F405
        raise ContractError("capsule exceeds its sealed byte ceiling")
    _identity(candidate, False)
    return candidate


def decode_decision_capsule(raw: bytes) -> dict[str, object]:
    return validate_decision_capsule(
        decode_canonical_json(raw, MAX_CAPSULE_BYTES))  # noqa: F405
