"""Adversarial tests for the ADR-0082 pure structural prerequisite."""
from __future__ import annotations

import base64
import copy
import hashlib
import json
import os
import stat
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
HARNESS = Path(__file__).resolve().parent
if str(HARNESS) not in sys.path:
    sys.path.insert(0, str(HARNESS))

from architecture_decision_record_v2 import (  # noqa: E402
    canonical_json as adr_json, self_digest as adr_self_digest, validate_document_bytes)
from authenticated_adr_approval_contract.documents import receipt_sha256 as approval_receipt_sha256  # noqa: E402
from authenticated_adr_lifecycle_contract import (  # noqa: E402
    ContractError, acceptance_sha256, decode_document, derive_proposal_binding,
    entry_sha256, golden_bytes, golden_fixture, ledger_sha256, load_golden,
    prerequisite_sha256, request_sha256, state_sha256, supersession_sha256,
    validate_document)
from authenticated_adr_lifecycle_contract.authority import trust_root_sha256  # noqa: E402
from authenticated_adr_lifecycle_contract.canonical import bounded_canonical_json  # noqa: E402
from authenticated_adr_lifecycle_contract.constants import (  # noqa: E402
    ACCEPTANCE_DOMAIN, ACCEPTANCE_SIGNATURE_DOMAIN,
    ADR_V2_SCHEMA_PATH, ADR_V2_SCHEMA_SHA256,
    APPROVAL_SCHEMA_PATH, APPROVAL_SCHEMA_SHA256,
    ENTRY_DOMAIN, FIXTURE_PATH, GOLDEN_SHA256, HEAD_DOMAIN, LEDGER_DOMAIN,
    MAX_GOLDEN_BYTES, MAX_REQUEST_VALIDITY_MS, PREREQUISITE_DOMAIN, PROPOSAL_FIXTURE_PATHS,
    RECORD_KEY_DOMAIN, REQUEST_DOMAIN, REQUEST_SIGNATURE_DOMAIN,
    SCHEMA_PATH, SCHEMA_SHA256, STATE_DOMAIN, STATE_SIGNATURE_DOMAIN,
    SUCCESS_MARKER, SUPERSESSION_DOMAIN, SUPERSESSION_SIGNATURE_DOMAIN,
    TRUST_ROOT_DOMAIN, VIEW_DOMAIN,
)
from authenticated_adr_lifecycle_contract.ledger import (  # noqa: E402
    current_head_set_sha256, rebuild_materialized_view, validate_ledger)
def _candidate() -> dict:
    return golden_fixture(ROOT)
def _receipt_bytes(receipt: dict) -> bytes:
    return bounded_canonical_json(receipt, 256 * 1024,
                                  "test authorization receipt")
def _seal_request(entry: dict) -> None:
    request = entry["request"]
    prerequisite = request["acceptance_prerequisite"]
    approval = prerequisite["authorization_receipt"]
    approval["receipt_id"] = ""
    approval["receipt_sha256"] = ""
    approval["receipt_sha256"] = approval_receipt_sha256(approval)
    approval["receipt_id"] = (
        f"architecture-decision-approval-receipt-{approval['receipt_sha256']}"
    )
    prerequisite["authorization_receipt_physical_sha256"] = hashlib.sha256(
        _receipt_bytes(approval)).hexdigest()
    prerequisite["prerequisite_sha256"] = prerequisite_sha256(prerequisite)
    request["request_id"] = ""
    request["request_sha256"] = ""
    request["request_sha256"] = request_sha256(request)
    request["request_id"] = (
        f"architecture-decision-lifecycle-request-{request['request_sha256']}"
    )
