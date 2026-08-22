from __future__ import annotations

import base64
import copy
import hashlib
import json
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "harness"))

from bootstrap_repo_read_execution_contract import (  # noqa: E402
    ContractError, canonical_json, decode_usage_ledger, load_fixture, terminal_replay,
    terminal_replay_from_documents, validate_document,
)
from bootstrap_repo_read_execution_contract.results import validate_delivery  # noqa: E402
from bootstrap_repo_read_execution_contract.shape import validate_path  # noqa: E402

FIXTURE = ROOT / "docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json"
FILE_SHA256 = "309b3da66c64669239ce40bd086cdcbb518d59dc7fd5e1bad60d6acf9107480d"
DIGESTS = {
    ("execution_trust_root", "root_sha256"): "ecdb7c3000c34ca55b05fe550562fe3fb17c8f616c4dd3676e477690a691b1e1",
    ("expected_manifest", "manifest_sha256"): "550bcf970dec5ff763507e27651e87777f47db780fc19af276ae3a3e58a72cd4",
    ("execution_policy", "execution_policy_sha256"): "7dae20cc0195fb947ca0894afeb889d50fdf80ff541b080b42d8974c03e71f9d",
    ("invocation", "invocation_sha256"): "b1f9e979d4685f6ffb4b888289f9fc2a10b410f6800505bcbc4205aafa4fcfdc",
    ("reserved_receipt", "receipt_sha256"): "6225715fa4ba8f4d453ba89b31cefa32c2b4adb113878d0ba8fcdeaf5b1f4cf4",
    ("effect_intent_receipt", "receipt_sha256"): "c69f8a80f7f16580e7a171d4ecdcc2f8c4abcf573522a560f45720597c446954",
    ("execution_result", "execution_result_sha256"): "5d67bc8a2913cc868bfa1ea204761ff6b7b64fbc804bd83b564062d7f173cc65",
    ("result_metadata", "metadata_sha256"): "e4a697dbc97fb5e5a3d1bfd8cf5f05893eb4ef4e16ac382108ba09bf14ec266a",
    ("completed_receipt", "receipt_sha256"): "2939e52a626e44c65b44a53d74a2e08069d2cce2d102457f85619f563fb28ba0",
    ("usage_ledger", "ledger_sha256"): "4bc0259d280e25c703ba26fc8b5156949eb1aab136960d14289d9c8d088e1c74",
}


class BootstrapRepoReadExecutionContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.golden = load_fixture(ROOT)

    def document(self) -> dict[str, object]:
        return copy.deepcopy(self.golden)

    def test_golden_hashes_top_shape_and_dependency_free_checker(self) -> None:
        self.assertEqual(len(self.golden), 18)
        for (artifact, field), digest in DIGESTS.items():
            self.assertEqual(self.golden[artifact][field], digest)
        self.assertEqual(hashlib.sha256(FIXTURE.read_bytes()).hexdigest(), FILE_SHA256)
        result = subprocess.run(
            [sys.executable, "-S", "-B",
             str(ROOT / "harness/bootstrap_repo_read_execution_contract/check.py"),
             "--golden", str(ROOT)], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ed25519, root pins, filesystem, durability, and effect NOT authenticated",
                      result.stdout)

    def test_schema_resolves_adr0056_and_adr0057_offline(self) -> None:
        try:
            from jsonschema import Draft202012Validator
            from referencing import Registry, Resource
        except ImportError as error:
            self.skipTest(f"optional jsonschema/referencing unavailable: {error}")
        schema = _json("docs/contracts/bootstrap-repo-read-execution-v1.schema.json")
        dependencies = [_json("docs/contracts/capability-grant-v1.schema.json"),
                        _json("docs/contracts/bootstrap-grant-issuance-v1.schema.json")]
        registry = Registry()
        for dependency in dependencies:
            registry = registry.with_resource(dependency["$id"],
                                              Resource.from_contents(dependency))
        validator = Draft202012Validator(schema, registry=registry)
        validator.validate(self.golden)
        bad = self.document()
        bad["expected_manifest"]["entries"][0]["unknown"] = True
        self.assertTrue(list(validator.iter_errors(bad)))
        path_validator = validator.evolve(schema=schema["$defs"]["path"])
        self.assertFalse(list(path_validator.iter_errors("资料/space name.bin")))
        for path in (".git/config", ".FORGE/state", "a/" * 256 + "z"):
            self.assertTrue(list(path_validator.iter_errors(path)))

    def test_binary_raw_content_and_canonical_base64_are_load_bearing(self) -> None:
        encoded = self.golden["execution_result"]["reads"][0]["content_base64url"]
        raw = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4))
        self.assertEqual(raw, b"\x00ADR-0058 fixture\n\xff")
        bad = self.document()
        bad["execution_result"]["reads"][0]["content_base64url"] += "="
        with self.assertRaises(ContractError):
            validate_document(bad)

    def test_only_raw_content_field_exceeds_generic_string_limit(self) -> None:
        canonical_json({"content_base64url": "A" * 20000})
        with self.assertRaises(ContractError):
            canonical_json({"source_revision": "A" * 20000})

    def test_manifest_paths_share_the_runtime_preflight_boundary(self) -> None:
        self.assertEqual(validate_path("资料/space name.bin", "path"),
                         "资料/space name.bin")
        self.assertEqual(validate_path("a/\u0085/b", "path"), "a/\u0085/b")
        for path in (".git/config", ".FORGE/state", "a\\b", "a/*",
                     "a/" * 256 + "z"):
            with self.subTest(path=path), self.assertRaises(ContractError):
                validate_path(path, "path")

    def test_exact_replay_is_content_free_and_ledger_only(self) -> None:
        receipt = self.golden["completed_receipt"]
        metadata = self.golden["result_metadata"]
        replay = copy.deepcopy(self.golden["first_delivery"])
        replay.update({"delivery_disposition": "exact_replay", "execution_result": None})
        validate_delivery(replay, receipt, metadata)
        looked_up = terminal_replay(
            self.golden["usage_ledger"], self.golden["execution_policy"]["execution_policy_sha256"],
            self.golden["invocation"]["invocation_sha256"])
        self.assertEqual(looked_up, {"execution_result": None, "receipt": receipt,
                                     "result_metadata": metadata})
        from_documents = terminal_replay_from_documents(
            self.golden["usage_ledger"], canonical_json(self.golden["execution_policy"]),
            canonical_json(self.golden["invocation"]))
        self.assertEqual(from_documents, looked_up)
        self.assertNotIn(b"content_base64url", canonical_json(self.golden["usage_ledger"]))

    def test_standalone_ledger_decode_needs_only_public_authority_chain(self) -> None:
        ledger = decode_usage_ledger(
            canonical_json(self.golden["usage_ledger"]), self.golden["signature_profile"],
            self.golden["issuance_trust_root"], self.golden["issuance_ledger"],
            self.golden["execution_trust_root"])
        self.assertEqual(ledger, self.golden["usage_ledger"])

    def test_execution_root_is_distinct_and_context_is_only_opaque_digest(self) -> None:
        issuance = {key["public_key_base64url"]
                    for key in self.golden["issuance_trust_root"]["keys"]}
        execution = {key["public_key_base64url"]
                     for key in self.golden["execution_trust_root"]["keys"]}
        self.assertTrue(issuance.isdisjoint(execution))
        self.assertEqual(set(self.golden["execution_policy"]["bindings"]),
                         {"context_sha256", "source_revision", "source_tree_sha256"})
        bad = self.document()
        bad["execution_trust_root"]["keys"][0]["public_key_base64url"] = next(iter(issuance))
        with self.assertRaises(ContractError):
            validate_document(bad)

    def test_python_accepts_signature_shaped_non_authentic_policy(self) -> None:
        document = self.document()
        shaped = "A" * 86
        document["execution_policy"]["signature"]["signature_base64url"] = shaped
        document["usage_ledger"]["entries"][0]["execution_policy"]["signature"][
            "signature_base64url"] = shaped
        from bootstrap_repo_read_execution_contract.ledger import ledger_sha256
        document["usage_ledger"]["ledger_sha256"] = ""
        document["usage_ledger"]["signature"]["signature_base64url"] = ""
        document["usage_ledger"]["ledger_sha256"] = ledger_sha256(document["usage_ledger"])
        document["usage_ledger"]["signature"]["signature_base64url"] = shaped
        validate_document(document)

    def test_noncanonical_bytes_and_unknown_fields_fail_closed(self) -> None:
        raw = FIXTURE.read_bytes().rstrip(b"\n")
        with self.assertRaises(ContractError):
            from bootstrap_repo_read_execution_contract.contract import decode_document
            decode_document(b" " + raw)
        with self.assertRaises(ContractError):
            decode_document(b'{"a":1,"a":2}')
        bad = self.document()
        bad["ContextPackage"] = None
        with self.assertRaises(ContractError):
            validate_document(bad)


def _json(relative: str) -> dict[str, object]:
    return json.loads((ROOT / relative).read_text())


if __name__ == "__main__":
    unittest.main()
