"""StructuralReplayManifest derivation, sealing, and validation."""

from __future__ import annotations

import hashlib

from kernel_decision_contract import validate_closure as validate_decision_closure
from kernel_decision_contract.constants import (
    MAX_CLOSURE_BYTES as DECISION_CLOSURE_MAX_BYTES,
)
from kernel_operational_contract.shape import bare_hash

from .codec import (ContractError, bounded_deepcopy, canonical_json,
                    decode_canonical_json, exact_object_shell,
                    require_constants, require_identity_strings,
                    validate_json_tree)
from .constants import *  # noqa: F403 - frozen vocabulary is intentionally central
from .shape import (artifact_receipt_refs, artifact_refs, atom_refs, attestations,
                    decision_closure_ref, event_refs, execution_refs,
                    invocation_refs, operational_closure_ref, transaction_ref)


def _record_ref(record: dict, id_field: str, hash_field: str) -> dict[str, object]:
    return {id_field: record[id_field], hash_field: record[hash_field]}


def _candidate_validated(closure: dict[str, object], *, copy_artifacts: bool = False
                         ) -> dict[str, object]:
    transaction, operational = closure["decision_transaction"], closure["operational_closure"]
    atoms = closure["cognitive_atoms"]
    artifacts = operational["artifacts"]
    if copy_artifacts:
        artifacts = bounded_deepcopy(artifacts, MAX_MANIFEST_BYTES)  # noqa: F405
    return {
        "api_version": MANIFEST_API,  # noqa: F405
        "artifact_receipt_refs": [_record_ref(item, "artifact_receipt_id",
                                              "artifact_receipt_sha256")
                                  for item in operational["artifact_receipts"]],
        "artifact_refs": artifacts,
        "attestations": {field: False for field in ATTESTATION_FIELDS},  # noqa: F405
        "canonicalization": CANONICALIZATION,  # noqa: F405
        "capability_invocation_refs": [_record_ref(item, "invocation_id",
                                                   "invocation_sha256")
                                       for item in operational["capability_invocations"]],
        "decision_closure_ref": _record_ref(closure, "closure_id", "closure_sha256"),
        "decision_transaction_ref": _record_ref(
            transaction, "decision_transaction_id", "decision_transaction_sha256"),
        "effect_replay_allowed": False,
        "execution_receipt_refs": [_record_ref(item, "execution_receipt_id",
                                               "execution_receipt_sha256")
                                   for item in operational["execution_receipts"]],
        "history_rewrite_allowed": False,
        "interaction_event_refs": [_record_ref(item, "event_id", "event_sha256")
                                   for item in operational["interaction_events"]],
        "kind": MANIFEST_KIND, "manifest_id": "", "manifest_sha256": "",  # noqa: F405
        "operational_closure_ref": _record_ref(
            operational, "closure_id", "closure_sha256"),
        "postdecision_atom_refs": [_record_ref(item, "atom_id", "atom_sha256")
                                   for item in atoms
                                   if item["source"]["source_phase"] == "postdecision"],
        "predecision_atom_refs": [_record_ref(item, "atom_id", "atom_sha256")
                                  for item in atoms
                                  if item["source"]["source_phase"] == "predecision"],
        "replay_mode": MANIFEST_MODE,  # noqa: F405
    }


def _identity(manifest: dict[str, object], allow_blank: bool) -> None:
    if allow_blank and manifest["manifest_id"] == manifest["manifest_sha256"] == "":
        return
    digest = bare_hash(manifest["manifest_sha256"], "manifest_sha256")
    if manifest["manifest_id"] != f"{MANIFEST_PREFIX}{digest}":  # noqa: F405
        raise ContractError("manifest_id must bind manifest_sha256")


def _local_preflight(value: object, *, allow_blank: bool = False,
                     ignore_identity: bool = False) -> dict[str, object]:
    manifest = exact_object_shell(  # noqa: F405
        value, MANIFEST_FIELDS, "StructuralReplayManifest")
    constants = {"api_version": MANIFEST_API, "canonicalization": CANONICALIZATION,  # noqa: F405
                 "kind": MANIFEST_KIND, "replay_mode": MANIFEST_MODE}  # noqa: F405
    require_constants(manifest, constants)
    if manifest["effect_replay_allowed"] is not False:
        raise ContractError("effect_replay_allowed must be false")
    if manifest["history_rewrite_allowed"] is not False:
        raise ContractError("history_rewrite_allowed must be false")
    require_identity_strings(manifest, "manifest_id", "manifest_sha256")
    validate_json_tree(manifest, MAX_MANIFEST_BYTES)  # noqa: F405
    if not ignore_identity:
        _identity(manifest, allow_blank)
    attestations(manifest["attestations"])
    decision_closure_ref(manifest["decision_closure_ref"])
    transaction_ref(manifest["decision_transaction_ref"])
    operational_closure_ref(manifest["operational_closure_ref"])
    predecision = atom_refs(
        manifest["predecision_atom_refs"], "predecision_atom_refs", 1)
    postdecision = atom_refs(
        manifest["postdecision_atom_refs"], "postdecision_atom_refs", 0)
    if len(predecision) + len(postdecision) > MAX_ATOMS:  # noqa: F405
        raise ContractError("combined decision atom refs exceed 256")
    atom_ids = [item["atom_id"] for item in predecision + postdecision]
    if len(atom_ids) != len(set(atom_ids)):
        raise ContractError("combined decision atom refs must be unique")
    artifact_refs(manifest["artifact_refs"], "artifact_refs", MAX_ARTIFACTS)  # noqa: F405
    artifact_receipt_refs(manifest["artifact_receipt_refs"])
    invocation_refs(manifest["capability_invocation_refs"])
    event_refs(manifest["interaction_event_refs"])
    execution_refs(manifest["execution_receipt_refs"])
    return manifest