def _seal_acceptance(entry: dict) -> None:
    request = entry["request"]
    prerequisite = request["acceptance_prerequisite"]
    receipt = entry["acceptance_receipt"]
    receipt["authorization_receipt_physical_sha256"] = prerequisite[
        "authorization_receipt_physical_sha256"]
    receipt["authorization_receipt_sha256"] = prerequisite[
        "authorization_receipt"]["receipt_sha256"]
    receipt["request_sha256"] = request["request_sha256"]
    receipt["acceptance_id"] = ""
    receipt["acceptance_sha256"] = ""
    receipt["acceptance_sha256"] = acceptance_sha256(receipt)
    receipt["acceptance_id"] = (
        f"architecture-decision-acceptance-{receipt['acceptance_sha256']}"
    )
def _seal_supersessions(entry: dict) -> None:
    acceptance = entry["acceptance_receipt"]
    for receipt in entry["supersession_receipts"]:
        receipt["request_sha256"] = entry["request"]["request_sha256"]
        receipt["superseded_at_unix_ms"] = acceptance["accepted_at_unix_ms"]
        receipt["superseded_by_acceptance_id"] = acceptance["acceptance_id"]
        receipt["superseded_by_adr_id"] = acceptance["adr_id"]
        receipt["superseded_by_proposal_binding_sha256"] = acceptance[
            "proposal_binding_sha256"]
        receipt["receipt_id"] = ""
        receipt["receipt_sha256"] = ""
        receipt["receipt_sha256"] = supersession_sha256(receipt)
        receipt["receipt_id"] = (
            f"architecture-decision-supersession-{receipt['receipt_sha256']}"
        )
def _seal_entry(entry: dict) -> None:
    _seal_request(entry)
    _seal_acceptance(entry)
    _seal_supersessions(entry)
    entry["entry_sha256"] = ""
    entry["entry_sha256"] = entry_sha256(entry)
def _seal_outer(node: dict, rebuild: bool = False) -> None:
    state = node["lifecycle_state"]
    ledger = state["ledger"]
    ledger["ledger_sha256"] = ""
    ledger["ledger_sha256"] = ledger_sha256(ledger)
    if rebuild:
        profile = node["signature_profile"]["profile_sha256"]
        ledger, rows = validate_ledger(ledger, profile,
                                       node["lifecycle_trust_root"],
                                       node["approval_trust_root"])
        state["materialized_view"] = rebuild_materialized_view(ledger, rows)
    else:
        view = state["materialized_view"]
        view["ledger_sha256"] = ledger["ledger_sha256"]
    state["state_sha256"] = ""
    state["state_sha256"] = state_sha256(state)
    result = node["lifecycle_result"]
    result["ledger_sha256"] = ledger["ledger_sha256"]
    result["materialized_view_sha256"] = state["materialized_view"]["view_sha256"]
    result["state_sha256"] = state["state_sha256"]
def _seal_final(node: dict, rebuild: bool = False) -> None:
    entry = node["lifecycle_state"]["ledger"]["entries"][-1]
    _seal_entry(entry)
    _seal_outer(node, rebuild)
    result = node["lifecycle_result"]
    result["entry_sha256"] = entry["entry_sha256"]
    result["receipt"] = copy.deepcopy(entry["acceptance_receipt"])
    if rebuild:
        state = node["lifecycle_state"]
        state["state_sha256"] = ""
        state["state_sha256"] = state_sha256(state)
        result["state_sha256"] = state["state_sha256"]
