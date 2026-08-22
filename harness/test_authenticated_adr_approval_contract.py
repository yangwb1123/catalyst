"""Focused adversarial tests for the ADR-0079 pure structural candidate core."""

from __future__ import annotations

import base64
import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "harness"
if str(HARNESS) not in sys.path:
    sys.path.insert(0, str(HARNESS))
import authenticated_adr_approval_contract as contract  # noqa: E402
from approval_record_contract import seal_record  # noqa: E402
from authenticated_adr_approval_contract.approvals import (  # noqa: E402
    declared_outcome, declared_reason_codes, validate_approval_records)
from authenticated_adr_approval_contract.constants import (  # noqa: E402
    ADR_V2_SCHEMA_PATH,
    ADR_V2_SCHEMA_SHA256,
    APPROVAL_RECORD_SIGNATURE_DOMAIN,
    APPROVAL_RECORD_SOD_SIGNATURE_DOMAIN,
    APPROVAL_RECORD_SCHEMA_PATH,
    APPROVAL_RECORD_SCHEMA_SHA256,
    FIXTURE_PATH,
    GOLDEN_SHA256,
    LEDGER_SIGNATURE_DOMAIN,
    POLICY_SIGNATURE_DOMAIN,
    PROPOSAL_BODY_SHA256,
    PROPOSAL_FIXTURE_PATH,
    PROPOSAL_PHYSICAL_SHA256,
    PROPOSAL_SELF_SHA256,
    RECEIPT_SIGNATURE_DOMAIN,
    REQUEST_SIGNATURE_DOMAIN,
    REVOCATION_SIGNATURE_DOMAIN,
    SCHEMA_PATH,
    SCHEMA_SHA256,
)
from authenticated_adr_approval_contract.authority import (  # noqa: E402
    trust_root_sha256,
    validate_trust_root,
)
from authenticated_adr_approval_contract.canonical import (  # noqa: E402
    MAX_ARRAY_ITEMS,
    MAX_STRING_BYTES,
    bounded_canonical_json,
)
from authenticated_adr_approval_contract.documents import validate_receipt_relations  # noqa: E402
from authenticated_adr_approval_contract.fixture import (  # noqa: E402
    _bind_entry_to_snapshot_for_tests, _reseal_two_entry_ledger_for_tests,
    _two_entry_candidate_for_tests)
from authenticated_adr_approval_contract.policy import policy_sha256  # noqa: E402
from authenticated_adr_approval_contract.proposal import (  # noqa: E402
    decode_proposal_document,
    encode_proposal_document,
)

class AuthenticatedADRApprovalStructuralTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = contract.load_golden(ROOT)

    def document(self):
        return copy.deepcopy(self.golden)

    def context(self, document=None):
        node = self.golden if document is None else document
        profile_hash = node["signature_profile"]["profile_sha256"]
        root = node["trust_root"]
        binding = node["proposal_binding"]
        _, metadata = decode_proposal_document(node["proposal_document_base64url"], binding,
                                               "test proposal")
        policy = node["authorization_policy"]
        snapshot = node["revocation_snapshot"]
        return profile_hash, root, metadata, policy, snapshot
    def test_exact_pins_one_lf_public_api_and_dependency_free_cli(self):
        fixture = ROOT / FIXTURE_PATH
        schema = ROOT / SCHEMA_PATH
        proposal = ROOT / PROPOSAL_FIXTURE_PATH
        self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), GOLDEN_SHA256)
        self.assertEqual(hashlib.sha256(schema.read_bytes()).hexdigest(), SCHEMA_SHA256)
        self.assertEqual(hashlib.sha256((ROOT / APPROVAL_RECORD_SCHEMA_PATH).read_bytes()).hexdigest(),
                         APPROVAL_RECORD_SCHEMA_SHA256)
        self.assertEqual(hashlib.sha256((ROOT / ADR_V2_SCHEMA_PATH).read_bytes()).hexdigest(),
                         ADR_V2_SCHEMA_SHA256)
        self.assertEqual(hashlib.sha256(proposal.read_bytes()).hexdigest(),
                         PROPOSAL_PHYSICAL_SHA256)
        self.assertEqual(self.golden["proposal_binding"]["body_sha256"],
                         PROPOSAL_BODY_SHA256)
        self.assertEqual(self.golden["proposal_binding"]["self_sha256"],
                         PROPOSAL_SELF_SHA256)
        raw = fixture.read_bytes()
        self.assertTrue(raw.endswith(b"\n") and not raw.endswith(b"\n\n"))
        self.assertEqual(raw, contract.golden_bytes(ROOT))
        command = [sys.executable, "-S", "-B",
                   str(HARNESS / "authenticated_adr_approval_contract_check.py")]
        result = subprocess.run(command + ["--golden", str(ROOT)], cwd=ROOT,
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, contract.SUCCESS_MARKER + "\n")
        self.assertIn("no authentication, authorization, acceptance, persistence, effect",
                      result.stdout)
        with tempfile.NamedTemporaryFile(suffix=".json") as stream:
            stream.write(raw[:-1])
            stream.flush()
            instance = subprocess.run(command + ["--file", stream.name], cwd=ROOT,
                                      capture_output=True, text=True, check=False)
        self.assertEqual(instance.returncode, 0, instance.stderr)
        physical = subprocess.run(command + ["--file", str(fixture)], cwd=ROOT,
                                  capture_output=True, text=True, check=False)
        self.assertEqual(physical.returncode, 1)
        self.assertEqual(physical.stdout, "")
    def test_schema_accepts_golden_only_with_explicit_approval_schema_registry(self):
        try:
            from jsonschema import Draft202012Validator
            from referencing import Registry, Resource
        except ImportError as error:
            self.skipTest(f"optional jsonschema/referencing unavailable: {error}")
        schema = json.loads((ROOT / SCHEMA_PATH).read_text())
        signature_domains = schema["x-forgeos-signature-domains"]
        expected_domains = {
            "policy": POLICY_SIGNATURE_DOMAIN,
            "revocation_snapshot": REVOCATION_SIGNATURE_DOMAIN,
            "authorization_request": REQUEST_SIGNATURE_DOMAIN,
            "authorization_receipt": RECEIPT_SIGNATURE_DOMAIN,
            "authorization_ledger": LEDGER_SIGNATURE_DOMAIN,
            "approval_record_authority": APPROVAL_RECORD_SIGNATURE_DOMAIN,
            "approval_record_separation_of_duty": APPROVAL_RECORD_SOD_SIGNATURE_DOMAIN,
        }
        for field, domain in expected_domains.items():
            self.assertEqual(signature_domains[field].encode(), domain)
        self.assertEqual(schema["x-forgeos-authority-semantics"]["positive_result"],
                         contract.SUCCESS_MARKER)
        approval = json.loads((ROOT / "docs/contracts/approval-record-v1.schema.json").read_text())
        registry = Registry().with_resource(approval["$id"], Resource.from_contents(approval))
        validator = Draft202012Validator(schema, registry=registry)
        validator.validate(self.golden)
        bad = self.document()
        bad["authorization_policy"]["unexpected"] = None
        self.assertTrue(list(validator.iter_errors(bad)))
        for field, value in (("adr_id", "ADR-0000"),
                             ("document_name", "ADR-0000-zero.md")):
            bad = self.document()
            bad["proposal_binding"][field] = value
            self.assertTrue(list(validator.iter_errors(bad)))
        with self.assertRaises(Exception) as caught:
            Draft202012Validator(schema).validate(self.golden)
        self.assertIn("Unresolvable", type(caught.exception).__name__ + str(caught.exception))
    def test_strict_json_canonical_duplicate_depth_unicode_int64_and_lf(self):
        raw = (ROOT / FIXTURE_PATH).read_bytes()[:-1]
        with self.assertRaisesRegex(contract.ContractError, "duplicate JSON key"):
            contract.decode_document(b'{"authorization_ledger":null,' + raw[1:])
        pretty = json.dumps(self.golden, indent=2).encode()
        with self.assertRaisesRegex(contract.ContractError, "exact compact canonical JSON"):
            contract.decode_document(pretty)
        with self.assertRaisesRegex(contract.ContractError, "exact compact canonical JSON"):
            contract.decode_document(raw + b"\n")
        floating = raw.replace(b'"trust_epoch":1', b'"trust_epoch":1.0', 1)
        with self.assertRaisesRegex(contract.ContractError, "non-integer"):
            contract.decode_document(floating)
        deep = b'{"a":' * 17 + b"null" + b"}" * 17
        with self.assertRaisesRegex(contract.ContractError, "depth exceeds"):
            contract.decode_document(deep)
        document = self.document()
        document["authorization_policy"]["policy_id"] = "bad\u202epolicy"
        with self.assertRaisesRegex(contract.ContractError, "forbidden control"):
            contract.validate_document(document)
        document = self.document()
        document["trust_root"]["trust_epoch"] = 2**63
        with self.assertRaisesRegex(contract.ContractError, "signed int64"):
            contract.validate_document(document)
    def test_domains_are_independent_and_signature_messages_are_not_verified(self):
        domains = {
            APPROVAL_RECORD_SIGNATURE_DOMAIN, APPROVAL_RECORD_SOD_SIGNATURE_DOMAIN,
            POLICY_SIGNATURE_DOMAIN, REVOCATION_SIGNATURE_DOMAIN,
            REQUEST_SIGNATURE_DOMAIN, RECEIPT_SIGNATURE_DOMAIN, LEDGER_SIGNATURE_DOMAIN,
        }
        self.assertEqual(len(domains), 7)
        digest = self.golden["authorization_policy"]["policy_sha256"]
        self.assertEqual(contract.signature_message(POLICY_SIGNATURE_DOMAIN, digest),
                         POLICY_SIGNATURE_DOMAIN + bytes.fromhex(digest))
        with self.assertRaisesRegex(contract.ContractError, "lowercase SHA-256"):
            contract.signature_message(POLICY_SIGNATURE_DOMAIN, "BAD")
        document = self.document()
        replacement = base64.urlsafe_b64encode(b"z" * 64).decode().rstrip("=")
        document["authorization_policy"]["signature"]["signature_base64url"] = replacement
        document["authorization_ledger"]["entries"][0]["policy"] = copy.deepcopy(
            document["authorization_policy"])
        document["authorization_ledger"]["ledger_sha256"] = contract.ledger_sha256(
            document["authorization_ledger"])
        self.assertEqual(contract.validate_document(document)["trust_root"]["trust_epoch"], 1)
        self.assertTrue(document["trust_root"]["trust_domain"].startswith("forgeos.fixture."))
    def test_root_usage_counts_distinctness_and_external_pin_absence_are_closed(self):
        document = self.document()
        document["trust_root"]["keys"][2]["usage"] = "approval_revocation_sign"
        with self.assertRaisesRegex(contract.ContractError, "usage counts"):
            contract.validate_document(document)
        document = self.document()
        document["trust_root"]["keys"][1]["principal"] = copy.deepcopy(
            document["trust_root"]["keys"][0]["principal"])
        with self.assertRaisesRegex(contract.ContractError, "pairwise distinct"):
            contract.validate_document(document)
        document = self.document()
        public = document["trust_root"]["keys"][0]["public_key_base64url"]
        document["trust_root"]["keys"][0]["public_key_base64url"] = public[:-1] + "B"
        with self.assertRaisesRegex(contract.ContractError, "canonical unpadded"):
            contract.validate_document(document)
        self.assertNotIn("pinned_root_sha256", self.golden)
    def test_policy_maps_every_proposal_role_and_enforces_non_self_approval(self):
        profile_hash, root, metadata, policy, _ = self.context()
        mutated = copy.deepcopy(policy)
        mutated["roles"]["owner_bindings"][0]["owner_ref"] = "missing-owner"
        mutated["policy_sha256"] = policy_sha256(mutated)
        with self.assertRaisesRegex(contract.ContractError, "every exact proposal owner_ref"):
            contract.validate_policy(mutated, profile_hash, root, metadata)
        mutated = copy.deepcopy(policy)
        approver = root["keys"][0]["principal"]
        mutated["roles"]["owner_bindings"][0]["principal"] = copy.deepcopy(approver)
        mutated["policy_sha256"] = policy_sha256(mutated)
        with self.assertRaisesRegex(contract.ContractError, "role separation"):
            contract.validate_policy(mutated, profile_hash, root, metadata)
        mutated = copy.deepcopy(policy)
        mutated["threshold"] = 1
        mutated["policy_sha256"] = policy_sha256(mutated)
        with self.assertRaisesRegex(contract.ContractError, "2..16"):
            contract.validate_policy(mutated, profile_hash, root, metadata)
        mutated = copy.deepcopy(policy)
        mutated["validity"]["not_before_unix_ms"] = metadata["proposed_at_unix_ms"] - 1
        mutated["validity"]["expires_at_unix_ms"] -= 1
        mutated["policy_sha256"] = policy_sha256(mutated)
        with self.assertRaisesRegex(contract.ContractError, "before the proposal"):
            contract.validate_policy(mutated, profile_hash, root, metadata)
    def test_approval_records_are_exact_gate_empty_and_triple_bound(self):
        _, root, _, policy, snapshot = self.context()
        base = self.golden["authorization_request"]["approval_records"][0]
        cases = []
        condition = {"condition_id": "extra", "condition_ref": "fixture/condition",
                     "condition_sha256": "9" * 64}
        cases.append(("conditions", lambda record: record.update(conditions=[condition])))
        cases.append(("bindings", lambda record: record["bindings"]["artifacts"][0].update(
            artifact_sha256="8" * 64)))
        cases.append(("fixed policy gate", lambda record: record["scope"].update(
            gate_id="another-gate")))
        cases.append(("RiskAcceptance", lambda record: record.update(risk_acceptance_refs=[{
            "authority_domain": "fixture", "risk_acceptance_id": "risk-one",
            "risk_acceptance_sha256": "7" * 64}])))
        cases.append(("encode exactly 64", lambda record: record["authority_proof"].update(
            proof_base64url=base64.urlsafe_b64encode(b"short-proof-data").decode().rstrip("="))))
        for expected, mutate in cases:
            with self.subTest(expected=expected):
                record = copy.deepcopy(base)
                mutate(record)
                record["approval_id"] = record["approval_sha256"] = ""
                sealed = seal_record(record)
                with self.assertRaisesRegex(contract.ContractError, expected):
                    validate_approval_records([sealed], policy, root, snapshot,
                                              self.golden["authorization_receipt"][
                                                  "evaluated_at_unix_ms"])
    def test_threshold_reject_veto_time_and_revocation_relations_fail_closed(self):
        profile_hash, root, _, policy, snapshot = self.context()
        request = copy.deepcopy(self.golden["authorization_request"])
        receipt = self.golden["authorization_receipt"]
        request["approval_records"] = request["approval_records"][:1]
        with self.assertRaisesRegex(contract.ContractError, "declared policy"):
            validate_receipt_relations(policy, request, snapshot, receipt, root)
        request = copy.deepcopy(self.golden["authorization_request"])
        record = request["approval_records"][0]
        record["decision"] = "reject"
        record["decision_basis"]["reason_codes"] = ["architecture_decision_rejected"]
        record["approval_id"] = record["approval_sha256"] = ""
        request["approval_records"][0] = seal_record(record)
        request["approval_records"].sort(key=lambda item: item["approval_id"])
        with self.assertRaisesRegex(contract.ContractError, "declared policy"):
            validate_receipt_relations(policy, request, snapshot, receipt, root)
        revoked = copy.deepcopy(snapshot)
        revoked["revoked_approval_ids"] = [
            self.golden["authorization_request"]["approval_records"][0]["approval_id"]]
        with self.assertRaisesRegex(contract.ContractError, "revocation snapshot"):
            validate_approval_records(self.golden["authorization_request"]["approval_records"],
                                      policy, root, revoked,
                                      receipt["evaluated_at_unix_ms"])
        bad_request = copy.deepcopy(self.golden["authorization_request"])
        bad_request["expires_at_unix_ms"] += 1
        with self.assertRaisesRegex(contract.ContractError, "signed policy maximum"):
            contract.validate_request(bad_request, profile_hash, root, policy, snapshot)
    def test_revocation_covers_every_used_signing_key(self):
        keys = ("fixture-policy-key-1", "fixture-request-key-1",
                "fixture-revocation-key-1", "fixture-state-key-1")
        for key_id in keys:
            with self.subTest(key_id=key_id):
                document = self.document()
                _reseal_with_revoked_key(document, key_id)
                with self.assertRaisesRegex(contract.ContractError, "revok"):
                    contract.validate_document(document)
    def test_proposal_bytes_status_and_three_digests_are_authoritative(self):
        proposal = (ROOT / PROPOSAL_FIXTURE_PATH).read_bytes()
        binding = self.golden["proposal_binding"]
        self.assertEqual(contract.validate_proposal_bytes(proposal, binding), binding)
        with self.assertRaisesRegex(contract.ContractError, "strict Proposed ADR v2"):
            contract.validate_proposal_bytes(proposal.replace(b'"status":"proposed"',
                                                               b'"status":"accepted"'),
                                             binding)
        mutated = copy.deepcopy(binding)
        mutated["physical_sha256"] = "0" * 64
        mutated["proposal_binding_sha256"] = contract.proposal_binding_sha256(mutated)
        with self.assertRaisesRegex(contract.ContractError, "do not match ProposalBinding"):
            contract.validate_proposal_bytes(proposal, mutated)
        document = self.document()
        encoded = document["proposal_document_base64url"]
        document["proposal_document_base64url"] = encoded[:-1] + (
            "A" if encoded[-1] != "A" else "B")
        with self.assertRaises(contract.ContractError):
            contract.validate_document(document)
    def test_digest_identity_cas_receipt_and_complete_ledger_chains_are_closed(self):
        mutations = (
            ("self digest", lambda d: d["trust_root"].update(root_sha256="0" * 64)),
            ("policy self", lambda d: d["authorization_policy"].update(
                policy_sha256="0" * 64)),
            ("revocation snapshot self", lambda d: d["revocation_snapshot"].update(
                revocation_sha256="0" * 64)),
            ("request self", lambda d: d["authorization_request"].update(
                request_sha256="0" * 64)),
            ("receipt self", lambda d: d["authorization_receipt"].update(
                receipt_sha256="0" * 64)),
            ("ledger self", lambda d: d["authorization_ledger"].update(
                ledger_sha256="0" * 64)),
        )
        for expected, mutate in mutations:
            with self.subTest(expected=expected):
                document = self.document()
                mutate(document)
                with self.assertRaisesRegex(contract.ContractError, expected):
                    contract.validate_document(document)
        document = self.document()
        document["authorization_request"]["expected_ledger_sha256"] = "1" * 64
        with self.assertRaisesRegex(contract.ContractError, "genesis"):
            contract.validate_document(document)
        document = self.document()
        document["authorization_ledger"]["entries"][0]["sequence"] = 2
        with self.assertRaisesRegex(contract.ContractError, "start at one"):
            contract.validate_document(document)
        document = self.document()
        document["authorization_ledger"]["clock_high_water_unix_ms"] -= 2_000
        with self.assertRaisesRegex(contract.ContractError, "clock high-water"):
            contract.validate_document(document)
    def test_non_genesis_ledger_and_complete_revocation_branches(self):
        valid = _two_entry_candidate_for_tests(self.golden)
        profile_hash, root, _, _, _ = self.context(valid)
        self.assertEqual(contract.validate_document(valid), valid)
        cases = []
        bad = copy.deepcopy(valid)
        bad["authorization_ledger"]["entries"][1]["receipt"]["prior_receipt_sha256"] = "0" * 64
        cases.append(("prior digest chain", _reseal_two_entry_ledger_for_tests(bad)))
        bad = copy.deepcopy(valid)
        bad["authorization_ledger"]["revocation_snapshots"][1]["prior_revocation_sha256"] = "0" * 64
        cases.append(("revocation prior digest", _reseal_two_entry_ledger_for_tests(bad)))
        bad = copy.deepcopy(valid)
        ledger = bad["authorization_ledger"]
        ledger["revocation_high_water_sequence"] = 1
        ledger["ledger_sha256"] = contract.ledger_sha256(ledger)
        cases.append(("revocation high-water", ledger))
        bad = copy.deepcopy(valid)
        first, second = bad["authorization_ledger"]["revocation_snapshots"]
        first["revoked_approval_ids"] = ["approval-record-" + "0" * 64]
        first["revocation_sha256"] = contract.revocation_sha256(first)
        second["prior_revocation_sha256"] = first["revocation_sha256"]
        cases.append(("monotonic", _reseal_two_entry_ledger_for_tests(bad)))
        bad = copy.deepcopy(valid)
        ledger = bad["authorization_ledger"]
        ledger["entries"][1]["request"]["idempotency_key"] = ledger["entries"][0][
            "request"]["idempotency_key"]
        cases.append(("duplicate idempotency", _reseal_two_entry_ledger_for_tests(bad)))
        bad = copy.deepcopy(valid)
        ledger = bad["authorization_ledger"]
        ledger["entries"][1]["request"]["approval_records"] = copy.deepcopy(ledger[
            "entries"][0]["request"]["approval_records"])
        ledger["entries"][1]["receipt"].update(
            authorization_decision="acceptance_transition_authorized",
            qualifying_approval_ids=ledger["entries"][0]["receipt"]["qualifying_approval_ids"],
            reason_codes=[])
        cases.append(("two authorized", _reseal_two_entry_ledger_for_tests(bad)))
        bad = copy.deepcopy(valid)
        ledger = bad["authorization_ledger"]
        first, second = ledger["entries"]
        _bind_entry_to_snapshot_for_tests(first, ledger["revocation_snapshots"][1])
        second["receipt"]["prior_receipt_sha256"] = first["receipt"]["receipt_sha256"]
        _bind_entry_to_snapshot_for_tests(second, ledger["revocation_snapshots"][0])
        ledger["ledger_sha256"] = contract.ledger_sha256(ledger)
        cases.append(("nondecreasing", ledger))
        for expected, candidate in cases:
            with self.subTest(expected=expected), self.assertRaisesRegex(
                    contract.ContractError, expected):
                contract.validate_ledger(candidate, profile_hash, root)
    def test_declared_failure_precedence_and_qualifying_ids_are_exact(self):
        records = copy.deepcopy(self.golden["authorization_request"]["approval_records"])
        records[1]["decision"] = "reject"
        policy = copy.deepcopy(self.golden["authorization_policy"])
        self.assertEqual(declared_reason_codes(policy, records), ["authenticated_reject"])
        self.assertEqual(declared_outcome(policy, records)[1], [records[0]["approval_id"]])
        policy["disposition"] = "deny"
        self.assertEqual(declared_reason_codes(policy, records), ["policy_denied"])
        self.assertEqual(declared_outcome(policy, records)[1], [])
        self.assertEqual(declared_reason_codes(self.golden["authorization_policy"], records[:1]),
                         ["insufficient_authenticated_approvals"])
    def test_candidate_explicitly_has_no_accepted_or_effect_authority(self):
        proposal = (ROOT / PROPOSAL_FIXTURE_PATH).read_bytes()
        _, metadata = decode_proposal_document(self.golden["proposal_document_base64url"],
                                               self.golden["proposal_binding"], "proposal")
        self.assertEqual(metadata["status"], "proposed")
        self.assertIsNone(metadata["acceptance_id"])
        self.assertIsNone(metadata["accepted_at_unix_ms"])
        self.assertNotIn("effect_id", self.golden["authorization_receipt"])
        self.assertNotIn("accepted_document", self.golden)
        self.assertNotIn(b"forge accept", proposal)
    def test_exact_n_and_n_plus_one_resource_boundaries(self):
        self.assertEqual(len(bounded_canonical_json([None] * MAX_ARRAY_ITEMS, 4096,
                                                    "max array")), 5 * MAX_ARRAY_ITEMS + 1)
        with self.assertRaisesRegex(contract.ContractError, "array item count"):
            bounded_canonical_json([None] * (MAX_ARRAY_ITEMS + 1), 4096, "large array")
        object_64 = {f"k{index}": None for index in range(64)}
        bounded_canonical_json(object_64, 4096, "max object")
        with self.assertRaisesRegex(contract.ContractError, "object field count"):
            bounded_canonical_json({f"k{index}": None for index in range(65)},
                                   4096, "large object")
        bounded_canonical_json("x" * MAX_STRING_BYTES, MAX_STRING_BYTES + 2,
                               "max string")
        with self.assertRaisesRegex(contract.ContractError, "string byte length"):
            bounded_canonical_json("x" * (MAX_STRING_BYTES + 1),
                                   MAX_STRING_BYTES + 3, "large string")
        value = None
        for _ in range(15):
            value = [value]
        bounded_canonical_json(value, 64, "max depth")
        with self.assertRaisesRegex(contract.ContractError, "depth exceeds"):
            bounded_canonical_json([value], 64, "large depth")
        self.assertTrue(encode_proposal_document(b"x" * (256 * 1024)))
        with self.assertRaisesRegex(contract.ContractError, "262144"):
            encode_proposal_document(b"x" * (256 * 1024 + 1))
        contract.record_key_sha256("a" * 16)
        contract.record_key_sha256("a" * 128)
        for key in ("a" * 15, "a" * 129):
            with self.assertRaisesRegex(contract.ContractError, "16..128"):
                contract.record_key_sha256(key)
    def test_root_accepts_sixteen_approvers_and_rejects_seventeen(self):
        profile_hash = self.golden["signature_profile"]["profile_sha256"]
        root = copy.deepcopy(self.golden["trust_root"])
        for index in range(3, 17):
            root["keys"].append({
                "key_id": f"fixture-extra-approver-key-{index:02d}",
                "principal": {"authority_domain": root["trust_domain"],
                              "principal_id": f"operator-extra-{index:02d}",
                              "principal_type": "operator"},
                "public_key_base64url": base64.urlsafe_b64encode(
                    bytes([index + 20]) * 32).decode().rstrip("="),
                "usage": "architecture_approval_sign",
            })
        root["keys"].sort(key=contract.canonical_json)
        root["root_sha256"] = trust_root_sha256(root)
        self.assertEqual(len(validate_trust_root(root, profile_hash)["keys"]), 20)
        root["keys"].append(copy.deepcopy(root["keys"][0]))
        root["keys"][-1]["key_id"] = "fixture-over-limit-key"
        root["keys"].sort(key=contract.canonical_json)
        root["root_sha256"] = trust_root_sha256(root)
        with self.assertRaisesRegex(contract.ContractError, "6..20"):
            validate_trust_root(root, profile_hash)

