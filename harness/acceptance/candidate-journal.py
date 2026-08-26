#!/usr/bin/env python3
"""Bounded raw-inotify helper for ordinary changes on trusted local filesystems.

Memory-mapped writes, privileged mount changes, and remote filesystems are not
inotify proof surfaces; the JavaScript owner rejects non-allowlisted roots.
"""
import ctypes
import json
import os
import selectors
import stat
import struct
import sys

IN_MODIFY = 0x00000002
IN_ATTRIB = 0x00000004
IN_CLOSE_WRITE = 0x00000008  # conservative: writable open/close may false-reject
IN_MOVED_FROM = 0x00000040
IN_MOVED_TO = 0x00000080
IN_CREATE = 0x00000100
IN_DELETE = 0x00000200
IN_DELETE_SELF = 0x00000400
IN_MOVE_SELF = 0x00000800
IN_UNMOUNT = 0x00002000
IN_Q_OVERFLOW = 0x00004000
IN_IGNORED = 0x00008000
IN_ONLYDIR = 0x01000000
IN_ISDIR = 0x40000000
FILE_MASK = (IN_MODIFY | IN_ATTRIB | IN_CLOSE_WRITE | IN_DELETE_SELF
             | IN_MOVE_SELF | IN_UNMOUNT | IN_IGNORED)
DIR_MASK = (FILE_MASK | IN_MOVED_FROM | IN_MOVED_TO | IN_CREATE | IN_DELETE
            | IN_ONLYDIR)
MAX_COMMAND = 4096
MAX_EVENTS = 32
MAX_EVENT_READ = 256 * 1024
EVENT_HEADER = struct.Struct("iIII")
LIBC = ctypes.CDLL(None, use_errno=True)
LIBC.inotify_init1.argtypes = [ctypes.c_int]
LIBC.inotify_init1.restype = ctypes.c_int
LIBC.inotify_add_watch.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_uint32]
LIBC.inotify_add_watch.restype = ctypes.c_int


