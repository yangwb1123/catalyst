"""Adversarial tests for explicit Platform Core contract file reads."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from .codec import ContractError
from .constants import MAX_INPUT_PATH_BYTES, MAX_INPUT_PATH_COMPONENTS
from .contract import read_bounded_file

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "harness/platform_core_contract/check.py"


class PlatformCoreFileReadTests(unittest.TestCase):
    """Descriptor-relative explicit input reads fail closed under file races."""

    def test_special_leaf_is_rejected_before_any_leaf_open(self) -> None:
        opened: list[str] = []
        original_open = os.open

        def observed_open(path, flags, *args, **kwargs):
            opened.append(os.fspath(path))
            return original_open(path, flags, *args, **kwargs)

        with mock.patch("platform_core_contract.contract.os.open", side_effect=observed_open):
            with self.assertRaises(ContractError):
                read_bounded_file(Path("/dev/null"))
        self.assertNotIn("null", opened)

    def test_path_budgets_reject_before_opening_any_descriptor(self) -> None:
        paths = (
            Path("x" * (MAX_INPUT_PATH_BYTES + 1)),
            Path(*(["x"] * MAX_INPUT_PATH_COMPONENTS), "input"),
        )
        for path in paths:
            with self.subTest(length=len(os.fspath(path))), \
                    mock.patch("platform_core_contract.contract.os.open") as opened, \
                    self.assertRaises(ContractError):
                read_bounded_file(path)
            opened.assert_not_called()

    def test_directory_fstat_failures_close_new_descriptors(self) -> None:
        directory_state = os.stat(".")
        cases = (
            ([101], [OSError("root fstat")], [mock.call(101)]),
            ([101, 102], [directory_state, OSError("child fstat")],
             [mock.call(102), mock.call(101)]),
        )
        for descriptors, states, expected_closes in cases:
            with self.subTest(descriptors=descriptors), \
                    mock.patch("platform_core_contract.contract.os.open",
                               side_effect=descriptors), \
                    mock.patch("platform_core_contract.contract.os.fstat",
                               side_effect=states), \
                    mock.patch("platform_core_contract.contract.os.close") as close, \
                    self.assertRaises(ContractError):
                read_bounded_file(Path("parent/input"))
            self.assertEqual(close.call_args_list, expected_closes)

    def test_nonregular_links_and_symlinked_golden_root_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            source = target / "input"
            source.write_bytes(b"safe")
            rejected = []
            if hasattr(os, "mkfifo"):
                fifo = root / "fifo"
                os.mkfifo(fifo)
                rejected.append(fifo)
            leaf_link = root / "leaf_link"
            leaf_link.symlink_to(source)
            rejected.append(leaf_link)
            ancestor_link = root / "ancestor_link"
            ancestor_link.symlink_to(target, target_is_directory=True)
            rejected.append(ancestor_link / "input")
            hardlink = root / "hardlink"
            os.link(source, hardlink)
            rejected.extend((source, hardlink))
            loop_a, loop_b = root / "loop_a", root / "loop_b"
            loop_a.symlink_to(loop_b, target_is_directory=True)
            loop_b.symlink_to(loop_a, target_is_directory=True)
            rejected.append(loop_a / "input")
            for path in rejected:
                with self.subTest(path=path), self.assertRaises(ContractError):
                    read_bounded_file(path, 128 * 1024)
            repo_link = root / "repo_link"
            repo_link.symlink_to(ROOT, target_is_directory=True)
            result = subprocess.run(
                [sys.executable, "-I", "-B", str(CHECKER), "--golden", str(repo_link)],
                cwd=ROOT, text=True, capture_output=True, check=False)
            self.assertEqual(result.returncode, 2)

    def test_rewrite_growth_leaf_and_ancestor_replacement_are_rejected(self) -> None:
        for attack in ("rewrite", "grow", "leaf", "ancestor"):
            with self.subTest(attack=attack), tempfile.TemporaryDirectory() as directory:
                root, active = Path(directory), Path(directory) / "active"
                active.mkdir()
                path = active / "input"
                path.write_bytes(b"a" * 70_000)
                original_read, attacked = os.read, False

                def racing_read(descriptor: int, size: int) -> bytes:
                    nonlocal attacked
                    chunk = original_read(descriptor, size)
                    if not attacked:
                        attacked = True
                        if attack == "rewrite":
                            path.write_bytes(b"b" * 70_000)
                        elif attack == "grow":
                            with path.open("ab") as stream:
                                stream.write(b"b")
                        elif attack == "leaf":
                            path.rename(active / "old")
                            path.write_bytes(b"b" * 70_000)
                        else:
                            active.rename(root / "old")
                            active.mkdir()
                            path.write_bytes(b"b" * 70_000)
                    return chunk

                with mock.patch("platform_core_contract.contract.os.read",
                                side_effect=racing_read):
                    with self.assertRaises(ContractError):
                        read_bounded_file(path, 128 * 1024)
