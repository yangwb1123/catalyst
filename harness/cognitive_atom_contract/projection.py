"""Strict CognitiveAtom v1 shape, digest, closure and shadow projection."""

from __future__ import annotations

import hashlib

from governance_contract import (ContractError, canonical_json, decode_record_set,
                                 validate_record_set)
from governance_contract.codec import decode_json
from governance_contract.shape import digest, enum, id_list, identifier, integer

from .constants import (API_VERSION, ATOM_DOMAIN, ATOM_ID_DOMAIN, ATOM_ID_RE,
                        ATOM_SET_DOMAIN, ATOM_TYPES, CANONICALIZATION, HASH_RE, ID_RE,
                        INTEGRITY_FIELDS,
                        KIND, MAX_ATOM_BYTES, MAX_ATOM_SET_BYTES, MAX_ATOMS,
                        METADATA_FIELDS, PROPOSITION_FIELDS, SHADOW_STATES,
                        SOURCE_FIELDS, SOURCE_SET_DOMAIN, SPEC_FIELDS, TOP_FIELDS,
                        VALIDITY_FIELDS)


def _exact_fields(value: object, fields: set[str], label: str,
                  issues: list[str]) -> bool:
    if not isinstance(value, dict):
        issues.append(f"{label}: expected object")
        return False
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown:
        issues.append(f"{label}: unknown fields {sorted(unknown)}")
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return not unknown and not missing


def canonical_atom_payload(atom: dict[str, object]) -> bytes:
    """Canonicalize one atom after blanking its self digest."""
    integrity = atom.get("integrity")
    if not isinstance(integrity, dict):
        raise ContractError("atom.integrity must be an object")
    payload = {**atom, "integrity": {**integrity, "canonical_sha256": ""}}
    encoded = canonical_json(payload)
    if len(encoded) + 64 > MAX_ATOM_BYTES:
        raise ContractError(f"atom exceeds {MAX_ATOM_BYTES} bytes")
    return encoded


def compute_atom_digest(atom: dict[str, object]) -> str:
    """Return the domain-separated digest for a CognitiveAtom."""
    if atom.get("kind") != KIND:
        raise ContractError(f"unsupported atom kind {atom.get('kind')!r}")
    return hashlib.sha256(ATOM_DOMAIN + canonical_atom_payload(atom)).hexdigest()


def compute_atom_set_digest(atoms: list[dict[str, object]]) -> str:
    """Return the digest of the sealed, sorted canonical atom set."""
    encoded = canonical_json(atoms)
    if len(encoded) > MAX_ATOM_SET_BYTES:
        raise ContractError(f"cognitive atom set exceeds {MAX_ATOM_SET_BYTES} bytes")
    return hashlib.sha256(ATOM_SET_DOMAIN + encoded).hexdigest()


def _derive_atom_id(task_id: str, claim_digest: str, context_digest: str,
                    policy_digest: str, source_tree_digest: str,
                    source_revision: str) -> str:
    task_bytes = task_id.encode("utf-8")
    revision_bytes = source_revision.encode("utf-8")
    framed = (len(task_bytes).to_bytes(8, "big") + task_bytes +
              bytes.fromhex(claim_digest) + bytes.fromhex(context_digest) +
              bytes.fromhex(policy_digest) + bytes.fromhex(source_tree_digest) +
              len(revision_bytes).to_bytes(8, "big") + revision_bytes)
    return "atom-" + hashlib.sha256(ATOM_ID_DOMAIN + framed).hexdigest()


def compute_atom_id(task_id: str, claim: dict[str, object]) -> str:
    """Derive the stable projection identity using frozen length framing."""
    metadata = claim["metadata"]
    return _derive_atom_id(task_id, claim["integrity"]["canonical_sha256"],
                           metadata["context_sha256"], metadata["policy_sha256"],
                           metadata["source_tree_sha256"], metadata["source_revision"])


def _reference_ids(record: dict[str, object]) -> list[str]:
    metadata = record["metadata"]
    references = list(metadata["supersedes_record_ids"])
    if record["kind"] == "KnowledgeClaim":
        spec = record["spec"]
        for field in ("supporting_evidence_record_ids",
                      "contradicting_evidence_record_ids",
                      "derived_from_claim_record_ids"):
            references.extend(spec[field])
    return references


