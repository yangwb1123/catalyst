"""KnowledgeUpdateProposal identity, record closure, and declared target."""

from __future__ import annotations

import copy
from typing import Any

from governance_contract import validate_record_set

from .canonical import (ContractError, bounded_canonical_json, bounded_digest,
                        decode_canonical)
from .closure import record_index, validate_exact_closure
from .constants import (CANONICALIZATION, MAX_PROPOSAL_BYTES, MAX_RECORD_SET_BYTES,
                        MAX_RECORDS, MAX_TARGET_BYTES, PROPOSAL_API, PROPOSAL_DOMAIN,
                        PROPOSAL_KIND, RECORD_SET_DOMAIN, TARGET_DOMAIN)
from .lifecycle import validate_lifecycle, validate_mutation_shapes
from .shape import (integer, require_keys, sha256, validate_bindings, validate_grant_ref,
                    validate_knowledge_scope, validate_principal, validate_task_binding)

PROPOSAL_FIELDS = {
    "api_version", "bindings", "canonicalization", "capability_grant_ref", "kind",
    "knowledge_scope", "mutations", "proposal_id", "proposal_sha256", "proposer",
    "record_set_sha256", "records", "submitted_at_unix_ms", "task_binding",
}
TARGET_FIELDS = {
    "bindings", "capability_grant_ref", "knowledge_scope", "mutations", "proposer",
    "record_set_sha256", "task_binding",
}


def record_set_sha256(records: list[dict[str, Any]]) -> str:
    if not isinstance(records, list) or not 1 <= len(records) <= MAX_RECORDS:
        raise ContractError(f"record set must contain 1..{MAX_RECORDS} records")
    bounded_canonical_json(records, MAX_RECORD_SET_BYTES, "governance record set")
    issues = validate_record_set(records)
    if issues:
        raise ContractError(f"ADR-0045 record set invalid: {issues[0]}")
    return bounded_digest(RECORD_SET_DOMAIN, records, MAX_RECORD_SET_BYTES,
                          "governance record set")


def _validate_shared(node: dict[str, Any], label: str) -> None:
    validate_bindings(node["bindings"], f"{label}.bindings")
    validate_grant_ref(node["capability_grant_ref"], f"{label}.capability_grant_ref")
    validate_knowledge_scope(node["knowledge_scope"], f"{label}.knowledge_scope")
    validate_mutation_shapes(node["mutations"], f"{label}.mutations")
    validate_principal(node["proposer"], f"{label}.proposer")
    sha256(node["record_set_sha256"], f"{label}.record_set_sha256")
    validate_task_binding(node["task_binding"], f"{label}.task_binding")


def _record_scope(records: list[dict[str, Any]], node: dict[str, Any]) -> None:
    project_id = node["task_binding"]["project_id"]
    scope = node["knowledge_scope"]["object_ref"]
    for index, record in enumerate(records):
        metadata = record["metadata"]
        if metadata["project_id"] != project_id or metadata["scope"] != scope:
            raise ContractError(
                f"records[{index}] must match task project and knowledge object_ref")


def _after_provenance(after_records: list[dict[str, Any]], node: dict[str, Any]) -> None:
    bindings, proposer, task = node["bindings"], node["proposer"], node["task_binding"]
    submitted = node["submitted_at_unix_ms"]
    for record in after_records:
        metadata, creator = record["metadata"], record["metadata"]["created_by"]
        expected = (
            metadata["context_sha256"] == bindings["context_sha256"] and
            metadata["policy_sha256"] == bindings["policy_sha256"] and
            metadata["source_revision"] == bindings["source_revision"] and
            metadata["source_tree_sha256"] == bindings["source_tree_sha256"] and
            creator["authority_domain"] == proposer["authority_domain"] and
            creator["principal_id"] == proposer["principal_id"] and
            creator["principal_type"] == proposer["principal_type"] and
            creator["role"] == task["role"] and creator["run_id"] == task["run_id"] and
            metadata["created_at_unix_ms"] <= submitted
        )
        if not expected:
            raise ContractError("mutation after Claims must bind proposal provenance and time")