def _seal_cascade_candidate(node: dict, changed_index: int, target_check: str, bypass) -> None:
    state = node["lifecycle_state"]
    ledger = state["ledger"]
    entries = ledger["entries"]
    profile = node["signature_profile"]["profile_sha256"]
    prior = None
    rows = {}
    with patch(target_check, side_effect=bypass):
        for index, entry in enumerate(entries):
            if index >= changed_index:
                request = entry["request"]
                request["expected_ledger_sha256"] = None if prior is None else prior["ledger_sha256"]
                request["expected_current_head_set_sha256"] = current_head_set_sha256(rows)
                entry["prior_entry_sha256"] = None if index == 0 else entries[index - 1]["entry_sha256"]
                for target in request["supersession_targets"]:
                    current = rows[target["adr_id"]]
                    for field in ("acceptance_id", "acceptance_sha256", "proposal_binding_sha256"):
                        target[field] = current[field]
                for target, receipt in zip(request["supersession_targets"], entry["supersession_receipts"]):
                    receipt["target_acceptance_id"] = target["acceptance_id"]
                    receipt["target_proposal_binding_sha256"] = target["proposal_binding_sha256"]
                prospective = copy.deepcopy(rows)
                binding = request["acceptance_prerequisite"]["proposal_binding"]
                prospective[binding["adr_id"]] = {
                    "adr_id": binding["adr_id"], "status": "accepted",
                    "proposal_binding_sha256": binding["proposal_binding_sha256"]}
                for target in request["supersession_targets"]:
                    prospective[target["adr_id"]]["status"] = "superseded"
                entry["resulting_current_head_set_sha256"] = current_head_set_sha256(prospective)
                _seal_entry(entry)
            prefix = copy.deepcopy(ledger)
            prefix["entries"] = copy.deepcopy(entries[:index + 1])
            prefix["last_sequence"] = index + 1
            prefix["current_head_set_sha256"] = entry["resulting_current_head_set_sha256"]
            prefix["ledger_sha256"] = ""
            prefix["ledger_sha256"] = ledger_sha256(prefix)
            prior, rows = validate_ledger(prefix, profile,
                                          node["lifecycle_trust_root"],
                                          node["approval_trust_root"])
        state["ledger"] = prior
        _seal_outer(node, rebuild=True)
        result = node["lifecycle_result"]
        result["entry_sha256"] = entries[-1]["entry_sha256"]
        result["receipt"] = copy.deepcopy(entries[-1]["acceptance_receipt"])
        validate_document(node)
def _seal_chronology_candidate(node: dict, changed_index: int = 2) -> None:
    _seal_cascade_candidate(
        node, changed_index,
        "authenticated_adr_lifecycle_contract.ledger._validate_entry_time",
        lambda item, _metadata, _prior:
        item["acceptance_receipt"]["accepted_at_unix_ms"])
def _seal_approval_candidate(node: dict, changed_index: int) -> None:
    _seal_cascade_candidate(
        node, changed_index,
        "authenticated_adr_lifecycle_contract.prerequisite._validate_ledger_binding",
        lambda *_args: None)
def _replace_final_source_time(node: dict, field: str, value: int) -> None:
    entry = node["lifecycle_state"]["ledger"]["entries"][-1]
    request = entry["request"]
    encoded = request["proposal_document_base64url"]
    raw = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4))
    newline = raw.find(b"\n", 4)
    metadata = json.loads(raw[4:newline])
    body = raw[newline + 6:]
    metadata[field] = value
    metadata["self_sha256"] = ""
    metadata["self_sha256"] = adr_self_digest(metadata, body)
    changed = b"---\n" + adr_json(metadata) + b"\n---\n\n" + body
    binding = derive_proposal_binding(changed, metadata["document_name"])
    prerequisite = request["acceptance_prerequisite"]
    prerequisite["proposal_binding"] = binding
    approval = prerequisite["authorization_receipt"]
    approval["proposal_binding_sha256"] = binding["proposal_binding_sha256"]
    approval["receipt_id"] = ""
    approval["receipt_sha256"] = ""
    approval["receipt_sha256"] = approval_receipt_sha256(approval)
    approval["receipt_id"] = (
        f"architecture-decision-approval-receipt-{approval['receipt_sha256']}"
    )
    prerequisite["authorization_receipt_physical_sha256"] = hashlib.sha256(
        _receipt_bytes(approval)).hexdigest()
    request["proposal_document_base64url"] = base64.urlsafe_b64encode(
        changed).decode("ascii").rstrip("=")
    entry["acceptance_receipt"]["proposal_binding_sha256"] = binding[
        "proposal_binding_sha256"]
    _seal_chronology_candidate(node)
