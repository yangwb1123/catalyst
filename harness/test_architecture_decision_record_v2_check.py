#!/usr/bin/env python3
"""Adversarial tests for the proposed-only ArchitectureDecisionRecord v2."""

from __future__ import annotations

import copy
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

from architecture_decision_record_v2 import (  # noqa: E402
    ContractError, GOLDEN, body_digest, canonical_json, self_digest,
    validate_document_bytes, validate_document_file, validate_golden,
)
from architecture_decision_record_v2.document import split_document  # noqa: E402


class ADRV2ContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.path = ROOT / GOLDEN
        cls.raw = cls.path.read_bytes()
        frontmatter, body = split_document(cls.raw)
        cls.metadata = json.loads(frontmatter)
        cls.body = body

    def render(self, mutate=None, body=None, name=None):
        metadata = copy.deepcopy(self.metadata)
        if name is not None:
            metadata["document_name"] = name
        if mutate is not None:
            mutate(metadata)
        body = self.body if body is None else body
        metadata["body_sha256"] = body_digest(body)
        metadata["self_sha256"] = ""
        metadata["self_sha256"] = self_digest(metadata, body)
        return b"---\n" + canonical_json(metadata) + b"\n---\n\n" + body

    def assert_invalid(self, raw, name="ADR-9001-proposed-boundary.md"):
        with self.assertRaises(ContractError):
            validate_document_bytes(raw, name)

    def test_golden_and_explicit_cli_are_valid(self):
        self.assertEqual(validate_golden(ROOT)["status"], "proposed")
        self.assertEqual(validate_document_file(self.path)["adr_id"], "ADR-9001")
        for args in (("--golden", str(ROOT)), ("--file", str(self.path))):
            result = subprocess.run(
                [sys.executable, "-B", str(HARNESS / "architecture_decision_record_v2_check.py"), *args],
                cwd=ROOT, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("VALID_PROPOSED_ARCHITECTURE_DECISION_RECORD_V2", result.stdout)

    def test_golden_pin_and_physical_basename_are_authoritative(self):
        mutated = self.raw.replace(b"byte-comparable", b"byte comparable", 1)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            destination = root / GOLDEN
            destination.parent.mkdir(parents=True)
            destination.write_bytes(mutated)
            with self.assertRaisesRegex(ContractError, "golden fixture bytes drifted"):
                validate_golden(root)
        self.assert_invalid(self.raw, "ADR-9002-proposed-boundary.md")

    def test_framing_bom_cr_multiline_and_trailing_bytes_are_rejected(self):
        self.assert_invalid(b"\xef\xbb\xbf" + self.raw)
        self.assert_invalid(self.raw.replace(b"---\n", b"---\r\n", 1))
        self.assert_invalid(self.raw.replace(b'null,"accepted', b'null,\n"accepted', 1))
        self.assert_invalid(self.raw + b"\n")
        self.assert_invalid(self.raw[:-1] + b" \n")

    def test_duplicate_unknown_noncanonical_and_floating_json_are_rejected(self):
        duplicate = self.raw.replace(
            b'{"acceptance_id":null,', b'{"acceptance_id":null,"acceptance_id":null,', 1)
        self.assert_invalid(duplicate)
        self.assert_invalid(self.render(lambda value: value.update(ambient_authority="none")))
        self.assert_invalid(self.raw.replace(b',"accepted_at', b', "accepted_at', 1))
        floating = self.raw.replace(b'"proposed_at_unix_ms":1786492800000',
                                    b'"proposed_at_unix_ms":1.5', 1)
        self.assert_invalid(floating)

    def test_body_and_self_digest_mutations_are_rejected(self):
        self.assert_invalid(self.raw.replace(b"Architecture decisions", b"Architecture decisionz", 1))
        body_hash = self.metadata["body_sha256"].encode()
        self.assert_invalid(self.raw.replace(body_hash, b"0" * 64, 1))
        self_hash = self.metadata["self_sha256"].encode()
        self.assert_invalid(self.raw.replace(self_hash, b"f" * 64, 1))

    def test_filename_adr_id_and_heading_must_agree(self):
        renamed = "ADR-9002-proposed-boundary.md"
        self.assert_invalid(self.render(name=renamed), renamed)
        self.assert_invalid(self.render(lambda value: value.update(adr_id="ADR-9002")))
        body = self.body.replace(b"# ADR-9001:", b"# ADR-9002:", 1)
        self.assert_invalid(self.render(body=body))
        self.assert_invalid(self.raw, "ADR-0000-invalid.md")
        self.assert_invalid(self.raw, "ADR-9001_Invalid.md")

    def test_body_requires_exact_order_nonempty_sections_and_no_extra_h2(self):
        swapped = self.body.replace(b"## Context", b"## Placeholder", 1)
        swapped = swapped.replace(b"## Decision", b"## Context", 1)
        swapped = swapped.replace(b"## Placeholder", b"## Decision", 1)
        self.assert_invalid(self.render(body=swapped))
        empty = self.body.replace(
            b"Architecture decisions need a deterministic proposed-state document that can be checked without consulting ambient authority or legacy ADR files.", b"", 1)
        self.assert_invalid(self.render(body=empty))
        extra = self.body.replace(b"\n\n## Limitations", b"\n\n## Extra\nNo.\n\n## Limitations", 1)
        self.assert_invalid(self.render(body=extra))
        indented = self.body.replace(
            b"\n\n## Limitations", b"\n\n ## Extra\nNo.\n\n## Limitations", 1)
        self.assert_invalid(self.render(body=indented))
        setext = self.body.replace(
            b"\n\n## Limitations", b"\n\nExtra\n-----\nNo.\n\n## Limitations", 1)
        self.assert_invalid(self.render(body=setext))

    def test_body_spacing_final_lf_and_line_trailing_space_are_exact(self):
        self.assert_invalid(self.render(body=self.body.replace(
            b"\n\n## Context", b"\n## Context", 1)))
        self.assert_invalid(self.render(body=self.body.replace(
            b"\n\n## Decision", b"\n## Decision", 1)))
        self.assert_invalid(self.render(body=self.body.replace(
            b"## Context\n", b"## Context \n", 1)))
        self.assert_invalid(self.render(body=self.body + b"\n"))

    def test_reference_sets_require_bounds_sorting_uniqueness_and_grammars(self):
        self.assert_invalid(self.render(lambda value: value.update(owner_refs=[])))
        self.assert_invalid(self.render(
            lambda value: value.update(owner_refs=["role:z", "role:a"])))
        self.assert_invalid(self.render(
            lambda value: value.update(approver_refs=["Role:Reviewer"])))
        self.assert_invalid(self.render(
            lambda value: value.update(affected_node_ids=["graph-node-short"])))
        self.assert_invalid(self.render(
            lambda value: value.update(implementation_refs=["docs/../secret.md"])))
        self.assert_invalid(self.render(
            lambda value: value.update(implementation_refs=[".git/config"])))
        self.assert_invalid(self.render(
            lambda value: value.update(implementation_refs=["src/main.go#L2147483648"])))
        self.assert_invalid(self.render(
            lambda value: value.update(context_claim_ids=["x" * 161])))

    def test_proposed_only_transition_fields_are_closed(self):
        mutations = [
            lambda value: value.update(status="accepted"),
            lambda value: value.update(accepted_at_unix_ms=1),
            lambda value: value.update(acceptance_id="approval:one"),
            lambda value: value.update(superseded_by=["ADR-9002"]),
        ]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                self.assert_invalid(self.render(mutate))

    def test_expiry_is_optional_and_strictly_after_proposal(self):
        expires = self.metadata["proposed_at_unix_ms"] + 1
        valid = self.render(lambda value: value.update(expires_at_unix_ms=expires))
        metadata = validate_document_bytes(valid, self.path.name)
        self.assertEqual(metadata["expires_at_unix_ms"], expires)
        self.assert_invalid(self.render(lambda value: value.update(
            expires_at_unix_ms=value["proposed_at_unix_ms"])))
        self.assert_invalid(self.render(lambda value: value.update(expires_at_unix_ms=-1)))

    def test_supersedes_is_declarative_sorted_and_cannot_reference_self(self):
        valid = self.render(lambda value: value.update(supersedes=["ADR-0001", "ADR-0002"]))
        self.assertEqual(validate_document_bytes(valid, self.path.name)["status"], "proposed")
        self.assert_invalid(self.render(
            lambda value: value.update(supersedes=["ADR-0002", "ADR-0001"])))
        self.assert_invalid(self.render(
            lambda value: value.update(supersedes=["ADR-9001"])))
        self.assert_invalid(self.render(
            lambda value: value.update(supersedes=["ADR-0000"])))

    def test_structured_arrays_are_sorted_bounded_and_owner_bound(self):
        self.assert_invalid(self.render(
            lambda value: value["alternatives"].reverse()))
        self.assert_invalid(self.render(
            lambda value: value["alternatives"][1].update(disposition="candidate")))
        self.assert_invalid(self.render(
            lambda value: value["validation_plan"][0].update(owner_ref="role:other")))
        self.assert_invalid(self.render(
            lambda value: value["validation_plan"][0].update(evidence_required=[])))
        self.assert_invalid(self.render(
            lambda value: value["revisit_triggers"][0].update(evidence_required="not-an-array")))
        two = copy.deepcopy(self.metadata["revisit_triggers"][0])
        two["trigger_id"] = "aaa-earlier"
        self.assert_invalid(self.render(
            lambda value: value["revisit_triggers"].append(two)))
        risk = {"description": "Risk.", "mitigation": "Mitigate.", "risk_id": "risk-one"}
        valid = self.render(lambda value: value.update(risks=[risk]))
        self.assertEqual(validate_document_bytes(valid, self.path.name)["risks"], [risk])

    def test_narrative_order_is_preserved_not_set_normalized(self):
        valid = self.render(lambda value: value["consequences"].reverse())
        metadata = validate_document_bytes(valid, self.path.name)
        self.assertTrue(metadata["consequences"][0].startswith("Declared owners"))

    def test_required_narratives_reject_whitespace_only_values(self):
        self.assert_invalid(self.render(lambda value: value.update(title="   ")))
        self.assert_invalid(self.render(lambda value: value.update(decision="\u00a0")))
        self.assert_invalid(self.render(lambda value: value["validation_plan"][0].update(
            evidence_required=["   "])))

    def test_document_frontmatter_body_depth_and_array_bounds_fail_closed(self):
        self.assert_invalid(b"---\n" + b"x" * (64 * 1024 + 1) + b"\n---\n\nx\n")
        frontmatter, _ = split_document(self.raw)
        self.assert_invalid(b"---\n" + frontmatter + b"\n---\n\n" + b"x" * (192 * 1024 + 1))
        nested = b'{"a":' * 17 + b"null" + b"}" * 17
        self.assert_invalid(b"---\n" + nested + b"\n---\n\nx\n")
        with self.assertRaisesRegex(ContractError, "array exceeds 64"):
            self.render(lambda value: value.update(consequences=["x"] * 65))
        with self.assertRaisesRegex(ContractError, "exceeds 4096"):
            self.render(lambda value: value.update(decision="x" * 4097))

    def test_forbidden_unicode_and_controls_are_rejected(self):
        bidi = self.raw.replace(b"Proposed Boundary Fixture", "Proposed\u202eBoundary".encode(), 1)
        self.assert_invalid(bidi)
        control = self.raw.replace(b"Legacy ADR files", b"Legacy\\u0000ADR files", 1)
        self.assert_invalid(control)
        tabbed_body = self.body.replace(b"Architecture decisions", b"Architecture\tdecisions", 1)
        self.assert_invalid(self.render(body=tabbed_body))
        bidi_body = self.body.replace(b"Architecture decisions", "Architecture\u202edecisions".encode(), 1)
        self.assert_invalid(self.render(body=bidi_body))
        c1_body = self.body.replace(b"Architecture decisions", "Architecture\u0085decisions".encode(), 1)
        self.assert_invalid(self.render(body=c1_body))

    def test_declared_responsibility_refs_create_no_ambient_authority(self):
        def missing_refs(value):
            value["owner_refs"] = ["principal:does-not-exist"]
            value["approver_refs"] = ["principal:also-missing"]
            value["validation_plan"][0]["owner_ref"] = "principal:does-not-exist"

        raw = self.render(missing_refs)
        metadata = validate_document_bytes(raw, self.path.name)
        self.assertEqual(metadata["accepted_at_unix_ms"], None)
        self.assertEqual(metadata["acceptance_id"], None)
        self.assertNotIn("approval_record", metadata)

    def test_legacy_adr_files_are_never_scanned(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            destination = root / GOLDEN
            destination.parent.mkdir(parents=True)
            destination.write_bytes(self.raw)
            legacy = root / "docs/adr/0001-legacy.md"
            legacy.parent.mkdir(parents=True)
            legacy.write_bytes(b"not an ADR v2 document\r\n")
            self.assertEqual(validate_golden(root)["adr_id"], "ADR-9001")


if __name__ == "__main__":
    unittest.main()