def _shape(value: object, *, allow_blank: bool = False) -> dict[str, object]:
    return _local_preflight(value, allow_blank=allow_blank)


def _blanked(value: object) -> dict[str, object]:
    exact_object_shell(value, MANIFEST_FIELDS, "manifest digest input")  # noqa: F405
    validate_json_tree(value, MAX_MANIFEST_BYTES)  # noqa: F405
    result = bounded_deepcopy(value, MAX_MANIFEST_BYTES)  # noqa: F405
    result["manifest_id"], result["manifest_sha256"] = "", ""
    return result


def _validate_projection(manifest: dict[str, object], closure: dict[str, object]) -> None:
    try:
        expected = _candidate_validated(closure)
    except (KeyError, TypeError) as error:
        raise ContractError(
            "decision closure cannot supply the manifest projection") from error
    identity = {"manifest_id", "manifest_sha256"}
    if any(manifest[field] != expected[field] for field in MANIFEST_FIELDS - identity):  # noqa: F405
        raise ContractError("manifest must be the exact ordered projection of its closure")


def _validated_dependency(manifest: dict[str, object],
                          decision_closure: object) -> dict[str, object]:
    validate_json_tree(decision_closure, DECISION_CLOSURE_MAX_BYTES)
    _validate_projection(manifest, decision_closure)
    closure = validate_decision_closure(decision_closure)
    _validate_projection(manifest, closure)
    return closure


def _digest_validated(blank: dict[str, object]) -> str:
    raw = canonical_json(blank)
    if len(raw) > MAX_MANIFEST_BYTES:  # noqa: F405
        raise ContractError("manifest blank preimage exceeds its byte ceiling")
    return hashlib.sha256(MANIFEST_DOMAIN + raw).hexdigest()  # noqa: F405


def _validate_with_validated_closure(
        value: object, closure: dict[str, object], *, allow_blank: bool = False
        ) -> dict[str, object]:
    manifest = _shape(value, allow_blank=allow_blank)
    return _validate_shaped_with_validated_closure(
        manifest, closure, allow_blank=allow_blank)


def _validate_shaped_with_validated_closure(
        manifest: dict[str, object], closure: dict[str, object], *,
        allow_blank: bool = False) -> dict[str, object]:
    _validate_projection(manifest, closure)
    blank = _blanked(manifest)
    digest = _digest_validated(blank)
    if len(canonical_json(manifest)) > MAX_MANIFEST_BYTES:  # noqa: F405
        raise ContractError("manifest exceeds its sealed byte ceiling")
    if not allow_blank and manifest["manifest_sha256"] != digest:
        raise ContractError("manifest_sha256 does not match canonical preimage")
    return manifest


def _derive_with_validated_closure(closure: dict[str, object]) -> dict[str, object]:
    manifest = _candidate_validated(closure, copy_artifacts=True)
    _shape(manifest, allow_blank=True)
    digest = _digest_validated(manifest)
    manifest["manifest_id"] = f"{MANIFEST_PREFIX}{digest}"  # noqa: F405
    manifest["manifest_sha256"] = digest
    if len(canonical_json(manifest)) > MAX_MANIFEST_BYTES:  # noqa: F405
        raise ContractError("manifest exceeds its sealed byte ceiling")
    _identity(manifest, False)
    return manifest


def structural_replay_manifest_digest(value: object,
                                      decision_closure: object) -> str:
    manifest = _local_preflight(value, ignore_identity=True)
    _validated_dependency(manifest, decision_closure)
    blank = _blanked(manifest)
    _shape(blank, allow_blank=True)
    return _digest_validated(blank)


def validate_structural_replay_manifest(value: object,
                                        decision_closure: object) -> dict[str, object]:
    manifest = _shape(value)
    closure = _validated_dependency(manifest, decision_closure)
    return _validate_shaped_with_validated_closure(manifest, closure)


def seal_structural_replay_manifest(value: object,
                                    decision_closure: object) -> dict[str, object]:
    if (type(value) is not dict or value.get("manifest_id") != "" or
            value.get("manifest_sha256") != ""):
        raise ContractError("sealing manifest requires blank own identity")
    shaped = _shape(value, allow_blank=True)
    closure = _validated_dependency(shaped, decision_closure)
    _validate_shaped_with_validated_closure(shaped, closure, allow_blank=True)
    manifest = bounded_deepcopy(shaped, MAX_MANIFEST_BYTES)  # noqa: F405
    digest = _digest_validated(manifest)
    manifest["manifest_id"] = f"{MANIFEST_PREFIX}{digest}"  # noqa: F405
    manifest["manifest_sha256"] = digest
    if len(canonical_json(manifest)) > MAX_MANIFEST_BYTES:  # noqa: F405
        raise ContractError("manifest exceeds its sealed byte ceiling")
    _identity(manifest, False)
    return manifest


def derive_structural_replay_manifest(decision_closure: object) -> dict[str, object]:
    """Derive the only manifest admitted for one sealed ADR-0090 closure."""
    validate_json_tree(decision_closure, DECISION_CLOSURE_MAX_BYTES)
    closure = validate_decision_closure(decision_closure)
    return _derive_with_validated_closure(closure)


def decode_structural_replay_manifest(raw: bytes,
                                      decision_closure: object) -> dict[str, object]:
    value = decode_canonical_json(raw, MAX_MANIFEST_BYTES)  # noqa: F405
    return validate_structural_replay_manifest(value, decision_closure)
