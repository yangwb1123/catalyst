#!/usr/bin/env python3
"""Exercise a standalone release binary offline; Python is only the test driver."""

import argparse
import hashlib
import http.server
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading

README_CONTENT = "Standalone binary smoke fixture.\n"
NOTE_BEFORE = "broken\n"


class SmokeFailure(Exception):
    """A required observation was absent or a subprocess failed."""


def require(condition, message):
    if not condition:
        raise SmokeFailure(message)


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def frames(events):
    return "".join("event: " + event["type"] + "\ndata: "
                   + json.dumps(event) + "\n\n" for event in events).encode()


def tool_stream(sequence, name, arguments):
    item = {"id": f"fc-{sequence}", "type": "function_call",
            "call_id": f"call-{sequence}", "name": name,
            "arguments": json.dumps(arguments), "status": "completed",
            "caller": {"type": "direct"}}
    return frames([
        {"type": "response.output_item.added", "item": item},
        {"type": "response.function_call_arguments.done", "item_id": item["id"],
         "name": name, "arguments": item["arguments"]},
        {"type": "response.completed",
         "response": {"status": "completed", "output": [item], "usage": None}},
    ])


def final_stream(answer):
    item = {"id": "msg-final", "type": "message", "role": "assistant",
            "phase": "final_answer", "status": "completed",
            "content": [{"type": "output_text", "text": answer, "annotations": []}]}
    return frames([
        {"type": "response.output_item.added", "item": item},
        {"type": "response.output_text.delta", "item_id": item["id"], "delta": answer},
        {"type": "response.completed",
         "response": {"status": "completed", "output": [item], "usage": None}},
    ])


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers["Content-Length"])
        self.server.requests.append(json.loads(self.rfile.read(length)))
        if self.path != "/v1/responses" or not self.server.responses:
            self.send_error(500)
            return
        body = self.server.responses.pop(0)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


class LocalResponses(http.server.HTTPServer):
    def __init__(self, responses):
        super().__init__(("127.0.0.1", 0), Handler)
        self.responses = list(responses)
        self.requests = []
        self.thread = threading.Thread(target=self.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_args):
        self.shutdown()
        self.server_close()
        self.thread.join(timeout=5)

    @property
    def env(self):
        return {"OPENAI_BASE_URL": f"http://127.0.0.1:{self.server_port}/v1",
                "OPENAI_API_KEY": "dummy-offline-key"}

    def completed(self, count):
        require(len(self.requests) == count and not self.responses,
                f"expected {count} local Responses requests; got {len(self.requests)}")


