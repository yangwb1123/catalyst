"""Adversarial tests for the ADR-0057 signed issuance document."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bootstrap_grant_issuance_contract as contract
from bootstrap_grant_issuance_contract.constants import (GOLDEN_FILE_SHA256,
                                                         MAX_GOLDEN_BYTES)
from bootstrap_grant_issuance_contract.documents import (policy_sha256, receipt_sha256,
                                                         request_sha256)
from bootstrap_grant_issuance_contract.ledger import ledger_sha256

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "docs/contracts/fixtures/bootstrap-grant-issuance-v1.json"


class BootstrapGrantIssuanceContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = contract.load_fixture(ROOT)

    def document(self) -> dict[str, object]:
        return copy.deepcopy(self.golden)

    def test_golden_exact_digests_and_dependency_free_cli(self):
        expected = {
            "signature_profile": ("profile_sha256", "b4a3662880bddc7e49682d264f89ededa31804c9795cba447dd63ce591a8bf1b"),
            "trust_root": ("root_sha256", "35b180615e7e4784e06d4c5618370904cd5b6824bc7e541bfeab62939891a27e"),
            "policy": ("policy_sha256", "d11e3b1e62056feba784f0183581eceb31a99401229e35a0bed277dc7982d803"),
            "request": ("request_sha256", "c839335c5a37f1a7d1efe2fe69d145accf4e71add05aaf092dda0943fabb968f"),
            "grant": ("grant_sha256", "3ec289c184e8b1907aedd07e19874f839ef2d66170a4e4fced9cc5ef28dec8a7"),
            "receipt": ("receipt_sha256", "9943cf3fe469005d99b6932f496ac7c2c89ad541502350a49512ce8ec475131a"),
            "ledger": ("ledger_sha256", "6e33c6433606f4ff9aebc28cbf750b3e33d70bed1f91aca1729293d84ad6c186"),
        }
        for artifact, (field, digest) in expected.items():
            self.assertEqual(self.golden[artifact][field], digest)
        self.assertEqual(hashlib.sha256(FIXTURE.read_bytes()).hexdigest(), GOLDEN_FILE_SHA256)
        result = subprocess.run(
            [sys.executable, "-S", "-B",
             str(ROOT / "harness/bootstrap_grant_issuance_contract/check.py"),
             "--golden", str(ROOT)], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ed25519 NOT authenticated", result.stdout)

    def test_schema_resolves_adr0056_offline_and_missing_registry_fails_closed(self):
        try:
            from jsonschema import Draft202012Validator
            from referencing import Registry, Resource
        except ImportError as error:
            self.skipTest(f"optional jsonschema/referencing unavailable: {error}")
        schema = json.loads((ROOT / "docs/contracts/bootstrap-grant-issuance-v1.schema.json").read_text())
        grant_schema = json.loads((ROOT / "docs/contracts/capability-grant-v1.schema.json").read_text())
        registry = Registry().with_resource(grant_schema["$id"],
                                            Resource.from_contents(grant_schema))
        validator = Draft202012Validator(schema, registry=registry)
        validator.validate(self.golden)
        bad_public = self.document()
        public = bad_public["trust_root"]["keys"][0]["public_key_base64url"]
        bad_public["trust_root"]["keys"][0]["public_key_base64url"] = public[:-1] + "B"
        self.assertTrue(list(validator.iter_errors(bad_public)))
        bad_signature = self.document()
        signature = bad_signature["policy"]["signature"]["signature_base64url"]
        bad_signature["policy"]["signature"]["signature_base64url"] = signature[:-1] + "B"
        self.assertTrue(list(validator.iter_errors(bad_signature)))
        bad_path = self.document()
        bad_path["request"]["scope"]["allow"][0]["resources"][0]["path"] = "docs/\u202efile"
        self.assertTrue(list(validator.iter_errors(bad_path)))
        for location in (("request",), ("ledger", "entries", 0, "request")):
            bad_revision = self.document()
            request = bad_revision
            for component in location:
                request = request[component]
            request["bindings"]["source_revision"] = "x" * 161
            self.assertTrue(list(validator.iter_errors(bad_revision)))
        with self.assertRaises(Exception) as caught:
            Draft202012Validator(schema).validate(self.golden)
        failure = type(caught.exception).__name__ + " " + str(caught.exception)
        self.assertIn("Unresolvable", failure)

    def test_strict_json_duplicate_noncanonical_number_depth_and_size(self):
        raw = FIXTURE.read_bytes().rstrip(b"\n")
        duplicate = b'{"grant":null,' + raw[1:]
        with self.assertRaisesRegex(contract.ContractError, "duplicate JSON key"):
            contract.decode_document(duplicate)
        pretty = json.dumps(self.golden, indent=2).encode()
        with self.assertRaisesRegex(contract.ContractError, "exact compact canonical JSON"):
            contract.decode_document(pretty)
        with self.assertRaisesRegex(contract.ContractError, "non-integer"):
            contract.decode_document(raw.replace(b'"trust_epoch":1',
                                                 b'"trust_epoch":1.0', 1))
        deep = b'{"a":' * 17 + b'null' + b'}' * 17
        with self.assertRaisesRegex(contract.ContractError, "depth exceeds"):
            contract.decode_document(deep)
        with self.assertRaisesRegex(contract.ContractError, "byte length"):
            contract.decode_document(b"x" * (MAX_GOLDEN_BYTES + 1))

    def test_unknown_control_bidi_int64_and_programmatic_key_rejected(self):
        document = self.document()
        document["unexpected"] = True
        with self.assertRaisesRegex(contract.ContractError, "unexpected fields"):
            contract.validate_document(document)

    def test_object_keys_share_the_string_byte_ceiling(self):
        oversized_key = "a" * 16_385
        document = self.document()
        document[oversized_key] = None
        with self.assertRaisesRegex(contract.ContractError, "object key byte length"):
            contract.canonical_json(document)
        raw = json.dumps({oversized_key: None}, separators=(",", ":")).encode()
        with self.assertRaisesRegex(contract.ContractError, "object key byte length"):
            contract.decode_document(raw)
        for bad in ("bad\u0001text", "bad\u202etext", "bad\u2028text"):
            document = self.document()
            document["policy"]["policy_id"] = bad
            with self.subTest(bad=repr(bad)), self.assertRaisesRegex(
                    contract.ContractError, "forbidden control"):
                contract.validate_document(document)
        document = self.document()
        document["trust_root"]["trust_epoch"] = 2**63
        with self.assertRaisesRegex(contract.ContractError, "signed int64"):
            contract.validate_document(document)
        document = self.document()
        document[1] = "non-string-key"
        with self.assertRaisesRegex(contract.ContractError, "unexpected fields"):
            contract.validate_document(document)

    def test_keys_usages_principals_and_canonical_base64_are_closed(self):
        document = self.document()
        document["trust_root"]["keys"][0]["usage"] = "request_auth"
        with self.assertRaisesRegex(contract.ContractError, "usage order"):
            contract.validate_document(document)
        document = self.document()
        document["trust_root"]["keys"][1]["principal"] = copy.deepcopy(
            document["trust_root"]["keys"][0]["principal"])
        with self.assertRaisesRegex(contract.ContractError, "pairwise distinct"):
            contract.validate_document(document)
        document = self.document()
        public = document["trust_root"]["keys"][0]["public_key_base64url"]
        document["trust_root"]["keys"][0]["public_key_base64url"] = public[:-1] + "B"
        with self.assertRaisesRegex(contract.ContractError, "encode exactly"):
            contract.validate_document(document)

    def test_python_checker_explicitly_does_not_authenticate_ed25519(self):
        document = self.document()
        document["policy"]["signature"]["signature_base64url"] = "A" * 86
        document["ledger"]["entries"][0]["policy"] = copy.deepcopy(document["policy"])
        document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])
        self.assertEqual(contract.validate_document(document)["policy"]["policy_sha256"],
                         self.golden["policy"]["policy_sha256"])
        document = self.document()
        signature = document["policy"]["signature"]["signature_base64url"]
        document["policy"]["signature"]["signature_base64url"] = signature[:-1] + "B"
        with self.assertRaisesRegex(contract.ContractError, "encode exactly"):
            contract.validate_document(document)

    def test_profile_scope_capability_environment_and_budget_are_closed(self):
        mutations = (
            ("profile", lambda d: d["policy"].update({"profile_id": "other"})),
            ("exact", lambda d: d["request"]["scope"]["allow"][0]["resources"][0].update(
                {"match": "subtree"})),
            ("repository-reader", lambda d: d["request"]["capability"].update(
                {"capability_id": "writer"})),
            ("environment_class", lambda d: d["request"]["task_binding"].update(
                {"environment_class": "production"})),
            ("hard budget", lambda d: d["request"]["budget"].update({"max_calls": 2})),
            ("source_revision", lambda d: d["request"]["bindings"].update(
                {"source_revision": "é" * 81})),
        )
        for expected, mutate in mutations:
            with self.subTest(expected=expected):
                document = self.document()
                mutate(document)
                with self.assertRaisesRegex(contract.ContractError, expected):
                    contract.validate_document(document)

    def test_source_revision_that_cannot_fit_the_grant_is_rejected_for_both_decisions(self):
        for denied in (False, True):
            with self.subTest(denied=denied):
                document = self.document()
                document["request"]["bindings"]["source_revision"] = "x" * 161
                if denied:
                    document = _denied_document(document)
                with self.assertRaisesRegex(contract.ContractError, "source_revision"):
                    contract.validate_document(document)

    def test_policy_request_exactness_freshness_ttl_and_idempotency(self):
        cases = (
            ("exact fields", lambda d: d["request"].update({"scope": {"allow": [
                {"resources": [d["request"]["scope"]["allow"][0]["resources"][0]]}],
                "deny": [], "effect_id": "repo.read"}})),
            ("five minutes", lambda d: d["request"].update(
                {"expires_at_unix_ms": d["request"]["requested_at_unix_ms"] + 300001})),
            ("visible ASCII", lambda d: d["request"].update({"idempotency_key": "short"})),
            ("1..3600000", lambda d: d["request"].update({"requested_ttl_ms": 3600001})),
        )
        for expected, mutate in cases:
            with self.subTest(expected=expected):
                document = self.document()
                mutate(document)
                _rehash_request_chain(document)
                with self.assertRaisesRegex(contract.ContractError, expected):
                    contract.validate_document(document)
        document = self.document()
        document["policy"]["max_ttl_ms"] = 1000
        _rehash_policy_request_chain(document)
        with self.assertRaisesRegex(contract.ContractError, "TTL exceeds"):
            contract.validate_document(document)
        document = self.document()
        document["policy"]["validity"]["expires_at_unix_ms"] = 1700000100000
        _rehash_policy_request_chain(document)
        with self.assertRaisesRegex(contract.ContractError, "outside Policy"):
            contract.validate_document(document)

    def test_structural_policy_denial_has_signed_shape_and_null_grant(self):
        document = _denied_document(self.document())
        self.assertIsNone(contract.validate_document(document)["grant"])
        self.assertEqual(document["receipt"]["denial_reason"], "policy_denied")
        broken = copy.deepcopy(document)
        broken.pop("receipt")
        with self.assertRaisesRegex(contract.ContractError, "unexpected fields"):
            contract.validate_document(broken)

    def test_public_digest_helpers_wrap_invalid_programmatic_inputs(self):
        with self.assertRaisesRegex(contract.ContractError, "must be an object"):
            policy_sha256([])
        with self.assertRaisesRegex(contract.ContractError, "lowercase SHA-256"):
            contract.signature_message(b"domain\0", None)
        with self.assertRaisesRegex(contract.ContractError, "visible ASCII"):
            contract.record_key_sha256("not valid")


def _rehash_request_chain(document: dict[str, object]) -> None:
    request = document["request"]
    request["request_sha256"] = request_sha256(request)
    receipt = document["receipt"]
    receipt["request_sha256"] = request["request_sha256"]
    receipt["receipt_sha256"] = receipt_sha256(receipt)
    document["result"]["receipt"] = copy.deepcopy(receipt)
    entry = document["ledger"]["entries"][0]
    entry["request"] = copy.deepcopy(request)
    entry["receipt"] = copy.deepcopy(receipt)
    document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])


def _rehash_policy_request_chain(document: dict[str, object]) -> None:
    policy = document["policy"]
    policy["policy_sha256"] = policy_sha256(policy)
    document["request"]["policy_sha256"] = policy["policy_sha256"]
    document["receipt"]["policy_sha256"] = policy["policy_sha256"]
    document["ledger"]["entries"][0]["policy"] = copy.deepcopy(policy)
    _rehash_request_chain(document)


def _denied_document(document: dict[str, object]) -> dict[str, object]:
    policy = document["policy"]
    policy["disposition"] = "deny"
    policy["policy_sha256"] = policy_sha256(policy)
    document["request"]["policy_sha256"] = policy["policy_sha256"]
    document["request"]["request_sha256"] = request_sha256(document["request"])
    receipt = document["receipt"]
    receipt.update({"decision": "denied", "denial_reason": "policy_denied",
                    "grant_envelope_sha256": None, "grant_id": None, "grant_sha256": None,
                    "policy_sha256": policy["policy_sha256"],
                    "request_sha256": document["request"]["request_sha256"]})
    receipt["receipt_sha256"] = receipt_sha256(receipt)
    document["grant"] = None
    document["result"].update({"grant": None, "receipt": copy.deepcopy(receipt)})
    entry = document["ledger"]["entries"][0]
    entry.update({"grant": None, "policy": copy.deepcopy(policy),
                  "receipt": copy.deepcopy(receipt), "request": copy.deepcopy(document["request"])})
    document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])
    return document


if __name__ == "__main__":
    unittest.main()