def source_closure(claim: dict[str, object],
                   records: list[dict[str, object]]) -> tuple[list[dict[str, object]], bytes, str]:
    """Build the minimal transitive source closure, sorted by record_id."""
    by_id = {record["metadata"]["record_id"]: record for record in records}
    pending = [claim["metadata"]["record_id"]]
    selected: set[str] = set()
    while pending:
        record_id = pending.pop()
        if record_id in selected:
            continue
        record = by_id.get(record_id)
        if record is None:
            raise ContractError(f"source closure references unknown record {record_id!r}")
        selected.add(record_id)
        pending.extend(_reference_ids(record))
    closure = [by_id[record_id] for record_id in sorted(selected)]
    encoded = canonical_json(closure)
    if len(encoded) > MAX_ATOM_SET_BYTES:
        raise ContractError(f"source closure exceeds {MAX_ATOM_SET_BYTES} bytes")
    closure_digest = hashlib.sha256(SOURCE_SET_DOMAIN + encoded).hexdigest()
    return closure, encoded, closure_digest


def _project_metadata(task_id: str, claim: dict[str, object]) -> dict[str, object]:
    metadata = claim["metadata"]
    return {
        "atom_id": compute_atom_id(task_id, claim),
        "context_sha256": metadata["context_sha256"],
        "policy_sha256": metadata["policy_sha256"],
        "project_id": metadata["project_id"], "scope": metadata["scope"],
        "source_revision": metadata["source_revision"],
        "source_tree_sha256": metadata["source_tree_sha256"], "task_id": task_id,
    }


def _project_source(claim: dict[str, object], records: list[dict[str, object]]) -> dict[str, object]:
    metadata = claim["metadata"]
    closure, closure_bytes, closure_digest = source_closure(claim, records)
    return {
        "canonical_sha256": claim["integrity"]["canonical_sha256"],
        "claim_aggregate_id": metadata["aggregate_id"],
        "claim_record_id": metadata["record_id"], "claim_sequence": metadata["sequence"],
        "closure_byte_count": len(closure_bytes), "closure_record_count": len(closure),
        "closure_sha256": closure_digest, "record_kind": "KnowledgeClaim",
    }


def _project_spec(claim: dict[str, object]) -> dict[str, object]:
    spec, status = claim["spec"], claim["status"]
    confidence = (spec["confidence_micros"] if spec["claim_type"] in
                  {"assumption", "hypothesis", "inference"} else None)
    return {
        "atom_type": spec["claim_type"], "authority_ref": None,
        "contradicting_evidence_record_ids": spec["contradicting_evidence_record_ids"],
        "derived_from_claim_record_ids": spec["derived_from_claim_record_ids"],
        "epistemic_state": status["state"], "hardness": "none",
        "instruction_allowed": False, "projection_confidence_micros": confidence,
        "projection_mode": "shadow",
        "proposition": {
            "object_type": spec["object_type"], "object_value": spec["object_value"],
            "predicate": spec["predicate"], "subject": spec["subject"],
        },
        "supporting_evidence_record_ids": spec["supporting_evidence_record_ids"],
        "validity": {"valid_from_unix_ms": status["valid_from_unix_ms"],
                     "valid_until_unix_ms": status["valid_until_unix_ms"]},
    }


def _project_one(task_id: str, claim: dict[str, object],
                 records: list[dict[str, object]]) -> dict[str, object]:
    atom: dict[str, object] = {
        "api_version": API_VERSION,
        "integrity": {"canonical_sha256": "", "canonicalization": CANONICALIZATION},
        "kind": KIND, "metadata": _project_metadata(task_id, claim),
        "source": _project_source(claim, records), "spec": _project_spec(claim),
    }
    atom["integrity"]["canonical_sha256"] = compute_atom_digest(atom)
    return atom


