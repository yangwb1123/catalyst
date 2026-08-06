"""Review template schema, context mapping, and failure-signature
classification for the ForgeOS review runner. Ported from the
ai-batch-runner toolset (ai/run-review.py) with its pbatch dependency
removed.
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

from review_bounds import INPUT_MAX_BYTES, read_text_bounded

_ROOT = Path(__file__).resolve().parent

try:
    import yaml
except ImportError:
    yaml = None

_DEFAULT_STAGES = {
    "00": "00-product-discovery.md",
    "01": "01-architecture-review.md",
    "02": "02-security-rfc-review.md",
    "03": "03-distributed-review.md",
    "04": "04-implementation-review.md",
    "05": "05-performance-review.md",
    "06": "06-production-readiness.md",
    "07": "07-sprint-planning.md",
    "08": "08-post-sprint-review.md",
    "09": "09-cto-review.md",
}

_DEFAULT_STAGE_VARS = {
    "00": {"PROJECT_NAME": "", "SUBSYSTEM": "", "FEATURE_DESCRIPTION": "",
           "BUSINESS_JUSTIFICATION": "", "TARGET_USERS": "",
           "PAIN_POINT_EVIDENCE": "", "COMPARABLE_IMPLEMENTATIONS": ""},
    "01": {"PROJECT_NAME": "", "SUBSYSTEM": "", "REPO_PATH": "",
           "PRIMARY_FILES": "", "ARCHITECTURE_SUMMARY": "",
           "PRODUCT_DISCOVERY_OUTPUT": ""},
    "02": {"PROJECT_NAME": "", "SUBSYSTEM": "", "REPO_PATH": "",
           "PRIMARY_FILES": "", "RFC_REFERENCES": "", "ARCHITECTURE_OUTPUT": ""},
    "03": {"PROJECT_NAME": "", "SUBSYSTEM": "", "REPO_PATH": "",
           "PRIMARY_FILES": "", "ARCHITECTURE_OUTPUT": "",
           "STORAGE_SUMMARY": "", "LOAD_PROFILE": ""},
    "04": {"PROJECT_NAME": "", "SUBSYSTEM": "", "REPO_PATH": "",
           "PRIMARY_FILES": "", "PRIOR_FINDINGS": ""},
    "05": {"PROJECT_NAME": "", "SUBSYSTEM": "", "REPO_PATH": "",
           "PRIMARY_FILES": "", "LOAD_PROFILE": "", "INFRA_SUMMARY": ""},
    "06": {"PROJECT_NAME": "", "SUBSYSTEM": "", "REPO_PATH": "",
           "PRIMARY_FILES": "", "DEPLOYMENT_TARGET": "", "SLO_TARGETS": "",
           "PRIOR_FINDINGS": ""},
    "07": {"PROJECT_NAME": "", "SUBSYSTEM": "", "SPRINT_GOAL": "",
           "TEAM_SIZE": "", "SPRINT_DURATION": "", "CRITICAL_HIGH_FINDINGS": "",
           "ARCHITECTURE_OUTPUT": "", "VELOCITY": ""},
    "08": {"PROJECT_NAME": "", "SUBSYSTEM": "", "SPRINT_GOAL": "",
           "COMMITTED_STORIES": "", "REPO_PATH": "", "SHIPPED_CHANGES": ""},
    "09": {"PROJECT_NAME": "", "SUBSYSTEM": "",
           "ALL_PRIOR_FINDINGS_SUMMARY": "", "CRITICAL_COUNT": "0",
           "HIGH_COUNT": "0", "GRADE_00": "N/A", "GRADE_01": "N/A",
           "GRADE_02": "N/A", "GRADE_03": "N/A", "GRADE_04": "N/A",
           "GRADE_05": "N/A", "GRADE_06": "N/A", "TEAM_SIZE": "", "AGE": ""},
}

_SESSION_FLAGS = {
    "start": ["--session-id", "{session}", "--name", "{name}"],
    "continue": ["--session-id", "{session}"],
}

_DEFAULT_VALIDATORS = {}

_AGENT_ERROR_PATTERNS = (
    re.compile(r"rate_?limit_?error", re.IGNORECASE),
    re.compile(r"insufficient_?quota", re.IGNORECASE),
    re.compile(r"quota_?exceeded", re.IGNORECASE),
    re.compile(r"credit_?balance_?too_?low", re.IGNORECASE),
    re.compile(r"insufficient\s+(?:balance|funds)", re.IGNORECASE),
    re.compile(r"billing_?error", re.IGNORECASE),
    re.compile(r"payment_?required", re.IGNORECASE),
    re.compile(r"invalid_?api_?key", re.IGNORECASE),
    re.compile(r"authentication_?error", re.IGNORECASE),
    re.compile(r"context_?length_?exceeded", re.IGNORECASE),
    re.compile(r"overloaded_?error", re.IGNORECASE),
    re.compile(r"429 too many requests", re.IGNORECASE),
    re.compile(r"network is unreachable", re.IGNORECASE),
    re.compile(r"no route to host", re.IGNORECASE),
    re.compile(r"temporary failure in name resolution", re.IGNORECASE),
    re.compile(r"name or service not known", re.IGNORECASE),
    re.compile(r"dns resolution failed", re.IGNORECASE),
    re.compile(r"getaddrinfo", re.IGNORECASE),
    re.compile(r"max retries exceeded", re.IGNORECASE),
    re.compile(r"certificate verify failed", re.IGNORECASE),
    re.compile(r"connectionerror", re.IGNORECASE),
    re.compile(r"sslerror", re.IGNORECASE),
    re.compile(r"proxyerror", re.IGNORECASE),
    re.compile(r"econnrefused", re.IGNORECASE),
    re.compile(r"econnreset", re.IGNORECASE),
    re.compile(r"etimedout", re.IGNORECASE),
    re.compile(r"curl: \(\d+\)", re.IGNORECASE),
    re.compile(r"connection timed out", re.IGNORECASE),
    re.compile(r"connect timed out", re.IGNORECASE),
    re.compile(r"operation timed out", re.IGNORECASE),
    re.compile(r"connection refused", re.IGNORECASE),
    re.compile(r"connection reset", re.IGNORECASE),
    re.compile(r"connection closed", re.IGNORECASE),
    re.compile(r"broken pipe", re.IGNORECASE),
    re.compile(r"failed to connect", re.IGNORECASE),
    re.compile(r"unable to connect", re.IGNORECASE),
    re.compile(r"could not connect", re.IGNORECASE),
    re.compile(r"^\[?error\]?:", re.IGNORECASE | re.MULTILINE),
    re.compile(r"^fatal:", re.IGNORECASE | re.MULTILINE),
)

CHAIN_SOURCES = {
    "01": {"PRODUCT_DISCOVERY_OUTPUT": ["00"]},
    "02": {"ARCHITECTURE_OUTPUT": ["01"]},
    "03": {"ARCHITECTURE_OUTPUT": ["01"]},
    "04": {"PRIOR_FINDINGS": ["01", "02", "03"]},
    "06": {"PRIOR_FINDINGS": ["02", "03", "04", "05"]},
    "07": {"CRITICAL_HIGH_FINDINGS": ["00", "01", "02", "03", "04", "05", "06"],
           "ARCHITECTURE_OUTPUT": ["01"]},
    "08": {"COMMITTED_STORIES": ["07"]},
    "09": {"ALL_PRIOR_FINDINGS_SUMMARY":
           ["00", "01", "02", "03", "04", "05", "06", "07", "08"]},
}


def load_stage_schema():
    """Load stages/vars from sdlc.yaml next to this script; fall back to
    the built-in defaults if the file is absent or unparsable."""
    sdlc_path = _ROOT / "sdlc.yaml"
    if not yaml or not sdlc_path.exists():
        return dict(_DEFAULT_STAGES), {k: dict(v) for k, v in _DEFAULT_STAGE_VARS.items()}
    try:
        data = yaml.safe_load(read_text_bounded(sdlc_path, INPUT_MAX_BYTES, "review schema"))
    except Exception:
        return dict(_DEFAULT_STAGES), {k: dict(v) for k, v in _DEFAULT_STAGE_VARS.items()}
    stages_cfg = (data or {}).get("stages")
    if not isinstance(stages_cfg, dict) or not stages_cfg:
        return dict(_DEFAULT_STAGES), {k: dict(v) for k, v in _DEFAULT_STAGE_VARS.items()}
    valid = all(isinstance(sid, str) and isinstance(value, dict)
                and isinstance(value.get("template", ""), str)
                and isinstance(value.get("vars", {}), dict)
                for sid, value in stages_cfg.items())
    if not valid:
        return dict(_DEFAULT_STAGES), {k: dict(v) for k, v in _DEFAULT_STAGE_VARS.items()}
    stages = {sid: s.get("template", "") for sid, s in stages_cfg.items()}
    stage_vars = {sid: dict(s.get("vars", {})) for sid, s in stages_cfg.items()}
    return stages, stage_vars


STAGES, STAGE_VARS = load_stage_schema()


def session_flags(key: str, session_id: str, session_name: str) -> list:
    flags = _SESSION_FLAGS.get(key, _SESSION_FLAGS["start"])
    return [f.replace("{session}", session_id).replace("{name}", session_name)
            for f in flags]


def load_context(path: str) -> dict:
    if not yaml:
        print("ERROR: PyYAML not installed; install it (pip install pyyaml) to use --context", file=sys.stderr)
        sys.exit(1)
    fpath = Path(path)
    if not fpath.exists():
        print(f"ERROR: context file not found: {path}", file=sys.stderr)
        sys.exit(1)
    try:
        data = yaml.safe_load(read_text_bounded(fpath, INPUT_MAX_BYTES, "review context")) or {}
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)
    if not isinstance(data, dict):
        print(f"ERROR: review context must be a mapping: {path}", file=sys.stderr)
        sys.exit(1)
    return data


def context_to_vars(ctx: dict, stage: str) -> dict:
    """Map context YAML keys to template {{VARIABLE}} placeholders."""
    files = ctx.get("files", [])
    files_str = "\n".join(f"  - {f}" for f in files) if isinstance(files, list) else str(files)
    rfcs = ctx.get("rfcs", [])
    rfcs_str = "\n".join(f"  - {r}" for r in rfcs) if isinstance(rfcs, list) else str(rfcs)
    base = {
        "PROJECT_NAME": ctx.get("project", ""),
        "SUBSYSTEM": ctx.get("subsystem", ""),
        "REPO_PATH": ctx.get("repo", os.getcwd()),
        "PRIMARY_FILES": files_str,
        "RFC_REFERENCES": rfcs_str,
        "ARCHITECTURE_SUMMARY": ctx.get("architecture_summary", "(see primary files)"),
        "STORAGE_SUMMARY": ctx.get("storage", ""),
        "LOAD_PROFILE": ctx.get("load_profile", "(not specified)"),
        "INFRA_SUMMARY": ctx.get("infra", "(not specified)"),
        "SLO_TARGETS": ctx.get("slo_targets", "(not specified)"),
        "DEPLOYMENT_TARGET": ctx.get("deployment_target", ctx.get("infra", "(not specified)")),
        "SPRINT_GOAL": ctx.get("sprint_goal", "(not specified)"),
        "TEAM_SIZE": str(ctx.get("team_size", "")),
        "SPRINT_DURATION": ctx.get("sprint_duration", ""),
        "VELOCITY": ctx.get("velocity", "(unknown)"),
        "AGE": ctx.get("age", "(unknown)"),
    }
    base.update(ctx.get(f"stage_{stage}", {}))
    return base


def fill_template(template_path: Path, variables: dict) -> str:
    text = read_text_bounded(template_path, INPUT_MAX_BYTES, "review template")
    for key, value in variables.items():
        text = text.replace("{{" + key + "}}", str(value) if value else f"(not provided: {key})")
    return text


def chain_variables(prior_outputs: dict, stage: str, variables: dict,
                    schema_defaults: dict) -> dict:
    """Inject completed stage outputs into the current stage's paste-style
    variables. A variable is treated as explicitly provided (left
    untouched) only when it differs from the schema placeholder default."""
    chained = {}
    for var, sources in CHAIN_SOURCES.get(stage, {}).items():
        current = variables.get(var)
        if current and current != schema_defaults.get(var):
            continue
        parts = []
        for src in sources:
            text = prior_outputs.get(src)
            if text:
                parts.append(f"--- Stage {src} output ---\n{text}")
        if parts:
            chained[var] = "\n\n".join(parts)
    return chained


def agent_failure_reason(returncode: int, output: str) -> str:
    """Return a short reason when the agent result must be discarded, or ''
    when the output is a usable result. Non-zero exit, empty output, and
    provider/CLI failure signatures reject the result."""
    if returncode != 0:
        return f"agent exited {returncode}"
    if not output or not output.strip():
        return "agent produced no output"
    for pattern in _AGENT_ERROR_PATTERNS:
        if pattern.search(output):
            return f"agent reported provider failure ({pattern.pattern})"
    return ""


