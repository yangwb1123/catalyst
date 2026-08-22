#!/usr/bin/env python3
"""Bounded adapter for the Catalyst project-snapshot product command."""

import sys

if not sys.flags.isolated:
    sys.stderr.write(
        "project-snapshot capture rejected: isolated Python startup required; "
        "invoke with python3 -I -B\n"
    )
    raise SystemExit(1)

import hashlib
import importlib
import importlib.util
import os
import re
import selectors
import signal
import stat
import subprocess
import time
from pathlib import Path


_CONTRACT_MODULE = "_forgeos_project_source_snapshot_contract"


def _load_contract() -> tuple[type[Exception], object, int]:
    package = (Path(__file__).resolve().parent / "_vendor" /
               "project_source_snapshot_contract")
    specification = importlib.util.spec_from_file_location(
        _CONTRACT_MODULE, package / "__init__.py",
        submodule_search_locations=[str(package)],
    )
    if specification is None or specification.loader is None:
        raise RuntimeError("vendored contract loader is unavailable")
    module = importlib.util.module_from_spec(specification)
    sys.modules[_CONTRACT_MODULE] = module
    try:
        specification.loader.exec_module(module)
        constants = importlib.import_module(f"{_CONTRACT_MODULE}.constants")
        return module.ContractError, module.decode_production, constants.MAX_ENVELOPE_BYTES
    except BaseException:
        for name in tuple(sys.modules):
            if name == _CONTRACT_MODULE or name.startswith(f"{_CONTRACT_MODULE}."):
                sys.modules.pop(name, None)
        raise


try:
    ContractError, decode_production, MAX_ENVELOPE_BYTES = _load_contract()
except BaseException:
    sys.stderr.write(
        "project-snapshot capture rejected: anchored vendored contract unavailable\n"
    )
    raise SystemExit(1)

MAX_STDOUT = MAX_ENVELOPE_BYTES + 1
MAX_STDERR = 32 * 1024 * 1024
TIMEOUT_SECONDS = 125
USAGE = "capture.py --forge FILE --root DIR --project-id ID --run-id ID"


class AdapterError(ValueError):
    """A stable adapter rejection."""


def _clean_absolute(value: str, label: str) -> str:
    if (not value.startswith("/") or value.startswith("//") or
            value != os.path.normpath(value) or
            any(ord(character) < 32 or ord(character) == 127 for character in value)):
        raise AdapterError(f"{label} must be a clean absolute path")
    return value


def parse_args(argv: list[str]) -> dict[str, str]:
    if len(argv) != 8:
        raise AdapterError("expected exactly four options")
    values: dict[str, str] = {}
    allowed = {"--forge", "--root", "--project-id", "--run-id"}
    for index in range(0, len(argv), 2):
        option, value = argv[index], argv[index + 1]
        if option not in allowed or option in values or not value or value.startswith("-"):
            raise AdapterError("invalid or duplicate option")
        values[option] = value
    if set(values) != allowed:
        raise AdapterError("required option is absent")
    for option in ("--project-id", "--run-id"):
        value = values[option]
        if len(value) > 160 or re.fullmatch(r"[a-z0-9][a-z0-9._:/-]*", value) is None:
            raise AdapterError(f"{option} is not a canonical identifier")
    return values