class Smoke:
    def __init__(self, binary, report):
        self.binary = binary
        self.report = report
        self.checks = []
        self.transcript = []
        self.version = None
        self.binary_digest = None

    def record(self, name):
        self.checks.append(name)
        print("PASS:", name, flush=True)

    def prepare(self, temp):
        self.temp = temp
        self.installed = temp / "bin" / self.binary.name
        self.installed.parent.mkdir()
        shutil.copy2(self.binary, self.installed)
        self.binary_digest = digest(self.installed)
        require(digest(self.binary) == self.binary_digest, "binary changed during copying")
        self.workspace = temp / "workspace"
        self.workspace.mkdir()
        (self.workspace / "README.md").write_text(README_CONTENT, encoding="utf-8")
        (self.workspace / "note.txt").write_text(NOTE_BEFORE, encoding="utf-8")
        self.state = temp / "state"

    def run(self, arguments, env=None, success=True):
        arguments = list(map(str, arguments))
        try:
            process = subprocess.run(
                [str(self.installed), *arguments], cwd=self.temp,
                env={"PATH": "", "LANG": "C.UTF-8", **(env or {})},
                capture_output=True, text=True, encoding="utf-8", errors="replace",
                timeout=30,
            )
        except subprocess.TimeoutExpired as error:
            self.transcript.append({"args": arguments, "error": "timeout after 30s"})
            raise SmokeFailure(f"command timed out: {arguments!r}") from error
        self.transcript.append({"args": arguments, "exit_code": process.returncode,
                                "stdout": process.stdout, "stderr": process.stderr})
        require((process.returncode == 0) == success,
                f"unexpected exit {process.returncode}: {arguments!r}; "
                f"stderr={process.stderr[:2000]!r}; stdout={process.stdout[:1000]!r}")
        return process

    def query(self, arguments):
        return json.loads(self.run(["--state-dir", self.state, "--json", *arguments]).stdout)

    def check_version(self):
        state = self.temp / "version-state"
        environment = {"FORGE_RUNTIME_HOME": str(state)}
        human = self.run(["--version"], environment)
        machine = self.run(["--json", "--version"], environment)
        version = json.loads(machine.stdout)
        require(isinstance(version, dict) and set(version) == {"name", "version"},
                "JSON version must contain exactly name and version")
        require(version["name"] == "forge-runtime", "unexpected product name")
        require(isinstance(version["version"], str) and version["version"].strip(),
                "JSON version must contain a nonempty version string")
        require(human.stdout.strip() == f"{version['name']} {version['version']}",
                "human and JSON version outputs disagree")
        require(not human.stderr and not machine.stderr, "version wrote to stderr")
        require(not state.exists() and not self.state.exists(), "version created Hub state")
        self.version = version
        self.record("human and JSON version agree without creating Hub state")

    def check_hub(self):
        require("agent" in self.run(["--help"]).stdout, "help omits agent")
        self.record("help outside checkout with an empty PATH")
        session = self.query(["-C", self.workspace, "session", "new", "--title", "binary smoke"])
        self.session_id = session["session"]["id"]
        prompt = self.query(["prompt", "add", self.session_id, "inspect README.md"])
        self.prompt_id = prompt["prompt"]["id"]
        self.record("SQLite session and prompt creation across separate processes")

    def check_deterministic_run(self):
        arguments = ["--state-dir", self.state, "--idempotency-key", "binary-smoke-run",
                     "-C", self.workspace, "run", "start", self.session_id, self.prompt_id,
                     "--read", "README.md"]
        events = [json.loads(line) for line in self.run(arguments).stdout.splitlines()]
        require(events and events[0]["type"] == "run_started", "missing run_started")
        require(events[-1]["type"] == "run_finished", "missing run_finished")
        require(events[-1]["outcome"]["status"] == "completed", "run did not complete")
        kinds = [event["type"] for event in events]
        require(kinds.index("tool_started") < kinds.index("tool_finished"), "tool event order")
        verify_tool_events(events, {"read_file": README_CONTENT})
        run_id = events[0]["run_id"]
        self.record("offline deterministic run executes workspace read and finishes")
        self.check_durable_run(run_id, events)
        require(self.run(arguments).stdout == "", "idempotent replay emitted new execution")
        replay = self.query(["run", "show", run_id])["inspection"]
        require(replay["events"] == events, "idempotent replay changed durable events")
        self.record("idempotent replay emits no new execution")

    def check_durable_run(self, run_id, events):
        inspection = self.query(["run", "show", run_id])["inspection"]
        require(inspection["recovery"]["status"] == "terminal", "Run is not terminal")
        require(inspection["events"] == events, "persisted events differ from output")
        verify_tool_events(inspection["events"], {"read_file": README_CONTENT})
        explanation = self.query(["run", "explain", run_id])["explanation"]
        require(explanation["continuation"]["safe"] is True, "unsafe terminal continuation")
        listing = self.query(["run", "list", self.session_id, "--limit", "5"])
        require(listing["runs"][0]["run_id"] == run_id, "Run listing lost completed Run")
        self.record("durable run query, explanation and listing")

    def check_missing_key(self):
        state = self.temp / "missing-key-state"
        process = self.run(["--state-dir", state, "-C", self.workspace, "agent", "inspect"],
                           success=False)
        require("OPENAI_API_KEY" in process.stderr, "missing credential diagnostic")
        require(not (state / "hub.sqlite3").exists(), "missing key created database")
        self.record("missing provider key fails before database creation")

    def check_readonly(self):
        responses = [tool_stream(1, "read_file", {"path": "README.md"}),
                     final_stream("read-only inspection complete")]
        with LocalResponses(responses) as server:
            process = self.run(["--state-dir", self.state, "-C", self.workspace, "agent",
                                "--model", "offline-test-model", "inspect README.md"], server.env)
            require("read-only inspection complete" in process.stdout, "missing Agent answer")
            require("[run] completed" in process.stdout, "read-only Agent did not complete")
            server.completed(2)
            names = {tool["name"] for tool in server.requests[0]["tools"]}
            require({"list_files", "read_file", "search_text"} <= names, "missing read tools")
            require(not {"edit_file", "exec_command"} & names, "read-only exposed dev tools")
            verify_provider_output(server.requests, "call-1", README_CONTENT)
        inspection = self.agent_inspection(process)
        verify_tool_events(inspection["events"], {"read_file": README_CONTENT})
        self.record("Agent read-only tools and completion through a local Responses fixture")

    def check_dev(self):
        with LocalResponses(dev_responses()) as server:
            process = self.run(["--state-dir", self.state, "-C", self.workspace, "agent", "--dev",
                                "--model", "offline-test-model", "repair note.txt and verify it"],
                               server.env)
            require((self.workspace / "note.txt").read_text() == "fixed\n", "edit was not applied")
            for tool in ("read_file", "edit_file", "exec_command"):
                require(f"[tool] {tool} started" in process.stdout, f"{tool} did not start")
                require(f"[tool] {tool} finished" in process.stdout, f"{tool} did not finish")
            require("fixed and verified" in process.stdout, "missing dev answer")
            require("[run] completed" in process.stdout, "dev Agent did not complete")
            require("not an OS sandbox" in process.stderr, "missing dev trust disclosure")
            server.completed(4)
            verify_dev_requests(server.requests)
            verify_provider_output(server.requests, "call-1", NOTE_BEFORE)
        self.dev_run_id = process.stdout.splitlines()[0].split()[1]
        verify_tool_events(self.agent_inspection(process)["events"],
                           {"read_file": NOTE_BEFORE, "edit_file": None, "exec_command": None})
        self.record("Agent --dev read/edit/execute/finish with empty PATH and local fixture")

    def agent_inspection(self, process):
        run_id = process.stdout.splitlines()[0].split()[1]
        inspection = self.query(["run", "show", run_id])["inspection"]
        require(inspection["recovery"]["status"] == "terminal", "Agent Run was not persisted")
        return inspection

    def check_final_hub(self):
        sessions = self.query(["-C", self.workspace, "session", "list"])
        require("sessions" in sessions, "Hub query did not return sessions")
        persisted = self.query(["run", "show", self.dev_run_id])["inspection"]
        require(persisted["recovery"]["status"] == "terminal", "dev Run was not persisted")
        require(persisted["recovery"]["outcome"]["status"] == "completed", "dev Run failed")
        require(any(event["type"] == "tool_finished" for event in persisted["events"]),
                "dev tool events were not persisted")
        require((self.state / "hub.sqlite3").is_file(), "Hub database is missing")
        self.record("Hub remains queryable after both provider fixtures shut down")

    def execute(self):
        with tempfile.TemporaryDirectory(prefix="forge-binary-smoke-") as directory:
            self.prepare(Path(directory))
            self.check_version()
            self.check_hub()
            self.check_deterministic_run()
            self.check_missing_key()
            self.check_readonly()
            self.check_dev()
            self.check_final_hub()

    def write_report(self, status, error=None):
        report = {"status": status, "binary": self.binary.name,
                  "binary_sha256": self.binary_digest, "version": self.version,
                  "checks": self.checks,
                  "provider": "loopback Responses fixtures; no real model request",
                  "environment": "binary copied alone outside checkout; empty PATH; temporary state/workspace",
                  "transcript": self.transcript}
        if error is not None:
            report["error"] = error
        self.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
                               encoding="utf-8")


