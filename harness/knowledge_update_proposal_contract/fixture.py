"""Deterministic KnowledgeUpdateProposal v1 cross-language golden construction."""

from __future__ import annotations

import copy
from pathlib import Path
from typing import Any

import capability_grant_contract
from governance_contract import compute_record_digest, validate_record_set

from .assessment import evaluate_declared_assessment, seal_request
from .canonical import ContractError, canonical_json
from .compatibility import project_artifact_resources, project_capability_grant_ref
from .constants import CANONICALIZATION, PROPOSAL_API, PROPOSAL_KIND
from .proposal import declared_target, seal_proposal


def _seal_record(record: dict[str, Any]) -> dict[str, Any]:
    record["integrity"]["canonical_sha256"] = ""
    record["integrity"]["canonical_sha256"] = compute_record_digest(record)
    return record


def _source_records(repo_root: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    fixture = repo_root / "docs/contracts/fixtures/governance-evidence-claim-v1.json"
    import json
    value = json.loads(fixture.read_text(encoding="utf-8"))
    return copy.deepcopy(value["records"][0]["record"]), copy.deepcopy(
        value["records"][1]["record"])


def _metadata(record: dict[str, Any], record_id: str, aggregate_id: str, sequence: int,
              scope: str, created_at: int, proposer: dict[str, Any],
              task: dict[str, Any], bindings: dict[str, Any], supersedes: list[str]) -> None:
    record["metadata"].update({
        "aggregate_id": aggregate_id,
        "context_sha256": bindings["context_sha256"],
        "created_at_unix_ms": created_at,
        "created_by": {**proposer, "role": task["role"], "run_id": task["run_id"]},
        "policy_sha256": bindings["policy_sha256"],
        "project_id": task["project_id"],
        "record_id": record_id,
        "scope": scope,
        "sequence": sequence,
        "source_revision": bindings["source_revision"],
        "source_tree_sha256": bindings["source_tree_sha256"],
        "supersedes_record_ids": supersedes,
    })
    record["status"]["valid_from_unix_ms"] = created_at
    record["status"]["valid_until_unix_ms"] = None


def _records(repo_root: Path, proposer: dict[str, Any], task: dict[str, Any],
             bindings: dict[str, Any], scope: str) -> list[dict[str, Any]]:
    evidence_source, claim_source = _source_records(repo_root)
    evidence = _evidence(evidence_source, proposer, task, bindings, scope)
    create = _create(claim_source, evidence, proposer, task, bindings, scope)
    before, after = _supersede_pair(
        claim_source, evidence, proposer, task, bindings, scope)
    records = sorted([evidence, after, before, create],
                     key=lambda record: record["metadata"]["record_id"])
    issues = validate_record_set(records)
    if issues:
        raise ContractError(f"fixture record set invalid: {issues[0]}")
    return records


def _evidence(source: dict[str, Any], proposer: dict[str, Any], task: dict[str, Any],
              bindings: dict[str, Any], scope: str) -> dict[str, Any]:
    evidence = copy.deepcopy(source)
    _metadata(evidence, "evidence-knowledge-update-support", "evidence-kup-support", 1,
              scope, 1_700_000_000_000, proposer, task, bindings, [])
    evidence["spec"]["subjects"] = [scope]
    evidence["spec"]["observed_at_unix_ms"] = 1_700_000_000_000
    evidence["spec"]["collector"]["run_id"] = task["run_id"]
    return _seal_record(evidence)


def _create(source: dict[str, Any], evidence: dict[str, Any], proposer: dict[str, Any],
            task: dict[str, Any], bindings: dict[str, Any], scope: str) -> dict[str, Any]:
    create = copy.deepcopy(source)
    _metadata(create, "claim-knowledge-update-create", "claim-kup-create", 1, scope,
              1_700_000_001_000, proposer, task, bindings, [])
    create["spec"].update({
        "object_value": "contract-only create declaration",
        "owner": {"principal_id": proposer["principal_id"],
                  "principal_type": proposer["principal_type"]},
        "predicate": "has-declared-contract-update",
        "review_by_unix_ms": 1_700_086_401_000,
        "subject": scope,
        "supporting_evidence_record_ids": [evidence["metadata"]["record_id"]],
    })
    return _seal_record(create)


def _supersede_pair(source: dict[str, Any], evidence: dict[str, Any],
                    proposer: dict[str, Any], task: dict[str, Any],
                    bindings: dict[str, Any], scope: str) -> tuple[dict[str, Any],
                                                                   dict[str, Any]]:
    before = copy.deepcopy(source)
    old_bindings = {**bindings, "context_sha256": "a" * 64,
                    "source_revision": "fixture-prior-revision",
                    "source_tree_sha256": "b" * 64}
    old_proposer = {"authority_domain": "forgeos.prior", "principal_id": "prior-agent",
                    "principal_type": "agent"}
    _metadata(before, "claim-knowledge-update-before", "claim-kup-revise", 1, scope,
              1_699_999_999_000, old_proposer, task, old_bindings, [])
    before["spec"].update({
        "object_value": "stable semantic value",
        "owner": {"principal_id": "knowledge-owner", "principal_type": "human"},
        "predicate": "has-declared-stable-property",
        "review_by_unix_ms": 1_700_086_399_000,
        "subject": scope,
        "supporting_evidence_record_ids": [evidence["metadata"]["record_id"]],
    })
    before = _seal_record(before)
    after = copy.deepcopy(before)
    _metadata(after, "claim-knowledge-update-after", "claim-kup-revise", 2, scope,
              1_700_000_001_500, proposer, task, bindings,
              [before["metadata"]["record_id"]])
    after["status"]["state"] = "contested"
    return before, _seal_record(after)


def _bindings(grant: dict[str, Any]) -> dict[str, Any]:
    value = {field: grant["bindings"][field] for field in (
        "context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
        "risk_sha256", "source_revision", "source_tree_sha256")}
    value["artifacts"] = [{
        "artifact_kind": "knowledge-update-input",
        "artifact_ref": "governance/knowledge-update-proposal-fixture-v1",
        "artifact_sha256": "8" * 64,
    }]
    return value


def _mutations(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    by_id = {record["metadata"]["record_id"]: record for record in records}
    mutations = [
        {
            "after_claim_ref": {"canonical_sha256": by_id[
                "claim-knowledge-update-after"]["integrity"]["canonical_sha256"],
                                "record_id": "claim-knowledge-update-after"},
            "before_claim_ref": {"canonical_sha256": by_id[
                "claim-knowledge-update-before"]["integrity"]["canonical_sha256"],
                                 "record_id": "claim-knowledge-update-before"},
            "operation": "supersede", "rationale": "declare a shadow-state successor",
            "reason_codes": ["declared_shadow_lifecycle_update"],
            "target_aggregate_id": "claim-kup-revise", "target_kind": "KnowledgeClaim",
        },
        {
            "after_claim_ref": {"canonical_sha256": by_id[
                "claim-knowledge-update-create"]["integrity"]["canonical_sha256"],
                                "record_id": "claim-knowledge-update-create"},
            "before_claim_ref": None,
            "operation": "create", "rationale": "declare a new candidate Claim",
            "reason_codes": ["declared_new_candidate_claim"],
            "target_aggregate_id": "claim-kup-create", "target_kind": "KnowledgeClaim",
        },
    ]
    mutations.sort(key=lambda item: item["target_aggregate_id"].encode("utf-8"))
    return mutations


def _candidate(grant: dict[str, Any], bindings: dict[str, Any], scope: str,
               records: list[dict[str, Any]]) -> dict[str, Any]:
    from .proposal import record_set_sha256
    return {
        "api_version": PROPOSAL_API,
        "bindings": bindings,
        "canonicalization": CANONICALIZATION,
        "capability_grant_ref": project_capability_grant_ref(grant),
        "kind": PROPOSAL_KIND,
        "knowledge_scope": {"object_kind": "knowledge", "object_ref": scope,
                            "object_scope_sha256": "9" * 64,
                            "scope_kind": "governance_object"},
        "mutations": _mutations(records),
        "proposal_id": "",
        "proposal_sha256": "",
        "proposer": grant["subject"],
        "record_set_sha256": record_set_sha256(records),
        "records": records,
        "submitted_at_unix_ms": 1_700_000_002_000,
        "task_binding": grant["task_binding"],
    }


def golden_fixture(repo_root: Path) -> dict[str, Any]:
    grant = capability_grant_contract.load_golden(repo_root)["grant"]
    proposer, task, bindings = grant["subject"], grant["task_binding"], _bindings(grant)
    scope = "knowledge:forgeos-contract-kernel"
    records = _records(repo_root, proposer, task, bindings, scope)
    proposal = seal_proposal(_candidate(grant, bindings, scope, records))
    target = declared_target(proposal)
    request = seal_request(proposal, target, proposal["submitted_at_unix_ms"])
    return {
        "assessment_request": request,
        "expected_artifact_resources": project_artifact_resources(proposal),
        "expected_assessment": evaluate_declared_assessment(request),
        "expected_capability_grant_ref": project_capability_grant_ref(grant),
        "knowledge_update_proposal": proposal,
    }


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    print(canonical_json(golden_fixture(root)).decode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