def canonical(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def write_ready(ok, error=None):
    record = {"ok": ok, "error": error}
    data = (canonical(record) + "\n").encode("utf-8")
    os.write(3, data)
    os.close(3)


def same_identity(first, second):
    return (first.st_dev, first.st_ino, stat.S_IFMT(first.st_mode)) == (
        second.st_dev, second.st_ino, stat.S_IFMT(second.st_mode))


def safe_component(name):
    if not name or name in (".", "..") or "/" in name or "\x00" in name:
        raise RuntimeError("candidate journal encountered an unsafe path component")
    try:
        name.encode("utf-8", "strict")
    except UnicodeError as error:
        raise RuntimeError("candidate journal requires UTF-8 paths") from error


class Journal:
    def __init__(self, root, excluded, excluded_prefixes):
        descriptor = LIBC.inotify_init1(os.O_NONBLOCK | os.O_CLOEXEC)
        if descriptor < 0:
            code = ctypes.get_errno()
            raise OSError(code, os.strerror(code))
        self.fd = descriptor
        self.root = root
        self.root_device = None
        self.excluded = excluded
        self.excluded_prefixes = excluded_prefixes
        self.watches = {}
        self.dirty = False
        self.overflow = False
        self.error = None
        self.events = []

    def excluded_name(self, name):
        return name in self.excluded or any(
            name.startswith(prefix) for prefix in self.excluded_prefixes)

    def add_watch(self, descriptor, kind, relative, directory):
        target = f"/proc/self/fd/{descriptor}".encode("ascii")
        watch = LIBC.inotify_add_watch(self.fd, target, DIR_MASK if directory else FILE_MASK)
        if watch < 0:
            code = ctypes.get_errno()
            raise OSError(code, os.strerror(code), relative or ".")
        mapping = (kind, relative)
        if watch in self.watches and self.watches[watch] != mapping:
            raise RuntimeError("candidate journal encountered duplicate watch coverage")
        self.watches[watch] = mapping

    def open_directory_component(self, parent, name):
        safe_component(name)
        before = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not stat.S_ISDIR(before.st_mode):
            raise RuntimeError("candidate journal ancestor is not a directory")
        flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
        descriptor = os.open(name, flags, dir_fd=parent)
        opened = os.fstat(descriptor)
        after = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not same_identity(before, opened) or not same_identity(opened, after):
            os.close(descriptor)
            raise RuntimeError("candidate journal ancestor changed during watch setup")
        return descriptor

    def open_root(self):
        current = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            for name in self.root.split(os.sep)[1:]:
                self.add_watch(current, "anchor", name, True)
                child = self.open_directory_component(current, name)
                os.close(current)
                current = child
            return current
        except Exception:
            os.close(current)
            raise

    def open_verified(self, parent, name, before, directory):
        flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
        if directory:
            flags |= os.O_DIRECTORY
        else:
            flags |= os.O_NONBLOCK
        descriptor = os.open(name, flags, dir_fd=parent)
        opened = os.fstat(descriptor)
        after = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not same_identity(before, opened) or not same_identity(opened, after):
            os.close(descriptor)
            raise RuntimeError("candidate journal path changed during watch setup")
        if opened.st_dev != self.root_device:
            os.close(descriptor)
            raise RuntimeError("candidate journal does not cross filesystem mounts")
        return descriptor

    def watch_tree(self, descriptor, relative=""):
        if relative == "":
            self.add_watch(descriptor, "dir", relative, True)
        names = sorted(os.listdir(descriptor))
        for name in names:
            safe_component(name)
            if self.excluded_name(name):
                continue
            child = name if relative == "" else f"{relative}/{name}"
            before = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
            if stat.S_ISDIR(before.st_mode):
                opened = self.open_verified(descriptor, name, before, True)
                try:
                    self.add_watch(opened, "dir", child, True)
                    self.watch_tree(opened, child)
                finally:
                    os.close(opened)
            elif stat.S_ISREG(before.st_mode):
                opened = self.open_verified(descriptor, name, before, False)
                try:
                    self.add_watch(opened, "file", child, False)
                finally:
                    os.close(opened)
            else:
                raise RuntimeError(f"candidate journal path is unsupported: {child}")

    def setup(self):
        root = self.open_root()
        try:
            opened = os.fstat(root)
            self.root_device = opened.st_dev
            self.watch_tree(root)
        finally:
            os.close(root)
        self.drain_events()
        if self.dirty:
            raise RuntimeError(self.error or "candidate changed while journal was arming")

    def event_path(self, watch, raw_name):
        mapping = self.watches.get(watch)
        if mapping is None:
            raise RuntimeError("candidate journal received an unknown watch descriptor")
        kind, relative = mapping
        if raw_name:
            try:
                name = raw_name.decode("utf-8", "strict")
            except UnicodeError as error:
                raise RuntimeError("candidate journal event path is not UTF-8") from error
            safe_component(name)
            if kind == "anchor":
                return "." if name == relative else None
            if self.excluded_name(name):
                return None
            return name if relative == "" else f"{relative}/{name}"
        return "." if kind == "anchor" or relative == "" else relative

    def record(self, watch, mask, raw_name):
        if mask & IN_Q_OVERFLOW:
            self.dirty = True
            self.overflow = True
            self.error = "candidate journal inotify queue overflowed"
            return
        try:
            path = self.event_path(watch, raw_name)
        except Exception as error:
            self.dirty = True
            self.error = str(error)
            return
        if path is None:
            return
        self.dirty = True
        if len(self.events) < MAX_EVENTS and path not in self.events:
            self.events.append(path)
        if mask & (IN_UNMOUNT | IN_IGNORED):
            self.error = "candidate journal watch coverage was lost"

    def parse_events(self, data):
        offset = 0
        while offset < len(data):
            if len(data) - offset < EVENT_HEADER.size:
                raise RuntimeError("candidate journal received a truncated inotify event")
            watch, mask, _cookie, length = EVENT_HEADER.unpack_from(data, offset)
            end = offset + EVENT_HEADER.size + length
            if end > len(data):
                raise RuntimeError("candidate journal received invalid inotify framing")
            name = data[offset + EVENT_HEADER.size:end].split(b"\0", 1)[0]
            self.record(watch, mask & ~IN_ISDIR, name)
            offset = end

    def drain_events(self):
        while True:
            try:
                data = os.read(self.fd, MAX_EVENT_READ)
            except BlockingIOError:
                return
            except OSError as error:
                self.dirty = True
                self.error = f"candidate journal inotify read failed: {error}"
                return
            if not data:
                self.dirty = True
                self.error = "candidate journal inotify descriptor closed"
                return
            try:
                self.parse_events(data)
            except Exception as error:
                self.dirty = True
                self.error = str(error)

    def response(self, identifier):
        return {
            "id": identifier, "op": "BARRIER", "ok": self.error is None,
            "dirty": self.dirty, "overflow": self.overflow,
            "events": list(self.events), "error": self.error,
        }

    def close(self):
        try:
            os.close(self.fd)
        except OSError:
            pass


def parse_command(line):
    if len(line) > MAX_COMMAND:
        raise RuntimeError("candidate journal command exceeded limit")
    try:
        text = line.decode("utf-8", "strict")
        value = json.loads(text)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise RuntimeError("candidate journal command is invalid JSON") from error
    if canonical(value) != text or list(value) != ["id", "op"]:
        raise RuntimeError("candidate journal command is not canonical")
    if (isinstance(value["id"], bool) or not isinstance(value["id"], int)
            or value["id"] < 1 or value["id"] > 9_007_199_254_740_991):
        raise RuntimeError("candidate journal command id is invalid")
    if value["op"] not in ("BARRIER", "CLOSE"):
        raise RuntimeError("candidate journal command operation is invalid")
    return value


def write_response(value):
    data = (canonical(value) + "\n").encode("utf-8")
    if len(data) > 256 * 1024:
        raise RuntimeError("candidate journal response exceeded limit")
    sys.stdout.buffer.write(data)
    sys.stdout.buffer.flush()


def process_commands(journal, buffer):
    while b"\n" in buffer:
        line, buffer = buffer.split(b"\n", 1)
        command = parse_command(line)
        journal.drain_events()
        if command["op"] == "CLOSE":
            return buffer, False
        write_response(journal.response(command["id"]))
    if len(buffer) > MAX_COMMAND:
        raise RuntimeError("candidate journal command buffer exceeded limit")
    return buffer, True


def serve(journal):
    selector = selectors.DefaultSelector()
    selector.register(journal.fd, selectors.EVENT_READ, "events")
    selector.register(0, selectors.EVENT_READ, "commands")
    buffer = b""
    try:
        while True:
            ready = selector.select()
            if any(key.data == "events" for key, _mask in ready):
                journal.drain_events()
            for key, _mask in ready:
                if key.data != "commands":
                    continue
                chunk = os.read(0, MAX_COMMAND)
                if not chunk:
                    return
                buffer, keep_running = process_commands(journal, buffer + chunk)
                if not keep_running:
                    return
    finally:
        selector.close()


def load_arguments():
    if len(sys.argv) != 4 or not os.path.isabs(sys.argv[1]):
        raise RuntimeError("candidate journal helper arguments are invalid")
    excluded = json.loads(sys.argv[2])
    prefixes = json.loads(sys.argv[3])
    if (not isinstance(excluded, list) or not excluded
            or any(not isinstance(item, str) or "/" in item for item in excluded)):
        raise RuntimeError("candidate journal exclusions are invalid")
    if (not isinstance(prefixes, list) or not prefixes
            or any(not isinstance(item, str) or not item or "/" in item
                   for item in prefixes)):
        raise RuntimeError("candidate journal exclusion prefixes are invalid")
    return os.path.realpath(sys.argv[1]), frozenset(excluded), tuple(prefixes)


def main():
    journal = None
    try:
        root, excluded, prefixes = load_arguments()
        journal = Journal(root, excluded, prefixes)
        journal.setup()
        write_ready(True)
    except Exception as error:
        try:
            write_ready(False, str(error)[:4096])
        except Exception:
            pass
        if journal is not None:
            journal.close()
        return 2
    try:
        serve(journal)
        return 0
    except Exception as error:
        os.write(2, f"candidate journal helper fatal: {error}\n".encode("utf-8")[:4096])
        return 3
    finally:
        journal.close()


if __name__ == "__main__":
    raise SystemExit(main())
