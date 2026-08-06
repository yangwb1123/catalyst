"""Bounded-text and bounded-process primitives for the ForgeOS review
runner. Ported from the ai-batch-runner toolset (ai/run-review.py +
pbatch/) with its pbatch dependency removed: every helper here is pure
stdlib and enforces hard caps so untrusted agent output and oversized
inputs cannot OOM the runner or escape the deadline.
"""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

INPUT_MAX_BYTES = 2 * 1024 * 1024
OUTPUT_MAX_BYTES = 64 * 1024
PROMPT_MAX_BYTES = 122880
VALIDATION_OUTPUT_MAX_BYTES = 256 * 1024


def read_text_bounded(path: Path, maximum: int, label: str = "file") -> str:
    """Read UTF-8 text without allocating an unbounded input buffer.

    The size check is repeated after opening to cover files that grow
    between stat and read. ValueError is used for policy rejection so
    callers fail without saving partial work.
    """
    if maximum < 1:
        raise ValueError(f"{label} limit must be positive")
    path = Path(path)
    try:
        if path.stat().st_size > maximum:
            raise ValueError(f"{label} exceeds {maximum} bytes: {path}")
        with path.open("rb") as handle:
            raw = handle.read(maximum + 1)
    except ValueError:
        raise
    except OSError as exc:
        raise ValueError(f"unable to read {label} {path}: {exc}") from exc
    if len(raw) > maximum:
        raise ValueError(f"{label} exceeds {maximum} bytes: {path}")
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ValueError(f"{label} is not valid UTF-8: {path}") from exc


def read_stream(stream, collector: list, cap: int = 0,
                overflow: Optional[list] = None, emit: bool = False,
                target=None) -> None:
    """Drain and collect a child stream, optionally displaying it live.
    Collection is capped at cap bytes; the stream is still drained (a
    blocked pipe would hang the agent) while collection and live display
    stop at the cap. Overflow is recorded for the caller to reject."""
    total = 0
    sink = target or sys.stdout
    try:
        for line in iter(lambda: stream.readline(8192), ""):
            within_cap = True
            if cap > 0:
                total += len(line.encode("utf-8", errors="replace"))
                within_cap = total <= cap
            if within_cap:
                collector.append(line)
                if emit:
                    sink.write(line)
                    sink.flush()
            elif overflow is not None:
                overflow[0] = True
    except ValueError:
        pass
    finally:
        stream.close()


def kill_group(proc: Optional[subprocess.Popen]) -> None:
    """Kill the whole child process group, tolerating an already-dead
    process, then reap the direct child before returning."""
    if proc is None:
        return
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        try:
            proc.kill()
        except Exception:
            pass
    try:
        proc.wait(timeout=1)
    except (subprocess.TimeoutExpired, ChildProcessError):
        pass


def drain_process(proc: subprocess.Popen, start: float, timeout: float,
                  output_cap: int):
    """Drain stdout/stderr under the deadline; raises TimeoutExpired when
    the deadline hits so the caller can kill the group."""
    out_lines: list = []
    err_lines: list = []
    out_over = [False]
    err_over = [False]
    out_reader = threading_reader(proc.stdout, out_lines, output_cap, out_over)
    err_reader = threading_reader(proc.stderr, err_lines, output_cap, err_over)
    out_reader.start()
    err_reader.start()
    deadline = time.monotonic() + timeout
    try:
        proc.wait(timeout=max(0.1, deadline - time.monotonic()))
    except subprocess.TimeoutExpired:
        raise
    out_reader.join(timeout=2)
    err_reader.join(timeout=2)
    return out_lines, err_lines, out_over, err_over


def threading_reader(stream, collector: list, cap: int, overflow: list):
    import threading
    return threading.Thread(
        target=read_stream, args=(stream, collector, cap, overflow),
        daemon=True,
    )


@dataclass
class ValidationResult:
    ok: bool
    exit_code: int = -1
    stdout: str = ""
    stderr: str = ""
    timed_out: bool = False


def run_validation(cmd: str, cwd: str, timeout: float = 600) -> ValidationResult:
    """Run one validator command in its own process group with a hard
    deadline and bounded diagnostics. A spawn crash fails closed."""
    if not isinstance(cmd, str):
        raise TypeError("validator command must be a string")
    try:
        proc = subprocess.Popen(cmd, shell=True, cwd=cwd,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                text=True, start_new_session=True)
    except Exception as exc:
        return ValidationResult(ok=False, exit_code=-1,
                                stdout="", stderr=str(exc), timed_out=False)
    start = time.monotonic()
    try:
        out, err, out_over, err_over = drain_process(
            proc, start, timeout, VALIDATION_OUTPUT_MAX_BYTES)
    except subprocess.TimeoutExpired:
        kill_group(proc)
        return ValidationResult(ok=False, exit_code=-1,
                                stdout="", stderr="", timed_out=True)
    stdout, stderr = "".join(out), "".join(err)
    if out_over[0] or err_over[0]:
        return ValidationResult(ok=False, exit_code=-1,
                                stdout=stdout, stderr=stderr, timed_out=False)
    rc = proc.returncode
    return ValidationResult(ok=rc == 0, exit_code=rc,
                            stdout=stdout, stderr=stderr, timed_out=False)


def revalidate_existing(path: Path, validate_cmd: str, workdir: str = "") -> bool:
    """Re-run the effective validators against an already-written artifact
    (the reuse path). Missing, empty, or symlinked artifacts fail closed;
    no validators apply -> trivially reusable."""
    if not path.is_file():
        return False
    if path.is_symlink():
        return False
    try:
        if path.stat().st_size == 0:
            return False
    except OSError:
        return False
    commands = [x.strip() for x in (validate_cmd or "").split(",") if x.strip()]
    if not commands:
        return True
    for raw in commands:
        import shlex
        cmd = raw.replace("{output}", shlex.quote(str(path)))
        cmd = cmd.replace("{cwd}", shlex.quote(workdir or str(path.parent)))
        result = run_validation(cmd, workdir or str(path.parent))
        if not result.ok:
            return False
    return True