def project_atom_set(task_id: str,
                     records: list[dict[str, object]]) -> list[dict[str, object]]:
    """Project valid governance claims without adding authority or hardness."""
    task_issues: list[str] = []
    identifier(task_id, "task_id", task_issues)
    source_issues = validate_record_set(records)
    if task_issues or source_issues:
        raise ContractError("; ".join(task_issues + source_issues))
    claims = [record for record in records if record["kind"] == "KnowledgeClaim" and
              record["spec"]["claim_type"] in ATOM_TYPES]
    if not claims:
        raise ContractError("source record set contains no projectable CognitiveAtom claims")
    atoms = [_project_one(task_id, claim, records) for claim in claims]
    atoms.sort(key=lambda atom: atom["metadata"]["atom_id"])
    encoded = canonical_json(atoms)
    if len(atoms) > MAX_ATOMS:
        raise ContractError(f"cognitive atom set exceeds {MAX_ATOMS} atoms")
    if len(encoded) > MAX_ATOM_SET_BYTES:
        raise ContractError(f"cognitive atom set exceeds {MAX_ATOM_SET_BYTES} bytes")
    return atoms


def project_atom_set_bytes(task_id: str, source_raw: bytes) -> bytes:
    """Decode a canonical source record set and return canonical atom-set bytes."""
    records = decode_record_set(source_raw)
    issues = validate_record_set(records)
    if issues:
        raise ContractError("; ".join(issues))
    return canonical_json(project_atom_set(task_id, records))


def _validate_atom(atom: dict[str, object], index: int, issues: list[str]) -> None:
    label = f"atoms[{index}]"
    if not _exact_fields(atom, TOP_FIELDS, label, issues):
        return
    if atom["api_version"] != API_VERSION:
        issues.append(f"{label}.api_version: unsupported version")
    if atom["kind"] != KIND:
        issues.append(f"{label}.kind: unsupported kind")
    integrity = atom["integrity"]
    if _exact_fields(integrity, INTEGRITY_FIELDS, f"{label}.integrity", issues):
        digest(integrity["canonical_sha256"], f"{label}.integrity.canonical_sha256", issues)
        if integrity["canonicalization"] != CANONICALIZATION:
            issues.append(f"{label}.integrity.canonicalization: unsupported format")
    metadata = atom["metadata"]
    if _exact_fields(metadata, METADATA_FIELDS, f"{label}.metadata", issues):
        for field in {"project_id", "scope", "source_revision", "task_id"}:
            identifier(metadata[field], f"{label}.metadata.{field}", issues)
        if not isinstance(metadata["atom_id"], str) or ATOM_ID_RE.fullmatch(metadata["atom_id"]) is None:
            issues.append(f"{label}.metadata.atom_id: expected atom- followed by lowercase SHA-256")
        for field in {"context_sha256", "policy_sha256", "source_tree_sha256"}:
            digest(metadata[field], f"{label}.metadata.{field}", issues)
        _validate_embedded_atom_id(atom, label, issues)
    _validate_source(atom["source"], label, issues)
    _validate_spec(atom["spec"], label, issues)
    try:
        if len(canonical_json(atom)) > MAX_ATOM_BYTES:
            issues.append(f"{label}: atom exceeds {MAX_ATOM_BYTES} bytes")
        if isinstance(integrity, dict) and integrity.get("canonical_sha256") != compute_atom_digest(atom):
            issues.append(f"{label}.integrity.canonical_sha256: digest mismatch")
    except ContractError as error:
        issues.append(f"{label}: cannot compute digest: {error}")


def _validate_source(value: object, label: str, issues: list[str]) -> None:
    source_label = f"{label}.source"
    if not _exact_fields(value, SOURCE_FIELDS, source_label, issues):
        return
    digest(value["canonical_sha256"], f"{source_label}.canonical_sha256", issues)
    digest(value["closure_sha256"], f"{source_label}.closure_sha256", issues)
    for field in {"claim_aggregate_id", "claim_record_id"}:
        identifier(value[field], f"{source_label}.{field}", issues)
    integer(value["claim_sequence"], f"{source_label}.claim_sequence", issues, 1)
    integer(value["closure_byte_count"], f"{source_label}.closure_byte_count", issues, 1,
            MAX_ATOM_SET_BYTES)
    integer(value["closure_record_count"], f"{source_label}.closure_record_count", issues, 1,
            MAX_ATOMS)
    if value["record_kind"] != "KnowledgeClaim":
        issues.append(f"{source_label}.record_kind: must be KnowledgeClaim")