def _reseal_with_revoked_key(document, key_id):
    snapshot = document["revocation_snapshot"]
    snapshot["revoked_key_ids"] = [key_id]
    snapshot["revocation_sha256"] = contract.revocation_sha256(snapshot)
    request = document["authorization_request"]
    request["revocation_sha256"] = snapshot["revocation_sha256"]
    request["request_id"] = request["request_sha256"] = ""
    request_digest = contract.request_sha256(request)
    request["request_id"] = f"architecture-decision-approval-request-{request_digest}"
    request["request_sha256"] = request_digest
    receipt = document["authorization_receipt"]
    receipt["request_sha256"] = request_digest
    receipt["revocation_sha256"] = snapshot["revocation_sha256"]
    receipt["receipt_id"] = receipt["receipt_sha256"] = ""
    receipt_digest = contract.receipt_sha256(receipt)
    receipt["receipt_id"] = f"architecture-decision-approval-receipt-{receipt_digest}"
    receipt["receipt_sha256"] = receipt_digest
    document["authorization_result"]["receipt"] = copy.deepcopy(receipt)
    ledger = document["authorization_ledger"]
    ledger["revocation_high_water_sha256"] = snapshot["revocation_sha256"]
    ledger["revocation_snapshots"] = [copy.deepcopy(snapshot)]
    ledger["entries"][0]["request"] = copy.deepcopy(request)
    ledger["entries"][0]["receipt"] = copy.deepcopy(receipt)
    ledger["ledger_sha256"] = contract.ledger_sha256(ledger)

if __name__ == "__main__":
    unittest.main()
