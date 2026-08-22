"""Focused and adversarial tests for the frozen ADR-0086 contract core."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import unittest
from pathlib import Path

from legacy_governance_read_import_contract import (
    ContractError, decode_request, decode_view, make_request, project_request,
    validate_view_against_request,
)
from legacy_governance_read_import_contract.canonical import canonical_json, digest, self_digest, sha256_bytes
from legacy_governance_read_import_contract.constants import (
    ADR_KIND, CANDIDATE_DOMAIN, CANDIDATE_ID_DOMAIN, MAX_ADR_BYTES,
    MAX_MEMORY_ENTRIES, MAX_REQUEST_BYTES, MAX_SOURCE_REF_BYTES, MAX_VIEW_BYTES,
    MEMORY_KIND, SOURCE_SET_DOMAIN, SUCCESS_MARKER, VIEW_DOMAIN,
)
from legacy_governance_read_import_contract.source import encode_base64url

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "docs" / "contracts" / "fixtures"
REQUEST = FIXTURES / "legacy-governance-read-import-request-v1.json"
GOLDEN = FIXTURES / "legacy-governance-read-import-view-v1.json"
SCHEMA = ROOT / "docs" / "contracts" / "legacy-governance-read-import-v1.schema.json"
MEMORY = FIXTURES / "legacy-governance-read-import-memory-v1.jsonl"
ADRS = (
    FIXTURES / "legacy-governance-read-import-ADR-0001.md",
    FIXTURES / "legacy-governance-read-import-ADR-0002.md",
)
BINDING = {
    "project_id": "fixture-project",
    "source_revision": "fixture-revision-0001",
    "source_tree_sha256": "0123456789abcdef" * 4,
}


def schema_validator(test: unittest.TestCase):
    try:
        import jsonschema
    except ImportError:
        test.skipTest("jsonschema is unavailable")
    schema = json.loads(SCHEMA.read_bytes())
    jsonschema.Draft202012Validator.check_schema(schema)
    return jsonschema, schema, jsonschema.Draft202012Validator(schema)


def definition_validator(jsonschema, schema: dict[str, object], definition: object):
    fragment = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": schema["$defs"],
        **definition,
    }
    return jsonschema.Draft202012Validator(fragment)


def fixture_sources(memory: bytes | None = None) -> list[tuple[str, str, bytes]]:
    return [
        (MEMORY_KIND, ".forge/memory.jsonl",
         MEMORY.read_bytes() if memory is None else memory),
        (ADR_KIND, "docs/adr/0001-legacy-database-choice.md", ADRS[0].read_bytes()),
        (ADR_KIND, "docs/adr/0002-legacy-replacement-claim.md", ADRS[1].read_bytes()),
    ]


def reseal_candidate(candidate: dict[str, object]) -> None:
    locator = {field: candidate[field] for field in
               ("ordinal", "request_sha256", "source_kind", "source_ref")}
    candidate["candidate_id"] = digest(
        CANDIDATE_ID_DOMAIN, locator, MAX_REQUEST_BYTES, "candidate locator")
    candidate["candidate_sha256"] = self_digest(
        CANDIDATE_DOMAIN, candidate, "candidate_sha256", MAX_VIEW_BYTES, "candidate")


def reseal_view(view: dict[str, object]) -> bytes:
    view["source_set_sha256"] = digest(
        SOURCE_SET_DOMAIN, view["sources"], MAX_VIEW_BYTES, "source descriptor set")
    view["view_sha256"] = self_digest(
        VIEW_DOMAIN, view, "view_sha256", MAX_VIEW_BYTES, "view")
    return canonical_json(view, MAX_VIEW_BYTES, "view")


def sealed_memory_only_view(count: int) -> bytes:
    view = json.loads(GOLDEN.read_bytes())
    request_sha = view["request_sha256"]
    source_ref = ".forge/memory.jsonl"
    candidates = []
    lines = []
    for ordinal in range(1, count + 1):
        entry = {"kind": "gap", "topic": f"topic-{ordinal:04d}", "detail": "d",
                 "iteration": ordinal, "created_at_unix": ordinal}
        raw = json.dumps(entry, separators=(",", ":")).encode("ascii")
        candidate = {
            "authority": None, "candidate_id": "", "candidate_sha256": "",
            "confidence": {"presence": "omitted", "raw_number_lexeme": None},
            "created_at_unix": ordinal, "current": False, "declared_kind": "gap",
            "declared_source": None, "declared_supersedes": None,
            "declared_topic": entry["topic"], "detail": "d", "hardness": "none",
            "instruction_allowed": False, "iteration": ordinal, "legacy_format": None,
            "ordinal": ordinal, "raw_byte_count": len(raw),
            "raw_bytes_base64url": encode_base64url(raw), "raw_sha256": sha256_bytes(raw),
            "request_sha256": request_sha, "source_kind": MEMORY_KIND,
            "source_ref": source_ref, "trust_state": "unverified_legacy",
        }
        reseal_candidate(candidate)
        candidates.append(candidate)
        lines.append(raw)
    source_raw = b"\n".join(lines) + b"\n"
    view["candidates"] = candidates
    view["conflict_sets"] = []
    view["declared_supersessions"] = []
    view["sources"] = [{"byte_count": len(source_raw),
                        "content_sha256": sha256_bytes(source_raw),
                        "source_kind": MEMORY_KIND, "source_ref": source_ref}]
    return reseal_view(view)


class GoldenTests(unittest.TestCase):
    def test_request_and_view_are_exact_goldens(self) -> None:
        request = make_request(BINDING, fixture_sources())
        self.assertEqual(request, REQUEST.read_bytes())
        projected = project_request(request) + b"\n"
        self.assertEqual(projected, GOLDEN.read_bytes())
        view = validate_view_against_request(projected[:-1], request)
        self.assertEqual(6, len(view["candidates"]))
        self.assertEqual(1, len(view["conflict_sets"]))
        self.assertEqual(1, len(view["declared_supersessions"]))

    def test_checker_is_stdin_only_and_stdout_exact(self) -> None:
        result = subprocess.run(
            [sys.executable, str(ROOT / "harness" /
                                 "legacy_governance_read_import_contract_check.py")],
            input=REQUEST.read_bytes(), capture_output=True, check=False,
        )
        self.assertEqual(0, result.returncode, result.stderr.decode())
        self.assertEqual(GOLDEN.read_bytes(), result.stdout)
        rejected = subprocess.run(
            [sys.executable, str(ROOT / "harness" /
                                 "legacy_governance_read_import_contract_check.py"), "path"],
            input=REQUEST.read_bytes(), capture_output=True, check=False,
        )
        self.assertEqual(2, rejected.returncode)
        self.assertEqual(b"", rejected.stdout)

    def test_checker_bad_type_and_malformed_input_fail_without_stdout(self) -> None:
        value = json.loads(REQUEST.read_bytes())
        value["sources"][0]["source_kind"] = []
        bad_type = json.dumps(value, sort_keys=True, separators=(",", ":")).encode() + b"\n"
        checker = str(ROOT / "harness" / "legacy_governance_read_import_contract_check.py")
        for raw in (bad_type, b"{"):
            result = subprocess.run([sys.executable, checker], input=raw,
                                    capture_output=True, check=False)
            self.assertEqual(2, result.returncode)
            self.assertEqual(b"", result.stdout)
            self.assertNotIn(b"Traceback", result.stderr)

    def test_raw_bytes_confidence_and_authority_are_preserved(self) -> None:
        view = decode_view(GOLDEN.read_bytes()[:-1])
        candidates = view["candidates"]
        self.assertEqual(
            [("omitted", None), ("explicit", "0"), ("explicit", "1e-7"),
             ("explicit", "1.000000")],
            [(item["confidence"]["presence"], item["confidence"]["raw_number_lexeme"])
             for item in candidates[:4]],
        )
        self.assertTrue(all(item["authority"] is None for item in candidates))
        self.assertTrue(all(item["current"] is False for item in candidates))
        self.assertTrue(all(item["hardness"] == "none" for item in candidates))
        self.assertTrue(all(item["instruction_allowed"] is False for item in candidates))
        self.assertEqual(["not_performed", "not_performed"],
                         [item["parsing"] for item in candidates[4:]])
        self.assertEqual(
            {"acceptance", "authority", "confidence_interpretation",
             "conflict_resolution", "currentness", "instruction_eligibility",
             "persistence", "runtime_effect", "source_authentication",
             "source_completeness", "status_interpretation", "truth",
             "winner_selection"},
            set(view["attestations"]),
        )
        self.assertTrue(all(value is False for value in view["attestations"].values()))


class SchemaTests(unittest.TestCase):
    def test_schema_accepts_both_exact_goldens(self) -> None:
        _, _, validator = schema_validator(self)
        validator.validate(json.loads(REQUEST.read_bytes()))
        validator.validate(json.loads(GOLDEN.read_bytes()))

    def test_success_marker_and_thirteen_false_semantics_are_frozen(self) -> None:
        _, schema, _ = schema_validator(self)
        metadata = schema["x-forgeos-authority-semantics"]
        attestations = json.loads(GOLDEN.read_bytes())["attestations"]
        self.assertEqual(SUCCESS_MARKER, metadata["positive_result"])
        self.assertEqual(attestations, metadata["attestations"])
        self.assertEqual(13, len(attestations))
        self.assertTrue(all(value is False for value in attestations.values()))

    def test_schema_kind_specific_source_bounds(self) -> None:
        jsonschema, schema, _ = schema_validator(self)
        source_validator = definition_validator(
            jsonschema, schema, {"$ref": "#/$defs/source"})
        descriptor_validator = definition_validator(
            jsonschema, schema, {"$ref": "#/$defs/source_descriptor"})
        request = json.loads(REQUEST.read_bytes())
        view = json.loads(GOLDEN.read_bytes())
        adr_source = copy.deepcopy(request["sources"][1])
        adr_source["byte_count"] = MAX_ADR_BYTES + 1
        adr_descriptor = copy.deepcopy(view["sources"][1])
        adr_descriptor["byte_count"] = MAX_ADR_BYTES + 1
        for validator, value in ((source_validator, adr_source),
                                 (descriptor_validator, adr_descriptor)):
            with self.assertRaises(jsonschema.ValidationError):
                validator.validate(value)
        adr_base64 = definition_validator(
            jsonschema, schema, {"$ref": "#/$defs/adr_base64url"})
        maximum_encoded = (MAX_ADR_BYTES * 4 + 2) // 3
        adr_base64.validate("A" * maximum_encoded)
        with self.assertRaises(jsonschema.ValidationError):
            adr_base64.validate("A" * (maximum_encoded + 1))

    def test_schema_source_and_candidate_cardinality_n_plus_one(self) -> None:
        jsonschema, schema, _ = schema_validator(self)
        request = json.loads(REQUEST.read_bytes())
        view = json.loads(GOLDEN.read_bytes())
        cases = (
            ("request", "sources", 0, request["sources"][0], 1),
            ("request", "sources", 1, request["sources"][1], 256),
            ("view", "sources", 0, view["sources"][0], 1),
            ("view", "sources", 1, view["sources"][1], 256),
            ("view", "candidates", 0, view["candidates"][0], MAX_MEMORY_ENTRIES),
            ("view", "candidates", 1, view["candidates"][4], 256),
        )
        for owner, field, index, value, maximum in cases:
            constraint = schema["$defs"][owner]["properties"][field]["allOf"][index]
            validator = definition_validator(jsonschema, schema, constraint)
            validator.validate([value] * maximum)
            with self.assertRaises(jsonschema.ValidationError):
                validator.validate([value] * (maximum + 1))

    def test_schema_rejects_forbidden_wire_text_everywhere(self) -> None:
        jsonschema, _, validator = schema_validator(self)
        request = json.loads(REQUEST.read_bytes())
        view = json.loads(GOLDEN.read_bytes())
        for forbidden in ("\n", "\r", "\u2028", "\u2029", "\u202e", "\x01"):
            values = []
            bad = copy.deepcopy(request)
            bad["binding"]["project_id"] = "a" + forbidden
            values.append(bad)
            bad = copy.deepcopy(request)
            bad["sources"][0]["source_ref"] += forbidden
            values.append(bad)
            bad = copy.deepcopy(view)
            bad["candidates"][0]["detail"] += forbidden
            values.append(bad)
            for value in values:
                with self.assertRaises(jsonschema.ValidationError):
                    validator.validate(value)

    def test_schema_rejects_pattern_final_terminators_and_bad_sha_lengths(self) -> None:
        jsonschema, _, validator = schema_validator(self)
        request = json.loads(REQUEST.read_bytes())
        cases = []
        for field, value in (("source_tree_sha256", "a" * 63 + "\n"),
                             ("source_tree_sha256", "a" * 63),
                             ("source_tree_sha256", "a" * 65)):
            bad = copy.deepcopy(request)
            bad["binding"][field] = value
            cases.append(bad)
        bad = copy.deepcopy(request)
        bad["sources"][0]["content_base64url"] += "\n"
        cases.append(bad)
        for malformed in ("A", "AB"):
            bad = copy.deepcopy(request)
            bad["sources"][0]["content_base64url"] = malformed
            cases.append(bad)
        view = json.loads(GOLDEN.read_bytes())
        view["candidates"][1]["confidence"]["raw_number_lexeme"] = "0\n"
        cases.append(view)
        for value in cases:
            with self.assertRaises(jsonschema.ValidationError):
                validator.validate(value)


class RequestAdversarialTests(unittest.TestCase):
    def test_cross_project_binding_changes_request_and_candidate_identity(self) -> None:
        first = REQUEST.read_bytes()
        second_binding = dict(BINDING, project_id="another-project")
        second = make_request(second_binding, fixture_sources())
        first_view = json.loads(project_request(first))
        second_view = json.loads(project_request(second))
        self.assertNotEqual(first_view["request_sha256"], second_view["request_sha256"])
        self.assertNotEqual(first_view["candidates"][0]["candidate_id"],
                            second_view["candidates"][0]["candidate_id"])

    def test_request_requires_exact_lf_framed_canonical_json(self) -> None:
        request = REQUEST.read_bytes()
        for raw in (request[:-1], request + b"\n", b" " + request, request[:-1] + b" \n"):
            with self.subTest(raw=raw[:4]):
                with self.assertRaises(ContractError):
                    decode_request(raw)

    def test_request_rejects_duplicate_unknown_order_and_bad_source_pin(self) -> None:
        request = REQUEST.read_bytes()
        duplicate = request.replace(b'{"api_version":',
                                    b'{"api_version":"duplicate","api_version":', 1)
        unknown = request.replace(b'{"api_version":', b'{"extra":false,"api_version":', 1)
        reordered = request.replace(b'{"api_version":', b'{"kind":"x","api_version":', 1)
        bad_pin = request.replace(b'18401766587cb448', b'08401766587cb448', 1)
        for raw in (duplicate, unknown, reordered, bad_pin):
            with self.assertRaises(ContractError):
                decode_request(raw)

    def test_memory_rejects_bad_framing_shape_and_confidence(self) -> None:
        valid = b'{"kind":"gap","topic":"t","detail":"d","iteration":1,' \
                b'"created_at_unix":2}\n'
        invalid = (
            valid[:-1],
            valid + b"\n",
            valid.replace(b"\n", b"\r\n"),
            valid.replace(b'"topic":"t",', b'"topic":"t","topic":"u",'),
            valid.replace(b'"topic":"t",', b'"topic":"t","unknown":1,'),
            valid.replace(b'"created_at_unix":2', b'"confidence":1.1,"created_at_unix":2'),
            b"\xff\n",
        )
        for memory in invalid:
            with self.subTest(memory=memory[-16:]):
                with self.assertRaises(ContractError):
                    project_request(make_request(BINDING, fixture_sources(memory)))

    def test_source_ref_adr_size_and_count_boundaries(self) -> None:
        exact_ref = "r" * MAX_SOURCE_REF_BYTES
        project_request(make_request(BINDING, [(ADR_KIND, exact_ref, b"x\n")]))
        with self.assertRaises(ContractError):
            make_request(BINDING, [(ADR_KIND, exact_ref + "r", b"x\n")])
        exact_adr = b"x" * (MAX_ADR_BYTES - 1) + b"\n"
        project_request(make_request(BINDING, [(ADR_KIND, "exact", exact_adr)]))
        with self.assertRaises(ContractError):
            project_request(make_request(
                BINDING, [(ADR_KIND, "too-large", b"x" * MAX_ADR_BYTES + b"\n")]))
        exact_sources = [(ADR_KIND, f"adr-{index:03d}", b"x\n") for index in range(256)]
        project_request(make_request(BINDING, exact_sources))
        with self.assertRaises(ContractError):
            make_request(BINDING, exact_sources + [(ADR_KIND, "adr-256", b"x\n")])

    def test_confidence_lexeme_boundary_is_exact_and_not_float_normalized(self) -> None:
        def memory(lexeme: str) -> bytes:
            return (b'{"kind":"gap","topic":"t","detail":"d","iteration":1,' +
                    f'"confidence":{lexeme},'.encode("ascii") +
                    b'"created_at_unix":2}\n')
        exact = "0e+" + "0" * 125
        view = json.loads(project_request(make_request(
            BINDING, [(MEMORY_KIND, "memory", memory(exact))])))
        self.assertEqual(exact, view["candidates"][0]["confidence"]["raw_number_lexeme"])
        with self.assertRaises(ContractError):
            project_request(make_request(
                BINDING, [(MEMORY_KIND, "memory", memory(exact + "0"))]))

    def test_confidence_extreme_exponents_have_exact_cross_runtime_range(self) -> None:
        def project(lexeme: str) -> dict[str, object]:
            raw = (b'{"kind":"gap","topic":"t","detail":"d","iteration":1,' +
                   f'"confidence":{lexeme},'.encode("ascii") +
                   b'"created_at_unix":2}\n')
            return json.loads(project_request(make_request(
                BINDING, [(MEMORY_KIND, "memory", raw)])))
        positive = ("0e999999999999999999999", "1e-999999999999999999999",
                    "0.01e-9223372036854775808")
        for lexeme in positive:
            candidate = project(lexeme)["candidates"][0]
            self.assertEqual(lexeme, candidate["confidence"]["raw_number_lexeme"])
        for lexeme in ("-1e-999999999999999999999", "1e999999999999999999999",
                       "1e9223372036854775807", "NaN", "1."):
            with self.assertRaises(ContractError):
                project(lexeme)

    def test_huge_memory_integer_is_closed_error_and_checker_emits_nothing(self) -> None:
        huge = b"1" * 5000
        memory = (b'{"kind":"gap","topic":"t","detail":"d","iteration":' + huge +
                  b',"created_at_unix":2}\n')
        request = make_request(BINDING, [(MEMORY_KIND, "memory", memory)])
        with self.assertRaises(ContractError):
            project_request(request)
        checker = str(ROOT / "harness" / "legacy_governance_read_import_contract_check.py")
        rejected = subprocess.run([sys.executable, checker], input=request,
                                  capture_output=True, check=False)
        self.assertEqual(2, rejected.returncode)
        self.assertEqual(b"", rejected.stdout)
        self.assertNotIn(b"Traceback", rejected.stderr)


class ViewAdversarialTests(unittest.TestCase):
    def test_view_rejects_tamper_and_trailing_bytes(self) -> None:
        golden = GOLDEN.read_bytes()[:-1]
        tampered = golden.replace(b'"current":false', b'"current":true', 1)
        with self.assertRaises(ContractError):
            decode_view(tampered)
        with self.assertRaises(ContractError):
            decode_view(golden + b"\n")

    def test_view_must_match_the_exact_request(self) -> None:
        other = make_request(dict(BINDING, source_revision="fixture-revision-0002"),
                             fixture_sources())
        with self.assertRaises(ContractError):
            validate_view_against_request(GOLDEN.read_bytes()[:-1], other)

    def test_standalone_view_rejects_self_consistent_illegal_ref(self) -> None:
        view = json.loads(GOLDEN.read_bytes())
        candidate = view["candidates"][4]
        illegal = candidate["source_ref"] + "x" * MAX_SOURCE_REF_BYTES
        candidate["source_ref"] = illegal
        candidate["document_name"] = illegal
        view["sources"][1]["source_ref"] = illegal
        reseal_candidate(candidate)
        with self.assertRaises(ContractError):
            decode_view(reseal_view(view))

    def test_standalone_view_rejects_self_consistent_adr_n_plus_one(self) -> None:
        view = json.loads(GOLDEN.read_bytes())
        candidate = view["candidates"][4]
        raw = b"x" * MAX_ADR_BYTES + b"\n"
        candidate["raw_byte_count"] = len(raw)
        candidate["raw_bytes_base64url"] = encode_base64url(raw)
        candidate["raw_sha256"] = sha256_bytes(raw)
        view["sources"][1]["byte_count"] = len(raw)
        view["sources"][1]["content_sha256"] = sha256_bytes(raw)
        reseal_candidate(candidate)
        with self.assertRaises(ContractError):
            decode_view(reseal_view(view))

    def test_standalone_view_bad_source_kinds_are_closed_contract_errors(self) -> None:
        for location, bad_kind in (("candidate", []), ("descriptor", {})):
            view = json.loads(GOLDEN.read_bytes())
            if location == "candidate":
                view["candidates"][0]["source_kind"] = bad_kind
                reseal_candidate(view["candidates"][0])
            else:
                view["sources"][0]["source_kind"] = bad_kind
            with self.assertRaises(ContractError):
                decode_view(reseal_view(view))

    def test_standalone_view_memory_candidate_count_n_and_n_plus_one(self) -> None:
        self.assertEqual(MAX_MEMORY_ENTRIES,
                         len(decode_view(sealed_memory_only_view(MAX_MEMORY_ENTRIES))
                             ["candidates"]))
        with self.assertRaises(ContractError):
            decode_view(sealed_memory_only_view(MAX_MEMORY_ENTRIES + 1))

    def test_contract_core_has_no_runtime_or_io_dependency(self) -> None:
        package = ROOT / "harness" / "legacy_governance_read_import_contract"
        source = b"\n".join(path.read_bytes() for path in sorted(package.glob("*.py")))
        for forbidden in (b"memory.Load", b"from memory", b"import memory", b"time.",
                          b"sqlite", b"open(", b"read_bytes", b"write_bytes"):
            self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
