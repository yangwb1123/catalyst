"""Agent invocation, bounded output handling, and validator gates for
the ForgeOS review runner. Ported from the ai-batch-runner toolset
(ai/run-review.py) with its pbatch dependency removed.
"""

from __future__ import annotations

import os
import shlex
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from typing import Optional

from pathlib import Path

from review_bounds import (OUTPUT_MAX_BYTES, PROMPT_MAX_BYTES, kill_group,
                           read_stream, read_text_bounded, run_validation)
from review_core import (STAGES, STAGE_VARS, agent_failure_reason,
                           chain_variables, context_to_vars, fill_template)

_ROOT = Path(__file__).resolve().parent

def run_agent(cmd: list, cwd: str, timeout: int, cap: int) -> tuple:
    """Run the agent, streaming output live from a reader thread while the
    main thread enforces the deadline; a hung agent must not block the
    runner forever. Returns (returncode, lines); returncode is -1 on
    timeout and -2 when output overflowed the cap."""
    proc = None
    completed = False
    try:
        proc = subprocess.Popen(cmd, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True,
                                cwd=cwd, start_new_session=True)
        assert proc.stdout is not None
        lines: list = []
        overflow = [False]
        reader = threading.Thread(
            target=read_stream,
            args=(proc.stdout, lines, cap, overflow, True, sys.stdout),
            daemon=True,
        )
        reader.start()
        deadline = time.monotonic() + timeout
        try:
            proc.wait(timeout=max(0.1, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            kill_group(proc)
            reader.join(timeout=1)
            return -1, lines
        reader.join(timeout=max(0, deadline - time.monotonic()))
        if reader.is_alive():
            kill_group(proc)
            reader.join(timeout=1)
            return -1, lines
        completed = True
        return (-2 if overflow[0] else proc.returncode), lines
    finally:
        if proc is not None and not completed:
            kill_group(proc)


def run_validators(stage: str, out_file, output: str, commands: list,
                   cwd: str) -> int:
    """Run every resolved validator against a temp copy of the output (AND
    semantics); atomically rename on success, delete on failure so a result
    that fails the project checks never lands as a review file."""
    out_path = Path(out_file)
    if out_path.is_symlink():
        print(f"\nStage {stage} REJECTED: output path {out_file} is a symlink; output NOT saved", file=sys.stderr)
        return 1
    fd, tmp_name = tempfile.mkstemp(prefix=out_path.name + ".", suffix=".tmp",
                                     dir=str(out_path.parent))
    os.close(fd)
    tmp_file = Path(tmp_name)
    tmp_file.write_text(output, encoding="utf-8")
    for raw in commands:
        cmd = raw.replace("{output}", shlex.quote(str(tmp_file)))
        cmd = cmd.replace("{cwd}", shlex.quote(cwd))
        result = run_validation(cmd, cwd)
        if result.ok:
            continue
        tmp_file.unlink(missing_ok=True)
        detail = " (timeout)" if result.timed_out else f" (exit={result.exit_code})"
        print(f"\nStage {stage} REJECTED: validation failed{detail}: {cmd}; output NOT saved", file=sys.stderr)
        for line in (result.stdout or "").strip().splitlines()[-5:]:
            print(f"  | {line}", file=sys.stderr)
        for line in (result.stderr or "").strip().splitlines()[-5:]:
            print(f"  | {line}", file=sys.stderr)
        return 1
    tmp_file.rename(out_file)
    print(f"\nWROTE: {out_file} (validated)", flush=True)
    return 0


def resolve_validators(value: str) -> list:
    """Expand a comma-separated list into validation commands. Registry
    names resolve through the built-in map (empty by default); anything
    else is used as a raw shell command."""
    out = []
    for item in [x.strip() for x in (value or "").split(",") if x.strip()]:
        out.append(_DEFAULT_VALIDATORS.get(item, item))
    return out


def stage_output_file(args, stage: str) -> Path:
    out_dir = Path(args.output_dir) if args.output_dir else _ROOT / "reviews" / (args.context_name or "review")
    out_dir.mkdir(parents=True, exist_ok=True)
    return out_dir / f"stage-{stage}.out.md"


def stage_prompt(args, stage: str, prompts_dir, ctx: dict,
                 prior_outputs: dict) -> tuple:
    """Fill the stage template with context and chained variables from
    prior stage outputs. Returns (prompt, error_string)."""
    base = prompts_dir.resolve()
    candidate = base / STAGES[stage]
    template_file = candidate.resolve()
    try:
        template_file.relative_to(base)
    except ValueError:
        return "", f"template escapes prompts directory: {STAGES[stage]}"
    if candidate.is_symlink() or not template_file.is_file():
        return "", f"template not found: {template_file}"
    defaults = STAGE_VARS.get(stage, {})
    variables = {k: ctx.get(k.lower(), v) for k, v in defaults.items()}
    variables.update(context_to_vars(ctx, stage))
    variables.update(chain_variables(prior_outputs, stage, variables, defaults))
    try:
        return fill_template(template_file, variables), ""
    except ValueError as exc:
        return "", f"template rejected: {exc}"


def invoke_agent(stage: str, args, out_file: Path, cmd: list, agent_bin: str,
                 timeout: int) -> Optional[str]:
    """Run the agent and reject transport/provider failures before saving."""
    try:
        rc, output = run_agent(cmd, args.repo or os.getcwd(), timeout, OUTPUT_MAX_BYTES)
    except FileNotFoundError:
        print(f"ERROR: '{agent_bin}' not found in PATH.", file=sys.stderr)
        return None
    if rc == -2:
        print(f"\nStage {stage} REJECTED: agent output exceeds {OUTPUT_MAX_BYTES} bytes; output NOT saved", file=sys.stderr)
        return None
    if rc < 0:
        print(f"\nStage {stage} REJECTED: agent timed out; output NOT saved", file=sys.stderr)
        return None
    text = "".join(output)
    reason = agent_failure_reason(rc, text)
    if reason:
        print(f"\nStage {stage} REJECTED: {reason}; output NOT saved", file=sys.stderr)
        return None
    return text


def run_stage(stage: str, prompt: str, args, session_flags_list=None) -> int:
    """Invoke the agent and persist only validated output. Rejected output
    is not saved and the stage fails, so quota or rate-limit replies never
    become committed review artifacts."""
    out_file = stage_output_file(args, stage)
    size = len((prompt or "").encode("utf-8", errors="replace"))
    if size > PROMPT_MAX_BYTES:
        print(f"\nStage {stage} REJECTED: prompt exceeds {PROMPT_MAX_BYTES} bytes ({size}); output NOT saved", file=sys.stderr)
        return 1
    timeout = getattr(args, "timeout", 0)
    if isinstance(timeout, bool) or not isinstance(timeout, int) or timeout < 0:
        timeout = 0
    timeout = timeout or 600
    agent_bin = args.agent_bin or "pi"
    cmd = [agent_bin, "-p", prompt]
    if args.model:
        cmd.extend(["--model", args.model])
    if session_flags_list:
        cmd.extend(session_flags_list)
    print(f"\n{'='*60}\n  Stage {stage}: {STAGES[stage]}\n  Output: {out_file}\n{'='*60}\n", flush=True)
    output = invoke_agent(stage, args, out_file, cmd, agent_bin, timeout)
    if output is None:
        return 1
    validate_spec = ",".join(value for value in
                             (getattr(args, "validate", ""), getattr(args, "validate_cmd", ""))
                             if value)
    commands = resolve_validators(validate_spec)
    if commands:
        return run_validators(stage, out_file, output, commands,
                              args.repo or os.getcwd())
    if out_file.is_symlink():
        print(f"\nStage {stage} REJECTED: output path is a symlink; output NOT saved", file=sys.stderr)
        return 1
    out_file.write_text(output, encoding="utf-8")
    print(f"\nWROTE: {out_file}", flush=True)
    return 0


def chain_stage_output(stage: str, args, prior_outputs: dict, failures: list) -> None:
    """Load a just-written output through the same bounded chain gate."""
    out_file = stage_output_file(args, stage)
    if not out_file.exists():
        return
    try:
        prior_outputs[stage] = read_text_bounded(out_file, OUTPUT_MAX_BYTES, "review output")
    except ValueError as exc:
        print(f"ERROR: completed stage {stage} cannot be chained: {exc}", file=sys.stderr)
        failures.append(stage)