def proposal_sha256(value: dict[str, Any], validate: bool = True) -> str:
    if validate:
        validate_proposal(value, allow_empty_identity=True)
    bounded_canonical_json(value, MAX_PROPOSAL_BYTES, "KnowledgeUpdateProposal")
    payload = copy.deepcopy(value)
    payload["proposal_id"] = ""
    payload["proposal_sha256"] = ""
    return bounded_digest(PROPOSAL_DOMAIN, payload, MAX_PROPOSAL_BYTES,
                          "KnowledgeUpdateProposal digest preimage")


def validate_proposal(value: Any, allow_empty_identity: bool = False) -> dict[str, Any]:
    bounded_canonical_json(value, MAX_PROPOSAL_BYTES, "KnowledgeUpdateProposal")
    node = require_keys(value, "KnowledgeUpdateProposal", PROPOSAL_FIELDS)
    if node["api_version"] != PROPOSAL_API or node["kind"] != PROPOSAL_KIND:
        raise ContractError("KnowledgeUpdateProposal API/kind is unsupported; aliases are rejected")
    if node["canonicalization"] != CANONICALIZATION:
        raise ContractError("KnowledgeUpdateProposal canonicalization is unsupported")
    _validate_shared(node, "KnowledgeUpdateProposal")
    integer(node["submitted_at_unix_ms"], "submitted_at_unix_ms")
    records = node["records"]
    digest = record_set_sha256(records)
    if node["record_set_sha256"] != digest:
        raise ContractError("KnowledgeUpdateProposal record_set_sha256 does not match")
    _record_scope(records, node)
    by_id = record_index(records)
    after_records = validate_lifecycle(node["mutations"], by_id)
    validate_exact_closure(records, after_records, by_id)
    _after_provenance(after_records, node)
    if allow_empty_identity and node["proposal_id"] == node["proposal_sha256"] == "":
        return node
    sha256(node["proposal_sha256"], "proposal_sha256")
    if node["proposal_id"] != f"knowledge-update-proposal-{node['proposal_sha256']}":
        raise ContractError("KnowledgeUpdateProposal identity is inconsistent")
    if proposal_sha256(node, False) != node["proposal_sha256"]:
        raise ContractError("KnowledgeUpdateProposal self digest does not match")
    return node


def seal_proposal(candidate: dict[str, Any]) -> dict[str, Any]:
    validate_proposal(candidate, allow_empty_identity=True)
    if candidate["proposal_id"] != "" or candidate["proposal_sha256"] != "":
        raise ContractError("unsealed KnowledgeUpdateProposal identity fields must be empty")
    node = copy.deepcopy(candidate)
    digest = proposal_sha256(node, False)
    node["proposal_id"] = f"knowledge-update-proposal-{digest}"
    node["proposal_sha256"] = digest
    return validate_proposal(node)


def decode_proposal(raw: bytes) -> dict[str, Any]:
    return validate_proposal(decode_canonical(raw, MAX_PROPOSAL_BYTES,
                                              "KnowledgeUpdateProposal"))


def validate_target(value: Any) -> dict[str, Any]:
    bounded_canonical_json(value, MAX_TARGET_BYTES, "KnowledgeUpdate declared target")
    node = require_keys(value, "KnowledgeUpdate declared target", TARGET_FIELDS)
    _validate_shared(node, "KnowledgeUpdate declared target")
    return node


def declared_target(proposal: dict[str, Any]) -> dict[str, Any]:
    validate_proposal(proposal)
    return validate_target({field: copy.deepcopy(proposal[field]) for field in TARGET_FIELDS})


def declared_target_sha256(value: dict[str, Any]) -> str:
    validate_target(value)
    return bounded_digest(TARGET_DOMAIN, value, MAX_TARGET_BYTES,
                          "KnowledgeUpdate declared target")