def _validate_embedded_atom_id(atom: dict[str, object], label: str,
                               issues: list[str]) -> None:
    metadata, source = atom["metadata"], atom["source"]
    if not isinstance(metadata, dict) or not isinstance(source, dict):
        return
    required = [metadata.get("task_id"), source.get("canonical_sha256"),
                metadata.get("context_sha256"), metadata.get("policy_sha256"),
                metadata.get("source_tree_sha256"), metadata.get("source_revision")]
    if not all(isinstance(item, str) for item in required):
        return
    if not all(HASH_RE.fullmatch(item) for item in required[1:5]):
        return
    expected = _derive_atom_id(*required)
    if metadata.get("atom_id") != expected:
        issues.append(f"{label}.metadata.atom_id: does not match embedded source/metadata identity")


def _validate_proposition(value: object, label: str, issues: list[str]) -> None:
    if not _exact_fields(value, PROPOSITION_FIELDS, label, issues):
        return
    object_type, object_value = value["object_type"], value["object_value"]
    enum(object_type, {"artifact_ref", "boolean", "integer", "null", "string"},
         f"{label}.object_type", issues)
    for field in {"predicate", "subject"}:
        identifier(value[field], f"{label}.{field}", issues)
    matches = {
        "artifact_ref": isinstance(object_value, str) and ID_RE.fullmatch(object_value) is not None,
        "boolean": isinstance(object_value, bool),
        "integer": type(object_value) is int,
        "null": object_value is None,
        "string": isinstance(object_value, str),
    }
    if isinstance(object_type, str) and object_type in matches and not matches[object_type]:
        issues.append(f"{label}.object_value: does not match object_type {object_type!r}")


def _validate_spec(value: object, label: str, issues: list[str]) -> None:
    spec_label = f"{label}.spec"
    if not _exact_fields(value, SPEC_FIELDS, spec_label, issues):
        return
    enum(value["atom_type"], set(ATOM_TYPES), f"{spec_label}.atom_type", issues)
    atom_type = value["atom_type"]
    states = SHADOW_STATES.get(atom_type, set()) if isinstance(atom_type, str) else set()
    enum(value["epistemic_state"], states, f"{spec_label}.epistemic_state", issues)
    if value["authority_ref"] is not None:
        issues.append(f"{spec_label}.authority_ref: must be null in shadow projection")
    if value["hardness"] != "none":
        issues.append(f"{spec_label}.hardness: must be none in shadow projection")
    if value["instruction_allowed"] is not False:
        issues.append(f"{spec_label}.instruction_allowed: must be false")
    if value["projection_mode"] != "shadow":
        issues.append(f"{spec_label}.projection_mode: must be shadow")
    confidence = value["projection_confidence_micros"]
    if atom_type in {"assumption", "hypothesis", "inference"}:
        integer(confidence, f"{spec_label}.projection_confidence_micros", issues, 0, 1_000_000)
    elif confidence is not None:
        issues.append(f"{spec_label}.projection_confidence_micros: must be null")
    for field in {"supporting_evidence_record_ids", "contradicting_evidence_record_ids",
                  "derived_from_claim_record_ids"}:
        id_list(value[field], f"{spec_label}.{field}", issues)
    support, contradict = (value["supporting_evidence_record_ids"],
                           value["contradicting_evidence_record_ids"])
    if (isinstance(support, list) and isinstance(contradict, list) and
            all(isinstance(item, str) for item in support + contradict) and
            set(support) & set(contradict)):
        issues.append(f"{spec_label}: supporting and contradicting evidence must be disjoint")
    _validate_proposition(value["proposition"], f"{spec_label}.proposition", issues)
    validity = value["validity"]
    if _exact_fields(validity, VALIDITY_FIELDS, f"{spec_label}.validity", issues):
        integer(validity["valid_from_unix_ms"], f"{spec_label}.validity.valid_from_unix_ms",
                issues, 0)
        integer(validity["valid_until_unix_ms"], f"{spec_label}.validity.valid_until_unix_ms",
                issues, 0, nullable=True)
        start, end = validity["valid_from_unix_ms"], validity["valid_until_unix_ms"]
        if type(start) is int and type(end) is int and end <= start:
            issues.append(f"{spec_label}.validity: valid_until_unix_ms must be greater than "
                          "valid_from_unix_ms")


