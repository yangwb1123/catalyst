#!/usr/bin/env python3
"""Run the repository Python coverage probe with a fixed isolated import plan."""

from __future__ import annotations

import locale
import os
import re
import signal
import subprocess
import sys
import tempfile
import threading
from collections import defaultdict
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import BinaryIO, NamedTuple


PYTEST_ARGUMENTS = (
    "--import-mode=importlib",
    "-p",
    "no:cacheprovider",
    "--cov",
    "--cov-report=json:coverage.json",
    "-q",
)
PYTEST_BOOTSTRAP = (
    "import sys; sys.path[:0] = sys.argv[1:3]; import pytest; "
    "raise SystemExit(pytest.main(sys.argv[3:]))"
)
MAX_ITEMS_PER_SHARD = 16
SUBPROCESS_TIMEOUT_SECONDS = 30 * 60
SUBPROCESS_STREAM_LIMIT_BYTES = 1024 * 1024
TERMINATION_GRACE_SECONDS = 5
SHARDED_FILES = frozenset({
    "harness/test_agent_engineering_check.py",
    "harness/test_governance_engineering_integration.py",
})


class _ProcessResult(NamedTuple):
    returncode: int
    stdout: str
    stderr: str
    diagnostic: str


class _Capture:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.stdout = bytearray()
        self.stderr = bytearray()
        self.failure = ""
        self.lock = threading.Lock()

    def reject(self, reason: str) -> bool:
        with self.lock:
            if self.failure:
                return False
            self.failure = reason
            return True


def _terminate_process_tree(process: subprocess.Popen[bytes]) -> None:
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    elif os.name == "nt":
        try:
            subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                timeout=TERMINATION_GRACE_SECONDS,
            )
        except (OSError, subprocess.TimeoutExpired):
            pass
    if process.poll() is None:
        try:
            process.kill()
        except ProcessLookupError:
            pass


def _remember_tail(output: bytearray, chunk: bytes, limit: int) -> bool:
    exceeded = len(output) + len(chunk) > limit
    if len(chunk) >= limit:
        output[:] = chunk[-limit:]
    else:
        excess = max(0, len(output) + len(chunk) - limit)
        if excess:
            del output[:excess]
        output.extend(chunk)
    return exceeded


def _drain_stream(
    stream: BinaryIO, name: str, capture: _Capture,
    process: subprocess.Popen[bytes],
) -> None:
    output = getattr(capture, name)
    read_chunk = getattr(stream, "read1", stream.read)
    try:
        while chunk := read_chunk(64 * 1024):
            if _remember_tail(output, chunk, capture.limit) and capture.reject(
                f"python coverage subprocess {name} exceeded "
                f"{capture.limit} bytes"
            ):
                _terminate_process_tree(process)
    except (OSError, ValueError) as error:
        if capture.reject(f"python coverage subprocess {name} read failed: {error}"):
            _terminate_process_tree(process)


