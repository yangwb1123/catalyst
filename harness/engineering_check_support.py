#!/usr/bin/env python3
"""Shared strict parsing and reference helpers for engineering contracts."""
# Registry v39 physical-source governance imports begin.
import hashlib
import stat
# Registry v39 physical-source governance imports end.
import os
from pathlib import Path

import yaml


MAX_SPEC_BYTES = 512 * 1024


class _UniqueKeyLoader(yaml.SafeLoader):
    """Safe YAML loader that rejects duplicate keys instead of overwriting."""


class _StrictKeyLoader(_UniqueKeyLoader):
    """Unique-key loader that also rejects anchors/aliases during composition,
    so the check costs one parse instead of a full token scan + a full load."""

    def compose_node(self, parent, index):
        event = self.peek_event()
        if (isinstance(event, yaml.events.AliasEvent)
                or getattr(event, "anchor", None) is not None):
            raise yaml.composer.ComposerError(
                None, None, "YAML anchors and aliases are not allowed", None)
        return super().compose_node(parent, index)


def _construct_unique_mapping(loader, node, deep=False):
    mapping = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        try:
            duplicate = key in mapping
        except TypeError as exc:
            raise yaml.constructor.ConstructorError(
                None, None, "unhashable mapping key", key_node.start_mark,
            ) from exc
        if duplicate:
            raise yaml.constructor.ConstructorError(
                None, None, f"duplicate key {key!r}", key_node.start_mark,
            )
        mapping[key] = loader.construct_object(value_node, deep=deep)
    return mapping


_UniqueKeyLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
    _construct_unique_mapping,
)


def read_bounded_spec(path):
    try:
        raw = read_regular_file(
            path, str(path), MAX_SPEC_BYTES,
            (0o600, 0o640, 0o644, 0o660, 0o664),
        )
    except MemoryError as error:
        raise ValueError("bounded spec read exhausted memory") from error
    except ValueError as error:
        if str(error).endswith(f" exceeds {MAX_SPEC_BYTES} bytes"):
            raise ValueError(f"file exceeds {MAX_SPEC_BYTES} bytes") from error
        raise
    if len(raw) > MAX_SPEC_BYTES:
        raise ValueError(f"file exceeds {MAX_SPEC_BYTES} bytes")
    return raw


def _load_yaml_once(path, raw):
    text = raw.decode("utf-8")
    return yaml.load(text, Loader=_StrictKeyLoader), None


# Parse-level cache: the same spec files are re-read by every governance check
# (dozens of times per `forge accept`, once per extension contract). The key is
# the exact bounded bytes digest, so forged/restored timestamps and path races
# cannot replay another document; callers receive deep copies.
_YAML_PARSE_CACHE = {}
_YAML_PARSE_CACHE_MAX = 128


def load_yaml(path):
    try:
        raw = read_bounded_spec(path)
        key = hashlib.sha256(raw).digest()
        if key in _YAML_PARSE_CACHE:
            data, error = _YAML_PARSE_CACHE[key]
            if error is None:
                import copy
                return copy.deepcopy(data), None
            return data, error
        result = _load_yaml_once(path, raw)
        if len(_YAML_PARSE_CACHE) >= _YAML_PARSE_CACHE_MAX:
            _YAML_PARSE_CACHE.pop(next(iter(_YAML_PARSE_CACHE)))
        _YAML_PARSE_CACHE[key] = result
        data, error = result
        if error is None:
            import copy
            return copy.deepcopy(data), None
        return data, error
    except (OSError, UnicodeDecodeError, ValueError, RecursionError, yaml.YAMLError) as exc:
        return None, str(exc).replace("\n", " ")


def mapping_issues(data, path, kind):
    if isinstance(data, dict):
        return []
    return [f"{path}: {kind} must be a YAML mapping"]


def unknown_field_issues(data, allowed, label):
    if not isinstance(data, dict):
        return []
    unknown = set(data) - set(allowed)
    return [f"{label}: unknown fields {sorted(unknown, key=lambda item: str(item))}"] if unknown else []


def header_issues(data, path, kind):
    issues = []
    if data.get("api_version") != "forgeos.agent-engineering/v1":
        issues.append(f"{path}: unsupported api_version")
    if data.get("kind") != kind or data.get("status") != "active_contract":
        issues.append(f"{path}: expected kind {kind!r} with active_contract status")
    return issues