def decode_atom_set(raw: bytes) -> list[dict[str, object]]:
    """Decode an exact compact canonical bounded CognitiveAtom set."""
    if len(raw) > MAX_ATOM_SET_BYTES:
        raise ContractError(f"cognitive atom set exceeds {MAX_ATOM_SET_BYTES} bytes")
    value = decode_json(raw)
    if not isinstance(value, list) or not value:
        raise ContractError("cognitive atom set must be a non-empty JSON array")
    if len(value) > MAX_ATOMS:
        raise ContractError(f"cognitive atom set exceeds {MAX_ATOMS} atoms")
    if not all(isinstance(atom, dict) for atom in value):
        raise ContractError("cognitive atom set entries must be JSON objects")
    if canonical_json(value) != raw:
        raise ContractError("cognitive atom set is not exact compact canonical JSON")
    return value


def validate_atom_set(atoms: list[dict[str, object]]) -> list[str]:
    """Validate structural atom properties without attesting source truth."""
    if not isinstance(atoms, list) or not atoms or len(atoms) > MAX_ATOMS:
        return [f"cognitive atom set must contain 1..{MAX_ATOMS} atoms"]
    issues: list[str] = []
    for index, atom in enumerate(atoms):
        if isinstance(atom, dict):
            _validate_atom(atom, index, issues)
        else:
            issues.append(f"atoms[{index}]: expected object")
    atom_ids = [atom.get("metadata", {}).get("atom_id") for atom in atoms
                if isinstance(atom, dict) and isinstance(atom.get("metadata"), dict)]
    if len(atom_ids) != len(atoms) or atom_ids != sorted(set(atom_ids)):
        issues.append("cognitive atom set must be uniquely sorted by atom_id")
    try:
        if len(canonical_json(atoms)) > MAX_ATOM_SET_BYTES:
            issues.append(f"cognitive atom set exceeds {MAX_ATOM_SET_BYTES} bytes")
    except ContractError as error:
        issues.append(str(error))
    return issues


def check_projection_bytes(task_id: str, source_raw: bytes, atom_raw: bytes) -> list[str]:
    """Strictly reproject source bytes and compare the atom-set bytes exactly."""
    try:
        records = decode_record_set(source_raw)
        source_issues = validate_record_set(records)
        if source_issues:
            return [f"source: {issue}" for issue in source_issues]
        atoms = decode_atom_set(atom_raw)
        atom_issues = validate_atom_set(atoms)
        if atom_issues:
            return atom_issues
        expected = canonical_json(project_atom_set(task_id, records))
        if atom_raw != expected:
            return ["cognitive atom set does not exactly match deterministic shadow projection"]
        return []
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["cognitive atom projection processing exhausted memory"]


def project_record_set(source_records: list[dict[str, object]],
                       task_id: str) -> list[dict[str, object]]:
    """Compatibility facade using source-first argument order."""
    return project_atom_set(task_id, source_records)


def validate_projection(source_records: list[dict[str, object]], task_id: str,
                        atoms: list[dict[str, object]]) -> list[str]:
    """Validate structural atoms and exact deterministic source correspondence."""
    source_issues = validate_record_set(source_records)
    if source_issues:
        return [f"source: {issue}" for issue in source_issues]
    atom_issues = validate_atom_set(atoms)
    if atom_issues:
        return atom_issues
    try:
        expected = project_atom_set(task_id, source_records)
        if canonical_json(atoms) != canonical_json(expected):
            return ["cognitive atom set does not exactly match deterministic shadow projection"]
        return []
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["cognitive atom projection processing exhausted memory"]