def _popen_options() -> dict[str, object]:
    if os.name == "posix":
        return {"start_new_session": True}
    if os.name == "nt":
        return {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
    return {}


def _bounded_subprocess(
    command: Sequence[str], *, cwd: Path, environment: dict[str, str] | None = None,
    timeout: float = SUBPROCESS_TIMEOUT_SECONDS,
    stream_limit: int = SUBPROCESS_STREAM_LIMIT_BYTES,
) -> _ProcessResult:
    if timeout <= 0 or stream_limit <= 0:
        raise ValueError("subprocess limits must be positive")
    capture = _Capture(stream_limit)
    process = subprocess.Popen(
        command, cwd=cwd, env=environment, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, **_popen_options(),
    )
    assert process.stdout is not None and process.stderr is not None
    threads = [
        threading.Thread(
            target=_drain_stream, args=(stream, name, capture, process), daemon=True,
        )
        for stream, name in ((process.stdout, "stdout"), (process.stderr, "stderr"))
    ]
    for thread in threads:
        thread.start()
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        capture.reject(f"python coverage subprocess timed out after {timeout:g}s")
        _terminate_process_tree(process)
        process.wait(timeout=TERMINATION_GRACE_SECONDS)
    for thread in threads:
        thread.join(timeout=TERMINATION_GRACE_SECONDS)
    if any(thread.is_alive() for thread in threads):
        capture.reject("python coverage subprocess pipe drain timed out")
        _terminate_process_tree(process)
        for stream in (process.stdout, process.stderr):
            stream.close()
        for thread in threads:
            thread.join(timeout=TERMINATION_GRACE_SECONDS)
    for stream in (process.stdout, process.stderr):
        stream.close()
    encoding = locale.getpreferredencoding(False)
    stdout = bytes(capture.stdout).decode(encoding, errors="replace")
    stderr = bytes(capture.stderr).decode(encoding, errors="replace")
    returncode = 1 if capture.failure else int(process.returncode or 0)
    return _ProcessResult(returncode, stdout, stderr, capture.failure)


def _process_output(completed: _ProcessResult) -> str:
    return "\n".join(
        part for part in (completed.stdout, completed.stderr, completed.diagnostic) if part
    )


def _repository_paths(script_path: Path) -> tuple[Path, Path]:
    script = script_path.resolve(strict=True)
    adapters = script.parent
    harness = adapters.parent
    root = harness.parent
    if adapters.name != "adapters" or harness.name != "harness":
        raise ValueError("coverage runner must remain at harness/adapters")
    return root, harness


def _validated_arguments(arguments: Sequence[str]) -> tuple[str, ...]:
    supplied = tuple(arguments)
    if supplied != PYTEST_ARGUMENTS:
        raise ValueError("coverage runner arguments do not match the fixed profile")
    return supplied


def _pytest_command(root: Path, harness: Path, arguments: Sequence[str]) -> list[str]:
    return [
        sys.executable, "-I", "-B", "-c", PYTEST_BOOTSTRAP,
        str(harness), str(root), *arguments,
    ]


def _collect_nodes(
    arguments: Sequence[str], root: Path, harness: Path,
) -> list[str]:
    profile = [
        argument for argument in arguments
        if argument != "--cov" and not argument.startswith("--cov-report=")
    ]
    command = _pytest_command(
        root, harness, [*profile, "--collect-only", "--rootdir", str(root)],
    )
    completed = _bounded_subprocess(command, cwd=root)
    if completed.returncode != 0:
        sys.stderr.write(_process_output(completed)[-8192:])
        return []
    nodes = []
    for raw in completed.stdout.splitlines():
        line = raw.strip()
        if "::" not in line or not line.split("::", 1)[0].endswith(".py"):
            continue
        if re.match(r"^.+\.py::.+$", line):
            nodes.append(line)
    summary = re.search(r"(\d+)\s+tests?\s+collected", completed.stdout)
    if not summary or int(summary.group(1)) != len(nodes) or len(set(nodes)) != len(nodes):
        sys.stderr.write(
            f"python coverage rejected: collection count mismatch ({len(nodes)} nodes)\n",
        )
        return []
    return nodes


def _shards(nodes: Sequence[str]) -> list[list[str]]:
    by_file: dict[str, list[str]] = defaultdict(list)
    for node in nodes:
        by_file[node.split("::", 1)[0]].append(node)
    shards = []
    for file, file_nodes in by_file.items():
        if file not in SHARDED_FILES:
            shards.append(file_nodes)
            continue
        by_class: dict[str, list[str]] = defaultdict(list)
        for node in file_nodes:
            parts = node.split("::")
            by_class["::".join(parts[:2]) if len(parts) > 2 else node].append(node)
        for class_nodes in by_class.values():
            for start in range(0, len(class_nodes), MAX_ITEMS_PER_SHARD):
                shards.append(class_nodes[start:start + MAX_ITEMS_PER_SHARD])
    return shards


def _worker_arguments(arguments: Sequence[str], targets: Sequence[str], root: Path) -> list[str]:
    report = "--cov-report=json:coverage.json"
    profile = [argument if argument != report else "--cov-report=" for argument in arguments]
    return [*profile, "--rootdir", str(root), *targets]


def _run_worker(
    arguments: Sequence[str], targets: Sequence[str], root: Path, harness: Path,
    data_dir: Path, index: int,
) -> tuple[int, str]:
    environment = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}
    environment["COVERAGE_FILE"] = str(data_dir / f".coverage.{index}")
    command = _pytest_command(
        root, harness, _worker_arguments(arguments, targets, root),
    )
    completed = _bounded_subprocess(command, cwd=root, environment=environment)
    return completed.returncode, _process_output(completed)