def unique_id_issues(items, path, kind):
    issues, seen = [], set()
    for index, item in enumerate(items if isinstance(items, list) else []):
        item_id = item.get("id") if isinstance(item, dict) else None
        if not isinstance(item_id, str) or not item_id.strip():
            issues.append(f"{path}: {kind}[{index}] requires a non-empty id")
        elif item_id in seen:
            issues.append(f"{path}: duplicate {kind} id {item_id!r}")
        else:
            seen.add(item_id)
    return issues, seen


def repo_path_issue(repo_root, raw, label, require_exists=True):
    if not isinstance(raw, str) or not raw.strip():
        return f"{label}: expected a non-empty repository-relative path"
    rel = Path(raw)
    if rel.is_absolute() or ".." in rel.parts:
        return f"{label}: unsafe repository path {raw!r}"
    target = repo_root / rel
    if require_exists and not target.exists():
        return f"{label}: referenced path does not exist: {raw!r}"
    if require_exists:
        try:
            target.resolve(strict=True).relative_to(repo_root.resolve(strict=True))
        except (OSError, ValueError):
            return f"{label}: path escapes repository through a symlink: {raw!r}"
    return None


# Registry v39 physical-source governance helpers begin.
SOURCE_SCAN_EXCLUDED_PATHS = frozenset({
    ".git", ".hg", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".svn",
    ".tox", ".venv", ".forge", "__pycache__", "build", "coverage", "dist",
    "node_modules", "target", "venv", "forge-runtime/target",
})
SOURCE_SCAN_MODES = (0o600, 0o640, 0o644, 0o664, 0o755)
SOURCE_SCAN_SUFFIXES = frozenset({
    ".cjs", ".cts", ".go", ".js", ".jsx", ".mjs", ".mts", ".py", ".rs",
    ".ts", ".tsx",
})


def _identity(info):
    return info.st_dev, info.st_ino