def dev_responses():
    return [
        tool_stream(1, "read_file", {"path": "note.txt"}),
        tool_stream(2, "edit_file", {"path": "note.txt", "old_text": "broken\n",
                                    "new_text": "fixed\n"}),
        tool_stream(3, "exec_command", {"program": "/bin/sh",
                    "argv": ["-c", 'IFS= read -r line < note.txt; test "$line" = fixed'],
                    "timeout_ms": 5000}),
        final_stream("fixed and verified"),
    ]


def verify_dev_requests(requests):
    names = {tool["name"] for tool in requests[0]["tools"]}
    require({"edit_file", "exec_command"} <= names, "missing dev tools")
    outputs = [item for request in requests for item in request["input"]
               if item.get("type") == "function_call_output" and item["call_id"] == "call-3"]
    require(outputs, "exec_command result was not returned to the provider")
    require("exit_code: 0" in outputs[0]["output"].splitlines(),
            f"verification command failed: {outputs[0]['output']!r}")


def verify_provider_output(requests, call_id, expected):
    outputs = [item["output"] for request in requests for item in request["input"]
               if item.get("type") == "function_call_output" and item["call_id"] == call_id]
    require(outputs, f"read result {call_id} was not returned to the provider")
    require(all(output == expected for output in outputs),
            f"provider read result {call_id} did not match expected file content")


def verify_tool_events(events, expected):
    results = [event for event in events if event["type"] == "tool_finished"]
    require(len(results) == len(expected), "unexpected number of completed tools")
    require({event["name"] for event in results} == set(expected), "unexpected completed tools")
    for result in results:
        name = result["name"]
        require(result["is_error"] is False, f"{name} returned a tool error: {result['output']!r}")
        require(result["truncated"] is False, f"{name} output was truncated")
        if expected[name] is not None:
            require(result["output"] == expected[name], f"{name} returned unexpected file content")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("report", type=Path)
    args = parser.parse_args(argv)
    binary, report = args.binary.resolve(), args.report.resolve()
    if binary == report:
        parser.error("report must not overwrite the binary")
    smoke = Smoke(binary, report)
    try:
        # Remove an old success before any check; interrupted runs must not reuse it.
        report.unlink(missing_ok=True)
        smoke.write_report("running")
        smoke.execute()
        smoke.write_report("passed")
    except (Exception, KeyboardInterrupt) as error:
        detail = f"{type(error).__name__}: {error}"
        try:
            smoke.write_report("failed", detail)
        except OSError as report_error:
            print(f"Cannot write failure report: {report_error}", file=sys.stderr)
        print(f"FAIL: {detail}", file=sys.stderr)
        return 1
    print(f"{len(smoke.checks)} smoke checks passed; report: {report}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
