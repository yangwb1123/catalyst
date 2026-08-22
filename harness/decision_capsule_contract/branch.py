"""Separately sealed compare-only EvaluationBranch v1."""

from __future__ import annotations

import hashlib

from kernel_operational_contract.shape import bare_hash

from .capsule import (_local_preflight as _validate_capsule_local,
                      validate_decision_capsule)
from .codec import (ContractError, bounded_deepcopy, canonical_json,
                    decode_canonical_json, exact_object_shell,
                    require_constants, require_identity_strings,
                    validate_json_tree)
from .constants import *  # noqa: F403 - frozen vocabulary is intentionally central
from .shape import (attestations, capsule_ref, decision_closure_ref,
                    manifest_ref)


def _record_ref(record: dict, id_field: str, hash_field: str) -> dict[str, object]:
    return {id_field: record[id_field], hash_field: record[hash_field]}


def _identity(branch: dict[str, object], allow_blank: bool) -> None:
    if allow_blank and branch["branch_id"] == branch["branch_sha256"] == "":
        return
    digest = bare_hash(branch["branch_sha256"], "branch_sha256")
    if branch["branch_id"] != f"{BRANCH_PREFIX}{digest}":  # noqa: F405
        raise ContractError("branch_id must bind branch_sha256")


def _local_preflight(value: object, *, allow_blank: bool = False,
                     ignore_identity: bool = False) -> dict[str, object]:
    branch = exact_object_shell(value, BRANCH_FIELDS, "EvaluationBranch")  # noqa: F405
    constants = {"api_version": BRANCH_API, "branch_mode": BRANCH_MODE,  # noqa: F405
                 "canonicalization": CANONICALIZATION,  # noqa: F405
                 "comparison_result": COMPARISON_RESULT, "kind": BRANCH_KIND}  # noqa: F405
    require_constants(branch, constants)
    if branch["effect_replay_allowed"] is not False:
        raise ContractError("effect_replay_allowed must be false")
    if branch["history_rewrite_allowed"] is not False:
        raise ContractError("history_rewrite_allowed must be false")
    require_identity_strings(branch, "branch_id", "branch_sha256")
    validate_json_tree(branch, MAX_BRANCH_BYTES)  # noqa: F405
    if not ignore_identity:
        _identity(branch, allow_blank)
    attestations(branch["attestations"])
    capsule_ref(branch["capsule_ref"])
    decision_closure_ref(branch["decision_closure_ref"])
    manifest_ref(branch["manifest_ref"])
    return branch


def _shape(value: object, *, allow_blank: bool = False) -> dict[str, object]:
    return _local_preflight(value, allow_blank=allow_blank)


def _blanked(value: object) -> dict[str, object]:
    exact_object_shell(value, BRANCH_FIELDS, "branch digest input")  # noqa: F405
    validate_json_tree(value, MAX_BRANCH_BYTES)  # noqa: F405
    result = bounded_deepcopy(value, MAX_BRANCH_BYTES)  # noqa: F405
    result["branch_id"], result["branch_sha256"] = "", ""
    return result


def _candidate_validated(capsule: dict[str, object]) -> dict[str, object]:
    closure, manifest = capsule["decision_closure"], capsule["replay_manifest"]
    return {
        "api_version": BRANCH_API, "attestations": {  # noqa: F405
            field: False for field in ATTESTATION_FIELDS},  # noqa: F405
        "branch_id": "", "branch_mode": BRANCH_MODE, "branch_sha256": "",  # noqa: F405
        "canonicalization": CANONICALIZATION,  # noqa: F405
        "capsule_ref": _record_ref(capsule, "capsule_id", "capsule_sha256"),
        "comparison_result": COMPARISON_RESULT,  # noqa: F405
        "decision_closure_ref": _record_ref(closure, "closure_id", "closure_sha256"),
        "effect_replay_allowed": False, "history_rewrite_allowed": False,
        "kind": BRANCH_KIND,  # noqa: F405
        "manifest_ref": _record_ref(manifest, "manifest_id", "manifest_sha256"),
    }


def _validate_comparison(branch: dict[str, object], capsule: dict[str, object]) -> None:
    try:
        expected = _candidate_validated(capsule)
    except (KeyError, TypeError) as error:
        raise ContractError(
            "decision capsule cannot supply the branch comparison") from error
    identity = {"branch_id", "branch_sha256"}
    if any(branch[field] != expected[field] for field in BRANCH_FIELDS - identity):  # noqa: F405
        raise ContractError("branch must be the unique structural comparison for its capsule")