def _file_stamp(info):
    return (_identity(info), info.st_mode, info.st_nlink, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


def _directory_flags():
    nofollow, directory = getattr(os, "O_NOFOLLOW", None), getattr(os, "O_DIRECTORY", None)
    if nofollow is None or directory is None:
        raise OSError("descriptor-safe directory traversal is unavailable")
    return (os.O_RDONLY | directory | nofollow | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NONBLOCK", 0))


def _open_parent_chain(path, label):
    target = Path(path)
    parts = target.parts[1:] if target.is_absolute() else target.parts
    if not parts or any(part in ("", ".", "..") for part in parts):
        raise OSError(f"unsafe {label} path")
    descriptors = [os.open("/" if target.is_absolute() else ".", _directory_flags())]
    links = []
    try:
        for part in parts[:-1]:
            lexical = os.stat(part, dir_fd=descriptors[-1], follow_symlinks=False)
            child = os.open(part, _directory_flags(), dir_fd=descriptors[-1])
            opened = os.fstat(child)
            if not stat.S_ISDIR(opened.st_mode) or _identity(lexical) != _identity(opened):
                os.close(child)
                raise OSError(f"{label} parent component changed")
            links.append((descriptors[-1], part, opened))
            descriptors.append(child)
        return descriptors, links, parts[-1]
    except BaseException:
        for descriptor in reversed(descriptors):
            os.close(descriptor)
        raise


def _verify_parent_chain(links, label):
    for parent, name, opened in links:
        lexical = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not stat.S_ISDIR(lexical.st_mode) or _identity(lexical) != _identity(opened):
            raise OSError(f"{label} parent component changed while reading")


def _read_regular_at(parent, name, label, maximum, modes, expected=None, after_open=None):
    nofollow = getattr(os, "O_NOFOLLOW", None)
    if nofollow is None:
        raise OSError("O_NOFOLLOW is unavailable")
    lexical = os.stat(name, dir_fd=parent, follow_symlinks=False)
    descriptor = os.open(
        name, os.O_RDONLY | nofollow | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NONBLOCK", 0), dir_fd=parent)
    try:
        opened = os.fstat(descriptor)
        if expected is not None and _identity(expected) != _identity(opened):
            raise OSError(f"{label} changed before opening")
        if (_identity(lexical) != _identity(opened) or not stat.S_ISREG(opened.st_mode)
                or stat.S_IMODE(opened.st_mode) not in modes or opened.st_nlink != 1):
            allowed = "/".join(f"{mode:04o}" for mode in modes)
            raise OSError(f"{label} must be regular {allowed} with link count one")
        if opened.st_size > maximum:
            raise ValueError(f"{label} exceeds {maximum} bytes")
        if after_open is not None:
            after_open()
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(maximum + 1)
        current = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if _file_stamp(opened) != _file_stamp(os.fstat(descriptor)) \
                or _file_stamp(opened) != _file_stamp(current):
            raise OSError(f"{label} changed while reading")
        if len(raw) > maximum:
            raise ValueError(f"{label} exceeds {maximum} bytes")
        return raw
    finally:
        os.close(descriptor)


def read_regular_file(
        path, label, maximum=128 * 1024 * 1024, modes=(0o644,), after_open=None):
    descriptors, links, name = _open_parent_chain(path, label)
    try:
        raw = _read_regular_at(
            descriptors[-1], name, label, maximum, modes, after_open=after_open)
        _verify_parent_chain(links, label)
        return raw
    finally:
        for descriptor in reversed(descriptors):
            os.close(descriptor)


def aggregate_manifest(rows):
    manifest = "".join(
        f"{digest}  {relative}\n" for relative, digest in rows).encode()
    return hashlib.sha256(manifest).hexdigest()


def real_directory(repo_root, relative, label):
    current = repo_root
    for part in relative.split("/"):
        current /= part
        if not stat.S_ISDIR(current.lstat().st_mode):
            raise OSError(f"{label} component {current} must be a real directory")
    return current


def physical_manifest_issues(repo_root, paths, expected_aggregate, label):
    issues, rows = [], []
    for relative, expected in paths.items():
        try:
            raw = read_regular_file(repo_root / relative, relative)
        except (OSError, ValueError) as error:
            issues.append(f"{relative}: {label} unreadable: {error}")
            continue
        digest = hashlib.sha256(raw).hexdigest()
        rows.append((relative, digest))
        if digest != expected:
            issues.append(f"{relative}: physical pin drifted")
    if len(rows) != len(paths) or aggregate_manifest(rows) != expected_aggregate:
        issues.append(f"{label} exact{len(paths)} aggregate drifted")
    return issues


def optional_package_manifest_issues(
        repo_root, package, paths, aggregate, label, repository_flavor,
        catalyst_flavor, scaffold_flavor):
    root = repo_root / package
    try:
        root.lstat()
    except FileNotFoundError as error:
        if repository_flavor == catalyst_flavor:
            return [f"{package}: required {label} unavailable: {error}"]
        return []
    except OSError as error:
        return [f"{package}: {label} unreadable: {error}"]
    if repository_flavor == scaffold_flavor:
        return [f"{package}: scaffold cannot contain Catalyst-only {label}"]
    try:
        root = real_directory(repo_root, package, label)
        expected_root = {name.split("/", 1)[0] for name in paths}
        if {entry.name for entry in root.iterdir()} != expected_root:
            return [f"{package}: {label} exact lexical closure drifted"]
        nested = {name.split("/", 1)[1] for name in paths if "/" in name}
        tests = real_directory(repo_root, f"{package}/tests", label) if nested else None
        if nested and {entry.name for entry in tests.iterdir()} != nested:
            return [f"{package}: {label} exact nested closure drifted"]
    except OSError as error:
        return [f"{package}: {label} unreadable: {error}"]
    mapped = {f"{package}/{name}": digest for name, digest in paths.items()}
    return physical_manifest_issues(repo_root, mapped, aggregate, label)


def proposed_adr_issues(
        repo_root, relative, physical, body, self_digest, markers, validator):
    try:
        raw = read_regular_file(repo_root / relative, relative, 256 * 1024)
        metadata = validator(raw, relative.rsplit("/", 1)[-1])
        normalized = " ".join(raw.decode().split())
    except (OSError, UnicodeDecodeError, ValueError) as error:
        return [f"{relative}: Proposed ADR failed: {error}"]
    expected = {
        "status": "proposed", "acceptance_id": None,
        "accepted_at_unix_ms": None, "body_sha256": body,
        "self_sha256": self_digest,
    }
    issues = []
    if hashlib.sha256(raw).hexdigest() != physical:
        issues.append(f"{relative}: physical pin drifted")
    issues.extend(f"{relative}: {field} drifted" for field, value in expected.items()
                  if metadata.get(field) != value)
    issues.extend(f"{relative}: boundary marker {marker!r} missing" for marker in
                  markers if marker not in normalized)
    return issues


def _directory_stamp(info):
    return (_identity(info), info.st_mode, info.st_nlink, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


def _scan_child_directory(parent, entry, relative, records, issues, label):
    descriptor = None
    try:
        lexical = entry.stat(follow_symlinks=False)
        descriptor = os.open(entry.name, _directory_flags(), dir_fd=parent)
        opened = os.fstat(descriptor)
        if not stat.S_ISDIR(opened.st_mode) or _identity(lexical) != _identity(opened):
            raise OSError("directory changed before opening")
        _scan_directory(descriptor, relative, records, issues, label)
        current = os.stat(entry.name, dir_fd=parent, follow_symlinks=False)
        if _identity(current) != _identity(opened):
            raise OSError("directory changed while scanning")
    except OSError as error:
        issues.append(f"{relative}: {label} source directory unreadable: {error}")
    finally:
        if descriptor is not None:
            os.close(descriptor)


def _scan_directory(descriptor, prefix, records, issues, label):
    opened = os.fstat(descriptor)
    try:
        with os.scandir(descriptor) as iterator:
            entries = sorted(iterator, key=lambda entry: entry.name)
    except OSError as error:
        issues.append(f"{prefix or '.'}: {label} source directory unreadable: {error}")
        return
    for entry in entries:
        relative = f"{prefix}/{entry.name}" if prefix else entry.name
        if relative in SOURCE_SCAN_EXCLUDED_PATHS:
            continue
        try:
            info = entry.stat(follow_symlinks=False)
        except OSError as error:
            issues.append(f"{relative}: {label} source path unreadable: {error}")
            continue
        if stat.S_ISLNK(info.st_mode):
            issues.append(f"{relative}: {label} source path must be real")
        elif stat.S_ISDIR(info.st_mode):
            _scan_child_directory(descriptor, entry, relative, records, issues, label)
        elif Path(entry.name).suffix.lower() in SOURCE_SCAN_SUFFIXES:
            try:
                raw = _read_regular_at(
                    descriptor, entry.name, relative, 1024 * 1024,
                    SOURCE_SCAN_MODES, expected=info)
                records.append((relative, raw))
            except (OSError, ValueError) as error:
                issues.append(f"{relative}: {label} reference scan unreadable: {error}")
    if _directory_stamp(opened) != _directory_stamp(os.fstat(descriptor)):
        issues.append(f"{prefix or '.'}: {label} source directory changed while scanning")


def _repository_source_records(repo_root, issues, label):
    descriptors = links = None
    try:
        descriptors, links, _ = _open_parent_chain(
            Path(repo_root) / ".forge-source-scan-anchor", label)
        records = []
        _scan_directory(descriptors[-1], "", records, issues, label)
        _verify_parent_chain(links, label)
        return records
    except OSError as error:
        issues.append(f"{label}: source tree unreadable: {error}")
        return []
    finally:
        for descriptor in reversed(descriptors or []):
            os.close(descriptor)


def source_reference_closure_issues(repo_root, tokens, allowed_counts, label):
    issues, seen = [], set()
    needles = tuple(token.lower().encode("utf-8") for token in tokens)
    allowed = {relative: tuple(counts)
               for relative, counts in allowed_counts.items()}
    for relative, raw in _repository_source_records(repo_root, issues, label):
        expected = allowed.get(relative)
        if expected is not None:
            seen.add(relative)
        raw = raw.lower()
        observed = tuple(raw.count(needle) for needle in needles)
        if expected is not None:
            if observed != expected:
                issues.append(
                    f"{relative}: unauthorized {label} allowed reference count drifted")
        elif expected is None and any(observed):
            issues.append(f"{relative}: unauthorized {label} runtime consumer")
    issues.extend(
        f"{relative}: required {label} allowed reference path missing"
        for relative in sorted(set(allowed) - seen))
    return issues
# Registry v39 physical-source governance helpers end.