def _worker_count(total: int) -> int:
    raw = os.environ.get("FORGE_COVERAGE_WORKERS", "8")
    try:
        requested = int(raw)
    except ValueError:
        requested = 8
    return max(1, min(total, requested, os.cpu_count() or 1))


def _combine_coverage(root: Path, data_dir: Path) -> bool:
    try:
        import coverage

        combined = coverage.Coverage(data_file=str(root / ".coverage"), auto_data=False)
        combined.combine(data_paths=[str(data_dir)], strict=True)
        combined.save()
        combined.json_report(outfile=str(root / "coverage.json"))
        return True
    except Exception as error:  # coverage must fail closed, never fake a report
        sys.stderr.write(f"python coverage combine failed: {error}\n")
        return False


def _worker_data_parent(root: Path) -> Path:
    system_temp = Path(tempfile.gettempdir()).resolve()
    if system_temp == root or root in system_temp.parents:
        return root.parent
    return system_temp


def _run_parallel(arguments: Sequence[str], root: Path, harness: Path) -> int:
    from concurrent.futures import ThreadPoolExecutor

    shards = _shards(_collect_nodes(arguments, root, harness))
    if not shards:
        sys.stderr.write("python coverage rejected: pytest collected no test nodes\n")
        return 1
    with tempfile.TemporaryDirectory(
        prefix=".forge-coverage-workers-", dir=_worker_data_parent(root),
    ) as raw:
        data_dir = Path(raw)
        with ThreadPoolExecutor(max_workers=_worker_count(len(shards))) as pool:
            futures = [
                pool.submit(_run_worker, arguments, targets, root, harness, data_dir, index)
                for index, targets in enumerate(shards)
            ]
            results = [future.result() for future in futures]
        failures = [output for code, output in results if code != 0]
        if failures:
            for output in failures[:4]:
                sys.stderr.write(output[-4096:])
            return 1
        return 0 if _combine_coverage(root, data_dir) else 1


def run(
    arguments: Sequence[str],
    *,
    flags: object = sys.flags,
    script_path: Path = Path(__file__),
    pytest_main: Callable[[list[str]], int] | None = None,
) -> int:
    if not getattr(flags, "isolated", False) or not getattr(
        flags, "dont_write_bytecode", False
    ):
        sys.stderr.write("python coverage rejected: isolated no-bytecode Python is required\n")
        return 2
    try:
        pytest_arguments = _validated_arguments(arguments)
        root, harness = _repository_paths(script_path)
    except (OSError, ValueError) as error:
        sys.stderr.write(f"python coverage rejected: {error}\n")
        return 2
    sys.path[:0] = [str(harness), str(root)]
    if pytest_main is not None:
        return int(pytest_main(list(pytest_arguments)))
    try:
        import pytest  # noqa: F401
        import pytest_cov  # noqa: F401
    except ModuleNotFoundError as error:
        if error.name not in {"pytest", "pytest_cov"}:
            raise
        sys.stderr.write(f"python coverage unavailable: No module named {error.name}\n")
        return 3
    return _run_parallel(pytest_arguments, root, harness)


if __name__ == "__main__":
    raise SystemExit(run(sys.argv[1:]))