def _identity(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def _open_runtime(path: str) -> tuple[int, os.stat_result]:
    if sys.platform != "linux" or not os.path.isdir("/proc/self/fd"):
        raise AdapterError("live capture requires Linux /proc descriptor execution")
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NONBLOCK | os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    before_path, before_fd = os.lstat(path), os.fstat(descriptor)
    if (_identity(before_path) != _identity(before_fd) or
            not stat.S_ISREG(before_fd.st_mode) or before_fd.st_nlink != 1):
        os.close(descriptor)
        raise AdapterError("runtime is not a stable single-link regular file")
    return descriptor, before_fd


def _kill_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def _read_process(process: subprocess.Popen[bytes]) -> tuple[bytes, bytes, int]:
    selector = selectors.DefaultSelector()
    streams = {process.stdout: bytearray(), process.stderr: bytearray()}
    for stream in streams:
        if stream is None:
            raise AdapterError("runtime pipe setup failed")
        os.set_blocking(stream.fileno(), False)
        selector.register(stream, selectors.EVENT_READ)
    deadline = time.monotonic() + TIMEOUT_SECONDS
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise AdapterError("runtime execution timed out")
            events = selector.select(min(remaining, 0.25))
            for key, _ in events:
                chunk = os.read(key.fd, 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                target = streams[key.fileobj]
                limit = MAX_STDOUT if key.fileobj is process.stdout else MAX_STDERR
                if len(target) + len(chunk) > limit:
                    raise AdapterError("runtime stream exceeds its bound")
                target.extend(chunk)
            if process.poll() is not None and not events:
                continue
    except (AdapterError, OSError):
        _kill_group(process)
        for stream in streams:
            if stream is not None and not stream.closed:
                stream.close()
        process.wait(timeout=5)
        raise
    finally:
        selector.close()
    try:
        return_code = process.wait(timeout=max(0.01, deadline - time.monotonic()))
    except subprocess.TimeoutExpired as error:
        _kill_group(process)
        process.wait(timeout=5)
        raise AdapterError("runtime execution timed out") from error
    return bytes(streams[process.stdout]), bytes(streams[process.stderr]), return_code


def _validate_output(raw: bytes, values: dict[str, str]) -> bytes:
    if (not raw or len(raw) > MAX_STDOUT or not raw.endswith(b"\n") or
            raw.endswith(b"\n\n")):
        raise AdapterError("runtime output framing is invalid")
    try:
        production = decode_production(raw[:-1])
    except ContractError as error:
        raise AdapterError(f"runtime output violates the strict contract: {error}") from error
    expected = (values["--project-id"], values["--run-id"])
    for node_name in ("request", "snapshot"):
        node = production[node_name]
        if (node["project_id"], node["run_id"]) != expected:
            raise AdapterError("runtime output does not bind the requested project and run")
    return raw


def run_capture(values: dict[str, str]) -> bytes:
    runtime, before = _open_runtime(values["--forge"])
    executable = f"/proc/self/fd/{runtime}"
    command = [executable, "project-snapshot", "capture",
               "--project-id", values["--project-id"], "--run-id", values["--run-id"],
               "--root", values["--root"]]
    environment = {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"}
    try:
        process = subprocess.Popen(
            command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, env=environment, pass_fds=(runtime,),
            start_new_session=True,
        )
        output, error_output, return_code = _read_process(process)
        after_fd, after_path = os.fstat(runtime), os.lstat(values["--forge"])
        if _identity(before) != _identity(after_fd) or _identity(after_fd) != _identity(after_path):
            raise AdapterError("runtime changed during execution")
    except (OSError, subprocess.SubprocessError) as error:
        raise AdapterError("runtime execution failed") from error
    finally:
        os.close(runtime)
    if return_code != 0:
        stderr_digest = hashlib.sha256(error_output).hexdigest()
        raise AdapterError(
            f"runtime rejected capture with exit {return_code}; "
            f"stderr_bytes={len(error_output)}; stderr_sha256={stderr_digest}"
        )
    return _validate_output(output, values)


def main(argv: list[str]) -> int:
    try:
        values = parse_args(argv)
    except AdapterError as error:
        print(f"{USAGE}: {error}", file=sys.stderr)
        return 2
    if sys.platform != "linux" or not os.path.isdir("/proc/self/fd"):
        print("project-snapshot: not_executed: compatible forge runtime is unavailable", file=sys.stderr)
        return 3
    try:
        values["--forge"] = _clean_absolute(values["--forge"], "runtime")
        values["--root"] = _clean_absolute(values["--root"], "root")
    except AdapterError as error:
        print(f"{USAGE}: {error}", file=sys.stderr)
        return 2
    if not os.path.lexists(values["--forge"]):
        print("project-snapshot: not_executed: compatible forge runtime is unavailable", file=sys.stderr)
        return 3
    try:
        output = run_capture(values)
    except (OSError, AdapterError) as error:
        print(f"project-snapshot capture rejected: {error}", file=sys.stderr)
        return 1
    written = sys.stdout.buffer.write(output)
    if written != len(output):
        print("project-snapshot capture rejected: short stdout write", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
