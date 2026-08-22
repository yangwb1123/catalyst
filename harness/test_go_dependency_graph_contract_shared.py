"""Shared malformed corpus for neutral ADR-0053 graph consumers."""

from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path

HARNESS = Path(__file__).resolve().parent
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

from governance_contract import ContractError  # noqa: E402
from go_package_dependency_graph_observation_producer import (  # noqa: E402
    canonical_json,
)
from go_package_dependency_graph_observation_producer.constants import (  # noqa: E402
    FIXTURE_PATH,
)
from go_package_dependency_graph_observation_producer.graph_contract import (  # noqa: E402
    validate_graph_bytes as validate_neutral_graph_bytes,
)
from local_go_package_impact_prescan_contract.graph import (  # noqa: E402
    validate_graph_bytes as validate_adr0062_graph_bytes,
)


class SharedGraphContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        wrapper = json.loads((ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        cls.graph = wrapper["production"]["graph_observation"]
        cls.raw = canonical_json(cls.graph)

    @staticmethod
    def outcome(validator, raw: bytes) -> tuple[str, object]:
        try:
            return "accepted", validator(raw)
        except ContractError as error:
            return "rejected", str(error)

    def mutated(self, change) -> bytes:
        graph = copy.deepcopy(self.graph)
        change(graph)
        return canonical_json(graph)

    def malformed_corpus(self) -> dict[str, bytes]:
        duplicate = self.raw.replace(
            b"{", b'{"api_version":"duplicate",', 1)
        noncanonical = json.dumps(
            self.graph, ensure_ascii=False, sort_keys=True, indent=2).encode("utf-8")
        return {
            "duplicate": duplicate,
            "unknown": self.mutated(lambda graph: graph.update(unknown=None)),
            "noncanonical": noncanonical,
            "order": self.mutated(lambda graph: graph["files"].reverse()),
            "profile": self.mutated(lambda graph: graph.update(profile_id="other")),
            "coverage drift": self.mutated(
                lambda graph: graph["coverage"].update(
                    regular_go_files_selected=
                    graph["coverage"]["regular_go_files_selected"] + 1)),
            "package drift": self.mutated(
                lambda graph: graph["packages"][0].update(name="drifted")),
            "dependency drift": self.mutated(
                lambda graph: graph["dependencies"][0].update(relation="contains")),
        }

    def test_neutral_validator_accepts_the_frozen_graph(self):
        self.assertEqual(validate_neutral_graph_bytes(self.raw), self.graph)
        self.assertEqual(validate_adr0062_graph_bytes(self.raw), self.graph)

    def test_neutral_and_adr0062_wrapper_reject_identically(self):
        for label, raw in self.malformed_corpus().items():
            with self.subTest(label=label):
                direct = self.outcome(validate_neutral_graph_bytes, raw)
                wrapped = self.outcome(validate_adr0062_graph_bytes, raw)
                self.assertEqual(direct, wrapped)
                self.assertEqual(direct[0], "rejected")


if __name__ == "__main__":
    unittest.main()