class LifecycleContractTests(unittest.TestCase):
    def assert_rejected(self, node: dict) -> None:
        with self.assertRaises(ContractError):
            validate_document(node)
    def test_golden_reconstructs_and_pins_every_owned_input(self) -> None:
        expected = golden_bytes(ROOT)
        physical = (ROOT / FIXTURE_PATH).read_bytes()
        self.assertEqual(physical, expected)
        self.assertEqual(hashlib.sha256(physical).hexdigest(), GOLDEN_SHA256)
        self.assertEqual(hashlib.sha256((ROOT / SCHEMA_PATH).read_bytes()).hexdigest(),
                         SCHEMA_SHA256)
        self.assertEqual(load_golden(ROOT), decode_document(physical[:-1]))
        for path in (*PROPOSAL_FIXTURE_PATHS,):
            metadata = validate_document_bytes((ROOT / path).read_bytes(), path.name)
            self.assertEqual(metadata["status"], "proposed")
            self.assertIsNone(metadata["acceptance_id"])
            self.assertEqual(metadata["superseded_by"], [])
    def test_golden_has_two_heads_then_one_atomic_sorted_join(self) -> None:
        node = _candidate()
        entries = node["lifecycle_state"]["ledger"]["entries"]
        self.assertEqual([item["sequence"] for item in entries], [1, 2, 3])
        self.assertEqual([item["request"]["acceptance_prerequisite"]
                          ["authorization_receipt"]["ledger_sequence"]
                          for item in entries], [2, 1, 7])
        join = entries[-1]
        self.assertEqual([item["target_adr_id"] for item in
                          join["supersession_receipts"]], ["ADR-9003", "ADR-9004"])
        view = node["lifecycle_state"]["materialized_view"]
        self.assertEqual(view["head_adr_ids"], ["ADR-9005"])
        self.assertEqual([item["status"] for item in view["decisions"]],
                         ["superseded", "superseded", "accepted"])
    def test_proof_signature_change_rejected_without_outer_reseal(self) -> None:
        node = _candidate()
        signature = node["lifecycle_state"]["ledger"]["entries"][-1]["request"][
            "acceptance_prerequisite"]["authorization_receipt"]["signature"]
        signature["signature_base64url"] = "A" * 86
        self.assert_rejected(node)
    def test_proof_signature_change_passes_after_complete_structural_reseal(self) -> None:
        node = _candidate()
        signature = node["lifecycle_state"]["ledger"]["entries"][-1]["request"][
            "acceptance_prerequisite"]["authorization_receipt"]["signature"]
        signature["signature_base64url"] = "A" * 86
        prerequisite = node["lifecycle_state"]["ledger"]["entries"][-1]["request"][
            "acceptance_prerequisite"]
        prerequisite["authorization_receipt_physical_sha256"] = hashlib.sha256(
            _receipt_bytes(prerequisite["authorization_receipt"])).hexdigest()
        _seal_final(node, rebuild=True)
        self.assertEqual(validate_document(node), node)
    def test_stale_ledger_and_head_cas_reject_after_local_reseal(self) -> None:
        for field in ("expected_ledger_sha256",
                      "expected_current_head_set_sha256"):
            with self.subTest(field=field):
                node = _candidate()
                entry = node["lifecycle_state"]["ledger"]["entries"][1]
                entry["request"][field] = "f" * 64
                _seal_entry(entry)
                self.assert_rejected(node)
    def test_target_binding_partial_and_order_mutations_reject(self) -> None:
        node = _candidate()
        entry = node["lifecycle_state"]["ledger"]["entries"][-1]
        entry["request"]["supersession_targets"][0]["acceptance_sha256"] = "a" * 64
        _seal_entry(entry)
        self.assert_rejected(node)
        node = _candidate()
        entry = node["lifecycle_state"]["ledger"]["entries"][-1]
        entry["supersession_receipts"].pop()
        entry["entry_sha256"] = entry_sha256(entry)
        self.assert_rejected(node)
        node = _candidate()
        entry = node["lifecycle_state"]["ledger"]["entries"][-1]
        entry["supersession_receipts"].reverse()
        entry["entry_sha256"] = entry_sha256(entry)
        self.assert_rejected(node)
    def test_duplicate_idempotency_and_duplicate_adr_reject(self) -> None:
        node = _candidate()
        entries = node["lifecycle_state"]["ledger"]["entries"]
        entries[-1]["request"]["idempotency_key"] = entries[0]["request"][
            "idempotency_key"]
        _seal_final(node)
        self.assert_rejected(node)
        node = _candidate()
        final = node["lifecycle_state"]["ledger"]["entries"][-1]
        final["request"]["acceptance_prerequisite"]["proposal_binding"]["adr_id"] = (
            "ADR-9003")
        self.assert_rejected(node)
    def test_exact_observation_source_proposed_and_expiry_boundaries(self) -> None:
        node = _candidate()
        entry = node["lifecycle_state"]["ledger"]["entries"][-1]
        entry["acceptance_receipt"]["accepted_at_unix_ms"] += 1
        _seal_chronology_candidate(node)
        self.assert_rejected(node)
        node = _candidate()
        accepted = node["lifecycle_state"]["ledger"]["entries"][-1][
            "acceptance_receipt"]["accepted_at_unix_ms"]
        _replace_final_source_time(node, "proposed_at_unix_ms", accepted + 1)
        self.assert_rejected(node)
        node = _candidate()
        _replace_final_source_time(node, "expires_at_unix_ms", accepted)
        self.assert_rejected(node)
        node = _candidate()
        approval = node["lifecycle_state"]["ledger"]["entries"][-1]["request"][
            "acceptance_prerequisite"]["authorization_receipt"]
        _replace_final_source_time(node, "proposed_at_unix_ms",
                                   approval["evaluated_at_unix_ms"] + 1)
        self.assert_rejected(node)
        node = _candidate()
        approval = node["lifecycle_state"]["ledger"]["entries"][-1]["request"][
            "acceptance_prerequisite"]["authorization_receipt"]
        _replace_final_source_time(node, "expires_at_unix_ms",
                                   approval["authorization_expires_at_unix_ms"] - 1)
        self.assert_rejected(node)
    def test_observation_time_must_not_regress(self) -> None:
        node = _candidate()
        entries = node["lifecycle_state"]["ledger"]["entries"]
        observed = entries[0]["acceptance_receipt"]["accepted_at_unix_ms"] - 1
        entry = entries[1]
        prerequisite = entry["request"]["acceptance_prerequisite"]
        prerequisite["observed_at_unix_ms"] = observed
        prerequisite["authorization_receipt"]["evaluated_at_unix_ms"] = observed - 1
        prerequisite["authorization_ledger_clock_high_water_unix_ms"] = observed
        entry["request"]["requested_at_unix_ms"] = observed
        entry["request"]["expires_at_unix_ms"] = observed + MAX_REQUEST_VALIDITY_MS
        entry["acceptance_receipt"]["accepted_at_unix_ms"] = observed
        _seal_chronology_candidate(node, 1)
        self.assert_rejected(node)
    def test_approval_projection_exact_capacity_bounds(self) -> None:
        node = _candidate()
        prerequisite = node["lifecycle_state"]["ledger"]["entries"][-1]["request"][
            "acceptance_prerequisite"]
        prerequisite["authorization_ledger_last_sequence"] = 64
        prerequisite["revocation_high_water_sequence"] = 256
        prerequisite["revocation_high_water_sha256"] = "e" * 64
        _seal_final(node, rebuild=True)
        self.assertEqual(validate_document(node), node)
        for field, value in (("authorization_ledger_last_sequence", 65),
                             ("revocation_high_water_sequence", 257)):
            with self.subTest(field=field):
                node = _candidate()
                prerequisite = node["lifecycle_state"]["ledger"]["entries"][-1][
                    "request"]["acceptance_prerequisite"]
                prerequisite[field] = value
                _seal_approval_candidate(node, 2)
                self.assert_rejected(node)
    def test_approval_clock_and_prior_link_exact_relations(self) -> None:
        node = _candidate()
        prerequisite = node["lifecycle_state"]["ledger"]["entries"][-1]["request"][
            "acceptance_prerequisite"]
        evaluated = prerequisite["authorization_receipt"]["evaluated_at_unix_ms"]
        prerequisite["authorization_ledger_clock_high_water_unix_ms"] = evaluated
        _seal_final(node, rebuild=True)
        self.assertEqual(validate_document(node), node)
        node = _candidate()
        prerequisite = node["lifecycle_state"]["ledger"]["entries"][-1]["request"][
            "acceptance_prerequisite"]
        prerequisite["authorization_ledger_clock_high_water_unix_ms"] = (
            prerequisite["authorization_receipt"]["evaluated_at_unix_ms"] - 1)
        _seal_approval_candidate(node, 2)
        self.assert_rejected(node)
        node = _candidate()
        entry = node["lifecycle_state"]["ledger"]["entries"][-1]
        entry["request"]["acceptance_prerequisite"]["authorization_receipt"][
            "prior_receipt_sha256"] = None
        _seal_approval_candidate(node, 2)
        self.assert_rejected(node)
        node = _candidate()
        entry = node["lifecycle_state"]["ledger"]["entries"][1]
        entry["request"]["acceptance_prerequisite"]["authorization_receipt"][
            "prior_receipt_sha256"] = "d" * 64
        _seal_approval_candidate(node, 1)
        self.assert_rejected(node)
    def test_roots_must_be_cryptographically_and_semantically_independent(self) -> None:
        for field in ("key_id", "public_key_base64url"):
            with self.subTest(field=field):
                node = _candidate()
                lifecycle = node["lifecycle_trust_root"]
                lifecycle["keys"][0][field] = node["approval_trust_root"]["keys"][0][field]
                lifecycle["root_sha256"] = trust_root_sha256(lifecycle)
                self.assert_rejected(node)
        node = _candidate()
        lifecycle = node["lifecycle_trust_root"]
        lifecycle["trust_domain"] = node["approval_trust_root"]["trust_domain"]
        for key in lifecycle["keys"]:
            key["principal"]["authority_domain"] = lifecycle["trust_domain"]
        lifecycle["root_sha256"] = trust_root_sha256(lifecycle)
        self.assert_rejected(node)
    def test_materialized_view_and_state_cross_snapshot_tampering_reject(self) -> None:
        node = _candidate()
        node["lifecycle_state"]["materialized_view"]["head_adr_ids"] = ["ADR-9003"]
        self.assert_rejected(node)
        node = _candidate()
        node["lifecycle_state"]["materialized_view"]["ledger_sha256"] = "0" * 64
        self.assert_rejected(node)
        node = _candidate()
        node["lifecycle_state"]["ledger"]["last_sequence"] = True
        self.assert_rejected(node)
    def test_result_replay_can_reference_history_but_stored_cannot(self) -> None:
        node = _candidate()
        first = node["lifecycle_state"]["ledger"]["entries"][0]
        result = node["lifecycle_result"]
        result["delivery_disposition"] = "exact_replay"
        result["entry_sha256"] = first["entry_sha256"]
        result["receipt"] = copy.deepcopy(first["acceptance_receipt"])
        self.assertEqual(validate_document(node), node)
        result["delivery_disposition"] = "stored"
        self.assert_rejected(node)
    def test_canonical_decoder_rejects_forms_and_bounds(self) -> None:
        raw = golden_bytes(ROOT)[:-1]
        with self.assertRaises(ContractError):
            decode_document(raw + b"\n")
        with self.assertRaises(ContractError):
            decode_document(raw.replace(b'{"api_version":',
                                        b'{"api_version":"duplicate","api_version":', 1))
        with self.assertRaises(ContractError):
            decode_document(raw.replace(b'"trust_epoch":1', b'"trust_epoch":1.0', 1))
        with self.assertRaises(ContractError):
            decode_document(raw.replace(b'"profile_id":',
                                        b'"unknown_field":"x","profile_id":', 1))
        with self.assertRaises(ContractError):
            decode_document(raw.replace(b'"stored"', b'"stored\\u202e"', 1))
    def test_domains_are_unique_nul_terminated_and_head_is_acyclic(self) -> None:
        domains = (TRUST_ROOT_DOMAIN, PREREQUISITE_DOMAIN, REQUEST_DOMAIN,
                   ACCEPTANCE_DOMAIN, SUPERSESSION_DOMAIN, ENTRY_DOMAIN,
                   LEDGER_DOMAIN, HEAD_DOMAIN, VIEW_DOMAIN, STATE_DOMAIN,
                   RECORD_KEY_DOMAIN, REQUEST_SIGNATURE_DOMAIN,
                   ACCEPTANCE_SIGNATURE_DOMAIN, SUPERSESSION_SIGNATURE_DOMAIN,
                   STATE_SIGNATURE_DOMAIN)
        self.assertEqual(len(domains), len(set(domains)))
        self.assertTrue(all(item.endswith(b"\0") for item in domains))
        node = _candidate()
        entry = node["lifecycle_state"]["ledger"]["entries"][-1]
        self.assertNotIn("resulting_current_head_set_sha256",
                         entry["acceptance_receipt"])
        self.assertNotIn("resulting_current_head_set_sha256",
                         entry["supersession_receipts"][0])
    def test_optional_json_schema_with_explicit_external_resolution(self) -> None:
        try:
            import jsonschema
            from referencing import Registry, Resource
        except ImportError as error:
            self.skipTest(f"optional jsonschema unavailable: {error}")
        schema = json.loads((ROOT / SCHEMA_PATH).read_text(encoding="utf-8"))
        approval = json.loads((ROOT / APPROVAL_SCHEMA_PATH).read_text(encoding="utf-8"))
        registry = Registry().with_resource(approval["$id"],
                                            Resource.from_contents(approval))
        jsonschema.Draft202012Validator(schema, registry=registry).validate(_candidate())
        with self.assertRaises(Exception):
            jsonschema.Draft202012Validator(schema).validate(_candidate())
    def test_dependency_schema_pins_modes_and_checker(self) -> None:
        self.assertEqual(hashlib.sha256((ROOT / APPROVAL_SCHEMA_PATH).read_bytes()).hexdigest(),
                         APPROVAL_SCHEMA_SHA256)
        self.assertEqual(hashlib.sha256((ROOT / ADR_V2_SCHEMA_PATH).read_bytes()).hexdigest(),
                         ADR_V2_SCHEMA_SHA256)
        package = HARNESS / "authenticated_adr_lifecycle_contract"
        owned = [ROOT / "docs/adr/ADR-0082-authenticated-architecture-decision-lifecycle-v1-prerequisite.md",
                 ROOT / SCHEMA_PATH, ROOT / FIXTURE_PATH,
                 *(ROOT / path for path in PROPOSAL_FIXTURE_PATHS),
                 *sorted(package.glob("*.py")),
                 HARNESS / "authenticated_adr_lifecycle_contract_check.py", Path(__file__)]
        self.assertEqual(len(owned), 20)
        for path in owned:
            info = path.lstat()
            self.assertTrue(stat.S_ISREG(info.st_mode))
            self.assertEqual(stat.S_IMODE(info.st_mode), 0o644)
            self.assertEqual(info.st_nlink, 1)
        self.assertFalse(any(package.rglob("__pycache__")))
        command = [sys.executable, "-S", "-B",
                   str(HARNESS / "authenticated_adr_lifecycle_contract_check.py"),
                   "--golden", str(ROOT)]
        result = subprocess.run(command, cwd=HARNESS, text=True,
                                capture_output=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, SUCCESS_MARKER + "\n")


if __name__ == "__main__":
    unittest.main()