def _validated_dependency(branch: dict[str, object],
                          capsule: object) -> dict[str, object]:
    supplied = _validate_capsule_local(capsule)
    _validate_comparison(branch, supplied)
    validated = validate_decision_capsule(capsule)
    _validate_comparison(branch, validated)
    return validated


def _digest_validated(blank: dict[str, object]) -> str:
    raw = canonical_json(blank)
    if len(raw) > MAX_BRANCH_BYTES:  # noqa: F405
        raise ContractError("branch blank preimage exceeds its byte ceiling")
    return hashlib.sha256(BRANCH_DOMAIN + raw).hexdigest()  # noqa: F405


def _validate_with_validated_capsule(
        value: object, capsule: dict[str, object], *, allow_blank: bool = False
        ) -> dict[str, object]:
    branch = _shape(value, allow_blank=allow_blank)
    return _validate_shaped_with_validated_capsule(
        branch, capsule, allow_blank=allow_blank)


def _validate_shaped_with_validated_capsule(
        branch: dict[str, object], capsule: dict[str, object], *,
        allow_blank: bool = False) -> dict[str, object]:
    _validate_comparison(branch, capsule)
    blank = _blanked(branch)
    digest = _digest_validated(blank)
    if len(canonical_json(branch)) > MAX_BRANCH_BYTES:  # noqa: F405
        raise ContractError("branch exceeds its sealed byte ceiling")
    if not allow_blank and branch["branch_sha256"] != digest:
        raise ContractError("branch_sha256 does not match canonical preimage")
    return branch


def _derive_with_validated_capsule(capsule: dict[str, object]) -> dict[str, object]:
    branch = _candidate_validated(capsule)
    _shape(branch, allow_blank=True)
    digest = _digest_validated(branch)
    branch["branch_id"] = f"{BRANCH_PREFIX}{digest}"  # noqa: F405
    branch["branch_sha256"] = digest
    if len(canonical_json(branch)) > MAX_BRANCH_BYTES:  # noqa: F405
        raise ContractError("branch exceeds its sealed byte ceiling")
    _identity(branch, False)
    return branch


def evaluation_branch_digest(value: object, capsule: object) -> str:
    branch = _local_preflight(value, ignore_identity=True)
    validated = _validated_dependency(branch, capsule)
    blank = _blanked(branch)
    _shape(blank, allow_blank=True)
    _validate_comparison(blank, validated)
    return _digest_validated(blank)


def validate_evaluation_branch(value: object,
                               capsule: object) -> dict[str, object]:
    branch = _local_preflight(value)
    validated = _validated_dependency(branch, capsule)
    validate_json_tree(branch, MAX_BRANCH_BYTES)  # noqa: F405
    return _validate_shaped_with_validated_capsule(branch, validated)


def seal_evaluation_branch(value: object,
                           capsule: object) -> dict[str, object]:
    if (type(value) is not dict or value.get("branch_id") != "" or
            value.get("branch_sha256") != ""):
        raise ContractError("sealing branch requires blank own identity")
    shaped = _local_preflight(value, allow_blank=True)
    validated = _validated_dependency(shaped, capsule)
    validate_json_tree(shaped, MAX_BRANCH_BYTES)  # noqa: F405
    _validate_shaped_with_validated_capsule(shaped, validated, allow_blank=True)
    branch = bounded_deepcopy(shaped, MAX_BRANCH_BYTES)  # noqa: F405
    digest = _digest_validated(branch)
    branch["branch_id"] = f"{BRANCH_PREFIX}{digest}"  # noqa: F405
    branch["branch_sha256"] = digest
    if len(canonical_json(branch)) > MAX_BRANCH_BYTES:  # noqa: F405
        raise ContractError("branch exceeds its sealed byte ceiling")
    _identity(branch, False)
    return branch


def derive_evaluation_branch(capsule: object) -> dict[str, object]:
    _validate_capsule_local(capsule)
    validated = validate_decision_capsule(capsule)
    return _derive_with_validated_capsule(validated)


def decode_evaluation_branch(raw: bytes,
                             capsule: object) -> dict[str, object]:
    value = decode_canonical_json(raw, MAX_BRANCH_BYTES)  # noqa: F405
    return validate_evaluation_branch(value, capsule)
